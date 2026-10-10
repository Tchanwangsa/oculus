//! Translating opencode's event stream into [`HarnessEvent`]s.

use std::time::Instant;

use serde_json::Value;

use crate::harness::child::str_of as s;
use crate::harness::event::{cap_output, classify, HarnessEvent, Provider};

use super::redact::scrub;
use super::session::SessionState;

/// The session id: `properties.sessionID` on most events, else inside `info`
/// or `part`. `info.id` is the session only on `session.created`/`updated`;
/// on a message event it is the message id.
pub(super) fn session_of(v: &Value) -> Option<&str> {
    let p = &v["properties"];
    p["sessionID"]
        .as_str()
        .or_else(|| p["info"]["sessionID"].as_str())
        .or_else(|| p["part"]["sessionID"].as_str())
        .or_else(|| {
            matches!(
                v["type"].as_str(),
                Some("session.created" | "session.updated")
            )
            .then(|| p["info"]["id"].as_str())
            .flatten()
        })
}

/// Events that name no session (pty, lsp, mcp, `server.heartbeat`…). Only a
/// `session.error` whose optional `sessionID` is missing is worth surfacing.
pub(super) fn translate_server(ty: &str, data: &Value) -> Vec<HarnessEvent> {
    if ty == "session.error" {
        let msg = data["error"]["data"]["message"]
            .as_str()
            .or_else(|| data["error"]["name"].as_str())
            .unwrap_or("opencode reported an error with no session");
        if is_trace(msg) {
            return Vec::new();
        }
        return vec![HarnessEvent::error_for(Provider::Opencode, friendly(msg))];
    }
    Vec::new()
}

/// Every `session.error` is sent twice, the second as a stack trace.
fn is_trace(msg: &str) -> bool {
    msg.contains("\n    at ")
}

/// A turn opens: reset the per-turn state and start the drain watchdog.
fn begin_turn(st: &mut SessionState) -> HarnessEvent {
    st.text.clear();
    st.reasoning.clear();
    st.tools.clear();
    st.open_tools.clear();
    st.tool_output_len.clear();
    st.assistants.clear();
    st.turn_errored = false;
    st.aborting = false;
    st.turn_open = true;
    st.awaiting_step = Some(Instant::now());
    HarnessEvent::TurnStarted
}

/// One `step-finish` folded into the running totals. `input` excludes cache
/// reads and `output` excludes reasoning, so both are added.
pub(super) fn usage(st: &mut SessionState, t: &Value, cost: f64) -> HarnessEvent {
    let n = |k: &str| t[k].as_u64().unwrap_or(0);
    let input = n("input")
        + t["cache"]["read"].as_u64().unwrap_or(0)
        + t["cache"]["write"].as_u64().unwrap_or(0);
    let output = n("output") + n("reasoning");
    st.total_input += input;
    st.total_output += output;
    st.total_cost += cost;
    HarnessEvent::Usage {
        input_tokens: st.total_input,
        output_tokens: st.total_output,
        context_tokens: Some(input + output),
        context_window: st.context_window,
        cost_usd: (st.total_cost > 0.0).then_some(st.total_cost),
    }
}

/// Every path that ends a turn comes here, and a closed turn yields nothing.
/// The queue releases a thread on `TurnFinished` alone, so a second one would
/// send a queued message twice and a missing one would strand the thread.
pub(super) fn close_turn(st: &mut SessionState, status: &str) -> Vec<HarnessEvent> {
    if !st.turn_open {
        return Vec::new();
    }
    st.turn_open = false;
    st.awaiting_step = None;
    let mut out = Vec::new();
    // Text whose closing snapshot never came (a dropped stream).
    let mut leftover: Vec<String> = st.text.drain().map(|(_, v)| v).collect();
    leftover.sort();
    for text in leftover {
        if !text.trim().is_empty() {
            out.push(HarnessEvent::AssistantMessage { text });
        }
    }
    st.reasoning.clear();
    st.tool_output_len.clear();
    let open: Vec<String> = st.open_tools.drain().collect();
    for id in open {
        out.push(HarnessEvent::ToolFinished {
            id,
            ok: false,
            output: "the turn ended before this finished".into(),
            title: None,
        });
    }
    out.push(HarnessEvent::TurnFinished {
        status: status.into(),
    });
    out
}

