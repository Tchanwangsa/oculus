//! Folding Codex notifications into [`HarnessEvent`]s.

use serde_json::{json, Value};

use crate::harness::child::str_of as s;
use crate::harness::event::cap_output;
use crate::harness::event::{classify, HarnessEvent, Provider, RateWindow, ToolKind};

use super::ThreadState;

/// Notifications about the account, not a thread.
pub(super) fn translate_account(method: &str, p: &Value) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    if method == "account/rateLimits/updated" {
        let windows = rate_windows(&p["rateLimits"]);
        if !windows.is_empty() {
            out.push(HarnessEvent::RateLimits { windows });
        }
    }
    out
}

/// The `rateLimits` object as the meter's windows. The keys are only
/// `primary`/`secondary`, so the duration is the label and the key a fallback.
pub(super) fn rate_windows(rl: &Value) -> Vec<RateWindow> {
    let mut windows = Vec::new();
    for (key, fallback) in [("primary", "5-hour"), ("secondary", "Weekly")] {
        let Some(w) = rl.get(key).filter(|w| !w.is_null()) else {
            continue;
        };
        let mins = w.get("windowDurationMins").and_then(|m| m.as_u64());
        let label = match mins {
            Some(10080) => "Weekly",
            Some(300) => "5-hour",
            Some(m) if m % 60 == 0 => return_label(format!("{}-hour", m / 60)),
            _ => fallback,
        };
        windows.push(RateWindow {
            label: label.to_string(),
            used_percent: w.get("usedPercent").and_then(|u| u.as_f64()).unwrap_or(0.0),
            resets_at: w.get("resetsAt").and_then(|r| r.as_i64()),
        });
    }
    windows
}