/// The free Zen models always refuse the HTTP API; that 400 becomes something
/// actionable. Everything else passes through.
pub(super) fn friendly(msg: &str) -> String {
    if msg.contains("free tier can only be used in OpenCode") || msg.contains("MissingSessionID") {
        return "opencode's free Zen models only work inside opencode itself — its gateway \
                refuses the app's requests. Pick a model from a provider you have signed in \
                to with `opencode auth login`."
            .into();
    }
    msg.to_string()
}

/// opencode's `{name, data:{message}}` error envelope (or `{message}`) as one
/// non-empty sentence.
pub(super) fn error_sentence(e: &Value) -> String {
    let message = e["data"]["message"]
        .as_str()
        .or_else(|| e["message"].as_str())
        .map(str::trim)
        .filter(|m| !m.is_empty());
    match (message, e["name"].as_str()) {
        (Some(m), _) => scrub(m),
        (None, Some(name)) => name.to_string(),
        (None, None) => scrub(&e.to_string().chars().take(400).collect::<String>()),
    }
}

/// Open a tool's row on its first `running` snapshot, or on a `completed`
/// that skipped `running`.
fn ensure_open(out: &mut Vec<HarnessEvent>, st: &mut SessionState, id: &str, input: &Value) {
    if st.open_tools.contains(id) {
        return;
    }
    let name = st
        .tools
        .get(id)
        .cloned()
        .unwrap_or_else(|| "unknown".into());
    let (kind, title) = classify(&name, input);
    st.open_tools.insert(id.to_string());
    out.push(HarnessEvent::ToolStarted {
        id: id.to_string(),
        kind,
        name,
        title,
        input: input.clone(),
    });
}

/// One session event's `properties` into harness events. The stream's rules
/// (1.18.31, replayed from `fixtures/harness/opencode-v1-*.ndjson`):
///
/// - Text and reasoning open as a `message.part.updated` snapshot, stream as
///   `message.part.delta` (`field` is `"text"` for both), and close as a
///   snapshot with the whole text. Tool parts never delta: each snapshot
///   re-sends the output so far.
/// - One assistant message per step. The user row is re-emitted after every
///   step, so only the first sight of its id opens a turn; that id is also
///   the `TurnAnchor`, since `prompt_async` returns none.
/// - An abort is `session.error{MessageAbortedError}`, idle, *then* the
///   partial answer and the errored assistant row, then a second idle.
/// - A bad model gets `session.error` then idle, and no assistant row; a bad
///   agent gets `session.error` and no idle at all.
pub(super) fn translate(ty: &str, p: &Value, st: &mut SessionState) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    match ty {
        "message.updated" => {
            let info = &p["info"];
            let id = s(info, "id");
            match info["role"].as_str() {
                // First sight of the id opens the turn, `summary` or not.
                Some("user") => {
                    if !st.seen_user.insert(id.clone()) {
                        return out;
                    }
                    out.push(begin_turn(st));
                    out.push(HarnessEvent::TurnAnchor { anchor: id });
                }
                // Created bare, then completed twice (plus once with `error`
                // on an abort). Only the transition to completed decides.
                Some("assistant") => {
                    st.awaiting_step = None;
                    let completed = !info["time"]["completed"].is_null();
                    let already = match st.assistants.iter_mut().find(|(i, _)| *i == id) {
                        Some((_, done)) => std::mem::replace(done, *done || completed),
                        None => {
                            st.assistants.push((id, completed));
                            false
                        }
                    };
                    if !completed || already {
                        return out;
                    }
                    let err = &info["error"];
                    if err["name"].as_str() == Some("MessageAbortedError") {
                        out.extend(close_turn(st, "interrupted"));
                    } else if err.is_object() {
                        let msg = err["data"]["message"]
                            .as_str()
                            .or_else(|| err["name"].as_str())
                            .unwrap_or("opencode error");
                        st.turn_errored = true;
                        out.push(HarnessEvent::error_for(Provider::Opencode, friendly(msg)));
                        out.extend(close_turn(st, "failed"));
                    } else if info["finish"].as_str() != Some("tool-calls") {
                        out.extend(close_turn(st, "completed"));
                    }
                }
                _ => {}
            }
        }
        "message.part.updated" => {
            let part = &p["part"];
            // The user's own prompt is a text part too.
            if st.seen_user.contains(&s(part, "messageID")) {
                return out;
            }
            let id = s(part, "id");
            match part["type"].as_str() {
                Some(kind @ ("text" | "reasoning")) => {
                    let thinking = kind == "reasoning";
                    let map = if thinking {
                        &mut st.reasoning
                    } else {
                        &mut st.text
                    };
                    if part["time"]["end"].is_null() {
                        map.entry(id).or_default();
                    } else {
                        map.remove(&id);
                        let text = s(part, "text");
                        if !text.trim().is_empty() {
                            out.push(if thinking {
                                HarnessEvent::Thinking { text }
                            } else {
                                HarnessEvent::AssistantMessage { text }
                            });
                        }
                    }
                }
                // Keyed by `callID`. `pending` has `input: {}` and is skipped.
                Some("tool") => {
                    let call = s(part, "callID");
                    st.tools.insert(call.clone(), s(part, "tool"));
                    let state = &part["state"];
                    let input = state.get("input").cloned().unwrap_or(Value::Null);
                    match state["status"].as_str() {
                        Some("running") => {
                            ensure_open(&mut out, st, &call, &input);
                            let output = state["metadata"]["output"].as_str().unwrap_or("");
                            let seen = st.tool_output_len.entry(call.clone()).or_insert(0);
                            if output.len() > *seen {
                                if let Some(text) = output.get(*seen..) {
                                    out.push(HarnessEvent::ToolOutputDelta {
                                        id: call.clone(),
                                        text: text.to_string(),
                                    });
                                }
                                *seen = output.len();
                            }
                        }
                        Some(status @ ("completed" | "error")) => {
                            let ok = status == "completed";
                            ensure_open(&mut out, st, &call, &input);
                            st.open_tools.remove(&call);
                            st.tool_output_len.remove(&call);
                            // A tool error (refusal, abort) does not end the turn.
                            let output = if ok {
                                s(state, "output")
                            } else {
                                state["error"]
                                    .as_str()
                                    .unwrap_or("the tool failed")
                                    .to_string()
                            };
                            out.push(HarnessEvent::ToolFinished {
                                id: call,
                                ok,
                                output: cap_output(&output),
                                title: None,
                            });
                        }
                        _ => {}
                    }
                }
                Some("step-finish") => {
                    out.push(usage(
                        st,
                        &part["tokens"],
                        part["cost"].as_f64().unwrap_or(0.0),
                    ));
                }
                _ => {}
            }
        }
        // A part never opened (a reconnect mid-answer) is read as answer text.
        "message.part.delta" => {
            let text = s(p, "delta");
            if text.is_empty() {
                return out;
            }
            let id = s(p, "partID");
            if let Some(acc) = st.reasoning.get_mut(&id) {
                acc.push_str(&text);
                out.push(HarnessEvent::ThinkingDelta { text });
            } else {
                st.text.entry(id).or_default().push_str(&text);
                out.push(HarnessEvent::AssistantDelta { text });
            }
        }
        "session.error" => {
            let err = &p["error"];
            if err["name"].as_str() == Some("MessageAbortedError") {
                // The errored assistant row that follows closes an abort,
                // unless none exists to carry it.
                st.aborting = true;
                if st.assistants.is_empty() {
                    out.extend(close_turn(st, "interrupted"));
                }
                return out;
            }
            let msg = err["data"]["message"]
                .as_str()
                .or_else(|| err["name"].as_str())
                .unwrap_or("opencode error");
            if is_trace(msg) {
                return out;
            }
            st.turn_errored = true;
            out.push(HarnessEvent::error_for(Provider::Opencode, friendly(msg)));
            // With no assistant row left open, nothing else will close it.
            let pending = st.assistants.last().is_some_and(|(_, done)| !done);
            if !pending {
                out.extend(close_turn(st, "failed"));
            }
        }
        // Normally the turn is already closed. Outside an abort, a turn still
        // open here (no assistant row, or one never completed) has nothing
        // else coming to close it.
        "session.idle" => {
            if !st.turn_open || st.aborting {
                return out;
            }
            if !st.turn_errored {
                out.push(HarnessEvent::error_for(
                    Provider::Opencode,
                    "opencode ended the turn without answering.",
                ));
            }
            out.extend(close_turn(st, "failed"));
        }
        _ => {}
    }
    out
}