pub(super) fn translate(method: &str, p: &Value, st: &mut ThreadState) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    match method {
        "turn/started" => {
            st.active_turn = p
                .pointer("/turn/id")
                .and_then(|s| s.as_str())
                .map(String::from);
            st.ignore_usage_until_turn = false;
            st.open_items.clear();
            st.streamed_messages.clear();
            st.partial_message.clear();
            out.push(HarnessEvent::TurnStarted);
        }
        "turn/completed" => {
            st.active_turn = None;
            let status = match p.pointer("/turn/status").and_then(|s| s.as_str()) {
                Some("failed") => "failed",
                Some("interrupted") => "interrupted",
                _ => "completed",
            };
            if let Some(msg) = p.pointer("/turn/error/message").and_then(|m| m.as_str()) {
                out.push(HarnessEvent::error_for(Provider::Codex, msg));
            }
            // Commit what a cut-short turn had said; a normal turn already
            // cleared this on `item/completed`.
            let partial = std::mem::take(&mut st.partial_message);
            if !partial.trim().is_empty() {
                out.push(HarnessEvent::AssistantMessage { text: partial });
            }
            out.push(HarnessEvent::TurnFinished {
                status: status.into(),
            });
        }
        "item/started" => {
            let item = &p["item"];
            let id = s(item, "id");
            match s(item, "type").as_str() {
                "commandExecution" => {
                    let input = json!({ "command": s(item, "command"), "cwd": s(item, "cwd") });
                    let (kind, title) = classify("commandExecution", &input);
                    st.open_items.insert(id.clone(), kind);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind,
                        name: "commandExecution".into(),
                        title,
                        input,
                    });
                }
                "fileChange" => {
                    let paths: Vec<String> = item
                        .get("changes")
                        .and_then(|c| c.as_array())
                        .map(|a| a.iter().map(|c| s(c, "path")).collect())
                        .unwrap_or_default();
                    let title = paths
                        .iter()
                        .map(|p| p.rsplit('/').next().unwrap_or(p).to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    st.open_items.insert(id.clone(), ToolKind::Edit);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind: ToolKind::Edit,
                        name: "fileChange".into(),
                        title,
                        input: json!({ "paths": paths }),
                    });
                }
                "mcpToolCall" => {
                    let title = s(item, "tool");
                    st.open_items.insert(id.clone(), ToolKind::Other);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind: ToolKind::Other,
                        name: format!("mcp__{}__{}", s(item, "server"), s(item, "tool")),
                        title,
                        input: item.get("arguments").cloned().unwrap_or(Value::Null),
                    });
                }
                "webSearch" => {
                    let title = s(item, "query");
                    st.open_items.insert(id.clone(), ToolKind::Web);
                    out.push(HarnessEvent::ToolStarted {
                        id,
                        kind: ToolKind::Web,
                        name: "webSearch".into(),
                        title,
                        input: json!({ "query": s(item, "query") }),
                    });
                }
                _ => {}
            }
        }
        "item/agentMessage/delta" => {
            st.streamed_messages.insert(s(p, "itemId"));
            let text = s(p, "delta");
            if !text.is_empty() {
                st.partial_message.push_str(&text);
                out.push(HarnessEvent::AssistantDelta { text });
            }
        }
        "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
            let text = s(p, "delta");
            if !text.is_empty() {
                out.push(HarnessEvent::ThinkingDelta { text });
            }
        }
        "item/commandExecution/outputDelta" => {
            let text = s(p, "delta");
            if !text.is_empty() {
                out.push(HarnessEvent::ToolOutputDelta {
                    id: s(p, "itemId"),
                    text,
                });
            }
        }
        "item/completed" => {
            let item = &p["item"];
            let id = s(item, "id");
            let status = s(item, "status");
            match s(item, "type").as_str() {
                "agentMessage" => {
                    st.partial_message.clear();
                    let text = s(item, "text");
                    if !text.trim().is_empty() {
                        out.push(HarnessEvent::AssistantMessage { text });
                    }
                }
                "reasoning" => {
                    let parts: Vec<String> = ["summary", "content"]
                        .iter()
                        .filter_map(|k| item.get(*k).and_then(|a| a.as_array()))
                        .flatten()
                        .filter_map(|x| x.as_str())
                        .filter(|t| !t.trim().is_empty())
                        .map(String::from)
                        .collect();
                    if !parts.is_empty() {
                        out.push(HarnessEvent::Thinking {
                            text: parts.join("\n\n"),
                        });
                    }
                }
                "plan" => {
                    let text = s(item, "text");
                    if !text.trim().is_empty() {
                        out.push(HarnessEvent::AssistantMessage { text });
                    }
                }
                "commandExecution" => {
                    ensure_open(
                        &mut out,
                        st,
                        &id,
                        "commandExecution",
                        json!({ "command": s(item, "command"), "cwd": s(item, "cwd") }),
                    );
                    let exit = item.get("exitCode").and_then(|c| c.as_i64());
                    let mut output = s(item, "aggregatedOutput");
                    if let Some(c) = exit.filter(|c| *c != 0) {
                        if !output.is_empty() && !output.ends_with('\n') {
                            output.push('\n');
                        }
                        output.push_str(&format!("exit code {c}"));
                    }
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: status == "completed" && exit.unwrap_or(0) == 0,
                        output: cap_output(&output),
                        title: None,
                    });
                }
                "fileChange" => {
                    ensure_open(&mut out, st, &id, "fileChange", json!({}));
                    let diffs: Vec<String> = item
                        .get("changes")
                        .and_then(|c| c.as_array())
                        .map(|a| {
                            a.iter()
                                .map(|c| format!("--- {}\n{}", s(c, "path"), s(c, "diff")))
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: status == "completed",
                        output: cap_output(&diffs.join("\n")),
                        title: None,
                    });
                }
                "mcpToolCall" => {
                    ensure_open(&mut out, st, &id, "mcpToolCall", json!({}));
                    let output = item
                        .pointer("/error/message")
                        .and_then(|m| m.as_str())
                        .map(String::from)
                        .or_else(|| item.get("result").map(|r| r.to_string()))
                        .unwrap_or_default();
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: status == "completed",
                        output: cap_output(&output),
                        title: None,
                    });
                }
                "webSearch" => {
                    ensure_open(&mut out, st, &id, "webSearch", json!({}));
                    let (title, output) = web_search_detail(item);
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        // A finished search carries no `status` (0.153.4).
                        ok: matches!(status.as_str(), "" | "completed"),
                        output: cap_output(&output),
                        // `item/started` announces an empty `query`.
                        title: Some(title),
                    });
                }
                _ => {}
            }
        }
        "thread/tokenUsage/updated" => {
            if st.ignore_usage_until_turn {
                return out;
            }
            let u = &p["tokenUsage"];
            let n = |path: &str| u.pointer(path).and_then(|x| x.as_u64());
            out.push(HarnessEvent::Usage {
                input_tokens: n("/total/inputTokens").unwrap_or(0),
                output_tokens: n("/total/outputTokens").unwrap_or(0),
                context_tokens: n("/last/totalTokens"),
                context_window: n("/modelContextWindow"),
                cost_usd: None,
            });
        }
        "error" => {
            let msg = p
                .pointer("/error/message")
                .and_then(|m| m.as_str())
                .unwrap_or("codex error");
            let will_retry = p
                .get("willRetry")
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            if !will_retry {
                out.push(HarnessEvent::error_for(Provider::Codex, msg));
            }
        }
        _ => {}
    }
    out
}

/// Leaks a few bytes per distinct window length ever seen.
fn return_label(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// A finished web search as a row title and body. `query` is an elided
/// summary; `action.queries` is what was actually sent, so it wins.
fn web_search_detail(item: &Value) -> (String, String) {
    let queries: Vec<String> = item
        .pointer("/action/queries")
        .and_then(|q| q.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|q| q.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let title = match queries.split_first() {
        Some((first, [])) => first.clone(),
        Some((first, rest)) => format!("{first} (+{} more)", rest.len()),
        None => s(item, "query"),
    };

    let mut body = String::new();
    for q in &queries {
        body.push_str(&format!("search: {q}\n"));
    }
    let results = item.get("results").and_then(|r| r.as_array());
    for r in results.into_iter().flatten() {
        let (t, url) = (s(r, "title"), s(r, "url"));
        if t.is_empty() && url.is_empty() {
            continue;
        }
        if !body.is_empty() && !body.ends_with("\n\n") {
            body.push('\n');
        }
        body.push_str(&format!("{t}\n{url}\n"));
    }
    (title, body)
}

/// The server can complete an item it never announced; give it a row to close.
fn ensure_open(
    out: &mut Vec<HarnessEvent>,
    st: &mut ThreadState,
    id: &str,
    name: &str,
    input: Value,
) {
    if st.open_items.contains_key(id) {
        return;
    }
    let (kind, title) = classify(name, &input);
    st.open_items.insert(id.to_string(), kind);
    out.push(HarnessEvent::ToolStarted {
        id: id.to_string(),
        kind,
        name: name.into(),
        title,
        input,
    });
}
