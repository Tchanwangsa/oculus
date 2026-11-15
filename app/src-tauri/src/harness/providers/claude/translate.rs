//! Folding Claude's stream-json lines into [`HarnessEvent`]s.
//!
//! `Agent` and backgrounded Bash run in the background: the subagent's own
//! lines (tagged `parent_tool_use_id`) are dropped, the tool row stays open
//! until the task's `task_notification`, and the CLI then starts a turn on
//! its own, announced as [`HarnessEvent::UnpromptedTurn`].

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use base64::Engine;
use serde_json::Value;

use crate::harness::child::str_of;
use crate::harness::event::{
    cap_output, classify, GapCause, HarnessEvent, Provider, RateWindow, Review, Source, TurnId,
    Turns,
};

use super::approvals::Agent;
use super::transcript::{anchor_for, tool_result_text, transcript_path};

fn varint(bytes: &mut &[u8]) -> Option<u64> {
    let mut value = 0;
    for shift in (0..70).step_by(7) {
        let (&byte, rest) = bytes.split_first()?;
        *bytes = rest;
        if shift == 63 && byte > 1 {
            return None;
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn protobuf_bytes(mut bytes: &[u8], field: u64) -> Option<&[u8]> {
    let mut found = None;
    while !bytes.is_empty() {
        let tag = varint(&mut bytes)?;
        if tag >> 3 == 0 {
            return None;
        }
        let len = match tag & 7 {
            0 => {
                varint(&mut bytes)?;
                continue;
            }
            1 => 8,
            2 => usize::try_from(varint(&mut bytes)?).ok()?,
            5 => 4,
            _ => return None,
        };
        let value = bytes.get(..len)?;
        bytes = bytes.get(len..)?;
        if tag >> 3 == field && tag & 7 == 2 {
            found = Some(value);
        }
    }
    found
}

fn is_narration(frame: &Value, index: usize, block: &Value) -> bool {
    if let Some(indexes) = frame
        .get("narration_block_indexes")
        .and_then(Value::as_array)
    {
        return indexes.iter().any(|i| i.as_u64() == Some(index as u64));
    }
    // The CLI's signature kind is protobuf path 2 → 1 → 8. Unknown or
    // malformed envelopes stay reasoning; ciphertext is never searched.
    let kind = || {
        let signature = block.get("signature")?.as_str()?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(signature)
            .ok()?;
        let envelope = protobuf_bytes(&decoded, 2)?;
        let header = protobuf_bytes(envelope, 1)?;
        Some(protobuf_bytes(header, 8)? == b"narration")
    };
    kind().unwrap_or(false)
}

/// Emit the text blocks gathered so far as one message, named by its first block.
fn flush_text(text: &mut String, first: &mut Option<String>, out: &mut Vec<HarnessEvent>) {
    let text = std::mem::take(text).trim().to_string();
    let source = first.take().and_then(Source::of);
    if !text.is_empty() {
        out.push(HarnessEvent::AssistantMessage { text, source });
    }
}

/// A content block's id: the line's uuid (each `assistant` line is a snapshot)
/// and the block's index in it. No uuid, no id.
fn block_ref(frame: &Value, index: usize) -> Option<String> {
    let uuid = frame.get("uuid")?.as_str().filter(|u| !u.is_empty())?;
    Some(format!("{uuid}/{index}"))
}

/// A tool the CLI runs in the background, from its `task_started` to its
/// `task_notification`.
pub(super) struct BackgroundTask {
    description: String,
    task_type: String,
    /// The launch's tool_result ("running in background"), held until the task ends.
    held: Option<String>,
}

/// Per-process translation state. `started_tools` dedupes: a tool_use block
/// can appear in more than one `assistant` line of the same message.
#[derive(Default)]
pub(super) struct Translator {
    pub(super) started_tools: HashSet<String>,
    /// Between the first stream event of a turn and its `result`.
    pub(super) turn_open: bool,
    /// The session's turn ordinals, shared with [`super::session::ClaudeSession::send`].
    pub(super) turns: Arc<Turns>,
    /// The open turn's ordinal.
    pub(super) turn: Option<TurnId>,
    /// The turn the last `result` closed, which a repeated `result` names again.
    pub(super) last_turn: Option<TurnId>,
    pub(super) saw_result: bool,
    /// What the last request occupied; `result.usage` sums the turn (spend).
    pub(super) last_context_tokens: Option<u64>,
    pub(super) interrupting: Arc<AtomicBool>,
    /// A message of ours went in and its turn has not opened yet; a turn that
    /// opens without it is the CLI's own.
    pub(super) expecting: Arc<AtomicBool>,
    /// From the `init` line; together they locate the transcript.
    pub(super) session_id: String,
    pub(super) cwd: String,
    /// The open turn's first `assistant` uuid — see [`super::transcript::anchor_for`].
    pub(super) turn_first_assistant: Option<String>,
    /// The open turn is one the CLI started itself.
    pub(super) turn_unprompted: bool,
    /// Running background tasks by tool_use id.
    pub(super) background: HashMap<String, BackgroundTask>,
    /// How many of them, for the session's other threads to read: a switch
    /// to another provider would kill them with the process.
    pub(super) background_count: Arc<AtomicUsize>,
    /// Background rows their notification already finished; the launch's
    /// tool_result, arriving later, is dropped.
    pub(super) settled: HashSet<String>,
    /// Descriptions of background tasks finished since a turn last opened.
    pub(super) finished: Vec<String>,
    /// The subagents started in this process, by task id: a permission
    /// prompt names its subagent by that id (`approvals.rs`).
    pub(super) agents: HashMap<String, Agent>,
}

/// A decision the CLI made on a call without asking anyone, as its row
/// shows it: the Auto classifier's either way, or a deny rule's refusal
/// (the protected paths, `sqlite3`). The host's own answers and calls the
/// mode allows say nothing.
fn review_of(meta: &Value) -> Option<Review> {
    let decision = meta.get("permission_decision")?;
    let status = match (
        str_of(decision, "decision").as_str(),
        str_of(decision, "reason_type").as_str(),
    ) {
        ("accept", "classifier") => "approved",
        ("reject", "classifier" | "rule") => "denied",
        _ => return None,
    };
    Some(Review {
        status: status.into(),
        risk: None,
        rationale: None,
        source: Some(str_of(decision, "reason_type")),
    })
}

impl Translator {
    fn open_turn(&mut self, out: &mut Vec<HarnessEvent>) {
        if self.turn_open {
            return;
        }
        self.turn_open = true;
        if self.expecting.swap(false, Ordering::SeqCst) {
            // What finished before the student's turn is that turn's to take up.
            self.finished.clear();
            self.turn = Some(self.turns.open());
        } else {
            self.turn_unprompted = true;
            self.turn = Some(self.turns.own());
            out.push(HarnessEvent::UnpromptedTurn {
                after: std::mem::take(&mut self.finished),
            });
        }
        out.push(HarnessEvent::TurnStarted {
            turn: self.turn.clone(),
        });
    }

    /// The process is gone: background rows still running never will finish.
    pub(super) fn close(&mut self) -> Vec<HarnessEvent> {
        self.background_count.store(0, Ordering::SeqCst);
        self.background
            .drain()
            .map(|(id, _)| HarnessEvent::ToolFinished {
                id,
                ok: false,
                output: "the session ended before this finished".into(),
                title: None,
            })
            .collect()
    }

    /// A `task_started` or `task_notification` line.
    fn task(&mut self, v: &Value, out: &mut Vec<HarnessEvent>) {
        let id = str_of(v, "tool_use_id");
        if id.is_empty() {
            return;
        }
        let kind = str_of(v, "subagent_type");
        if v.get("subtype").and_then(|s| s.as_str()) == Some("task_started") && !kind.is_empty() {
            let agent = Agent {
                kind,
                tool: id.clone(),
                description: str_of(v, "description"),
            };
            self.agents.insert(str_of(v, "task_id"), agent);
        }
        match v.get("subtype").and_then(|s| s.as_str()) {
            // A foreground task (a subagent's own Bash) ends with its tool_result.
            Some("task_started")
                if v.get("is_backgrounded").and_then(Value::as_bool) == Some(true) =>
            {
                self.background.insert(
                    id,
                    BackgroundTask {
                        description: str_of(v, "description"),
                        task_type: str_of(v, "task_type"),
                        held: None,
                    },
                );
            }
            Some("task_notification") => {
                let Some(task) = self.background.remove(&id) else {
                    return;
                };
                if task.held.is_none() {
                    self.settled.insert(id.clone());
                }
                let output = if task.task_type == "local_agent" {
                    str_of(v, "summary")
                } else {
                    task.held.unwrap_or_else(|| str_of(v, "summary"))
                };
                out.push(HarnessEvent::ToolFinished {
                    id,
                    ok: v.get("status").and_then(|s| s.as_str()) == Some("completed"),
                    output: cap_output(&output),
                    title: None,
                });
                self.finished.push(task.description);
            }
            _ => {}
        }
    }

    pub(super) fn translate(&mut self, v: &Value) -> Vec<HarnessEvent> {
        let out = self.translate_line(v);
        self.background_count
            .store(self.background.len(), Ordering::SeqCst);
        out
    }

    fn translate_line(&mut self, v: &Value) -> Vec<HarnessEvent> {
        let mut out = Vec::new();
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        // A subagent's own output; the parent's tool row carries its result.
        if matches!(ty, "assistant" | "user" | "stream_event")
            && v.get("parent_tool_use_id").is_some_and(Value::is_string)
        {
            return out;
        }
        match ty {
            "system" => {
                let subtype = v.get("subtype").and_then(|s| s.as_str());
                if matches!(subtype, Some("task_started" | "task_notification")) {
                    self.task(v, &mut out);
                }
                if subtype == Some("init") {
                    self.session_id = str_of(v, "session_id");
                    self.cwd = str_of(v, "cwd");
                    out.push(HarnessEvent::SessionStarted {
                        provider_session_id: str_of(v, "session_id"),
                        model: v.get("model").and_then(|m| m.as_str()).map(String::from),
                        cwd: str_of(v, "cwd"),
                    });
                }
            }
            "stream_event" => {
                let Some(ev) = v.get("event") else { return out };
                let et = ev.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match et {
                    "message_start" => self.open_turn(&mut out),
                    "content_block_start" => {
                        // A start can carry text the deltas do not repeat.
                        if let Some(cb) = ev.get("content_block") {
                            match cb.get("type").and_then(|t| t.as_str()) {
                                Some("text") => {
                                    let t = str_of(cb, "text");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        // The stream names no block a committed message is known by.
                                        out.push(HarnessEvent::AssistantDelta {
                                            text: t,
                                            item: None,
                                        });
                                    }
                                }
                                Some("thinking") => {
                                    let t = str_of(cb, "thinking");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        out.push(HarnessEvent::ThinkingDelta { text: t });
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "content_block_delta" => {
                        if let Some(d) = ev.get("delta") {
                            match d.get("type").and_then(|t| t.as_str()) {
                                Some("text_delta") => {
                                    let t = str_of(d, "text");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        out.push(HarnessEvent::AssistantDelta {
                                            text: t,
                                            item: None,
                                        });
                                    }
                                }
                                Some("thinking_delta") => {
                                    let t = str_of(d, "thinking");
                                    if !t.is_empty() {
                                        self.open_turn(&mut out);
                                        out.push(HarnessEvent::ThinkingDelta { text: t });
                                    }
                                }
                                // Tool inputs arrive whole on the `assistant` line.
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            "assistant" => {
                let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
                    return out;
                };
                if self.turn_first_assistant.is_none() {
                    self.turn_first_assistant =
                        v.get("uuid").and_then(|u| u.as_str()).map(String::from);
                }
                self.open_turn(&mut out);
                if let Some(u) = v.pointer("/message/usage") {
                    let n = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                    let ctx = n("input_tokens")
                        + n("cache_read_input_tokens")
                        + n("cache_creation_input_tokens");
                    if ctx > 0 {
                        self.last_context_tokens = Some(ctx);
                    }
                }
                let mut text = String::new();
                let mut first = None;
                for (index, block) in content.iter().enumerate() {
                    match block.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            let t = str_of(block, "text");
                            if !t.is_empty() {
                                if !text.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(&t);
                                first = first.or_else(|| block_ref(v, index));
                            }
                        }
                        Some("thinking") => {
                            flush_text(&mut text, &mut first, &mut out);
                            let t = str_of(block, "thinking");
                            let source = block_ref(v, index);
                            let narration = is_narration(v, index, block);
                            if !t.trim().is_empty() {
                                let source = source.and_then(Source::of);
                                if narration {
                                    out.push(HarnessEvent::AssistantMessage { text: t, source });
                                } else {
                                    out.push(HarnessEvent::Thinking {
                                        text: t,
                                        ms: None,
                                        source,
                                    });
                                }
                            } else if narration {
                                // Public narration the CLI sent with no text.
                                out.push(HarnessEvent::DeliveryGap {
                                    source,
                                    cause: GapCause::EmptyNarration,
                                });
                            }
                        }
                        Some("tool_use") => {
                            flush_text(&mut text, &mut first, &mut out);
                            let id = str_of(block, "id");
                            if id.is_empty() || !self.started_tools.insert(id.clone()) {
                                continue;
                            }
                            let name = str_of(block, "name");
                            let input = block.get("input").cloned().unwrap_or(Value::Null);
                            let (kind, title) = classify(&name, &input);
                            out.push(HarnessEvent::ToolStarted {
                                id,
                                kind,
                                name,
                                title,
                                input,
                            });
                        }
                        _ => {}
                    }
                }
                flush_text(&mut text, &mut first, &mut out);
            }
            "user" => {
                // Only tool results; the user's own text is what we sent.
                let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
                    return out;
                };
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) != Some("tool_result") {
                        continue;
                    }
                    let id = str_of(block, "tool_use_id");
                    let is_error = block
                        .get("is_error")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    let output = tool_result_text(block, v.get("tool_use_result"));
                    if self.settled.remove(&id) {
                        continue;
                    }
                    let meta = v.get("tool_result_meta").and_then(Value::as_array);
                    let review = meta
                        .into_iter()
                        .flatten()
                        .find(|m| str_of(m, "id") == id)
                        .and_then(review_of);
                    if let Some(review) = review {
                        out.push(HarnessEvent::ToolReviewed {
                            id: id.clone(),
                            review,
                        });
                    }
                    // A launch that failed has no task to wait for.
                    if let Some(task) = self.background.get_mut(&id) {
                        if !is_error {
                            task.held = Some(output);
                            continue;
                        }
                        self.background.remove(&id);
                    }
                    out.push(HarnessEvent::ToolFinished {
                        id,
                        ok: !is_error,
                        output: cap_output(&output),
                        title: None,
                    });
                }
            }
            "result" => {
                self.saw_result = true;
                let is_error = v.get("is_error").and_then(|b| b.as_bool()).unwrap_or(false);
                let subtype = v.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                // A stopped turn reports `is_error: true` with an
                // `[ede_diagnostic]` in `errors`; only `terminal_reason:
                // "aborted_streaming"` (or our flag) names it. The rest is fallback.
                let interrupted = self.interrupting.swap(false, Ordering::SeqCst)
                    || v.get("terminal_reason")
                        .and_then(|t| t.as_str())
                        .is_some_and(|t| t.starts_with("aborted"))
                    || matches!(
                        v.get("stop_reason").and_then(|s| s.as_str()),
                        Some("interrupted") | Some("interrupt")
                    )
                    || subtype.contains("interrupt");
                // An interrupted result reports zero usage; don't fold it in.
                if let Some(u) = v.get("usage").filter(|_| !interrupted) {
                    let n = |k: &str| u.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                    let input = n("input_tokens");
                    let cached = n("cache_read_input_tokens") + n("cache_creation_input_tokens");
                    let context_window =
                        v.get("modelUsage")
                            .and_then(|m| m.as_object())
                            .and_then(|m| {
                                m.values()
                                    .filter_map(|x| x.get("contextWindow")?.as_u64())
                                    .max()
                            });
                    out.push(HarnessEvent::Usage {
                        input_tokens: input + cached,
                        output_tokens: n("output_tokens"),
                        context_tokens: self.last_context_tokens,
                        context_window,
                        cost_usd: v.get("total_cost_usd").and_then(|c| c.as_f64()),
                    });
                }
                if is_error && !interrupted {
                    let msg = v
                        .get("result")
                        .and_then(|r| r.as_str())
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .or_else(|| {
                            v.get("errors").and_then(|e| e.as_array()).map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            })
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("claude: {subtype}"));
                    out.push(HarnessEvent::error_for(Provider::Claude, msg));
                }
                let status = if interrupted {
                    "interrupted"
                } else if is_error {
                    "failed"
                } else {
                    "completed"
                };
                // The turn's rows are on disk now, and the next turn would
                // move the landmark `anchor_for` walks back from. An unprompted
                // turn's question is the CLI's synthetic notification row; the
                // anchor belongs to the student's question.
                let first = self.turn_first_assistant.take();
                if !self.turn_unprompted {
                    if let Some(path) = transcript_path(&self.cwd, &self.session_id) {
                        if let Some(anchor) = anchor_for(&path, first.as_deref()) {
                            out.push(HarnessEvent::TurnAnchor { anchor });
                        }
                    }
                }
                let turn = match self.turn.take() {
                    Some(turn) => Some(turn),
                    // A result with no stream events before it (an error) still answers our message.
                    None if self.expecting.swap(false, Ordering::SeqCst) => self.turns.take_sent(),
                    // A repeated `result` names the turn it already closed.
                    None => self.last_turn.clone(),
                };
                self.last_turn = turn.clone();
                self.turn_open = false;
                self.turn_unprompted = false;
                out.push(HarnessEvent::TurnFinished {
                    status: status.into(),
                    turn,
                });
            }
            "rate_limit_event" => {
                if let Some(w) = v
                    .pointer("/rate_limit_info/unifiedWindows")
                    .and_then(|w| w.as_object())
                {
                    let mut windows = Vec::new();
                    for (k, win) in w {
                        let label = match k.as_str() {
                            "five_hour" => "5-hour",
                            "seven_day" => "Weekly",
                            "seven_day_opus" => "Weekly Opus",
                            "seven_day_sonnet" => "Weekly Sonnet",
                            other => other,
                        };
                        windows.push(RateWindow {
                            label: label.to_string(),
                            used_percent: win
                                .get("utilization")
                                .and_then(|u| u.as_f64())
                                .unwrap_or(0.0)
                                * 100.0,
                            resets_at: win.get("resetsAt").and_then(|r| r.as_i64()),
                        });
                    }
                    windows.sort_by(|a, b| a.label.cmp(&b.label));
                    out.push(HarnessEvent::RateLimits { windows });
                }
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::event::ToolKind;

    const BACKGROUND: &str =
        include_str!("../../../../fixtures/harness/claude-background-agent.ndjson");
    const BACKGROUND_FAST: &str =
        include_str!("../../../../fixtures/harness/claude-background-agent-fast.ndjson");
    const AGENT: &str = "toolu_01RHoJhb1LDywxCL6b82JR7C";
    // Converted from T3's events.import-claudeagent-23e1f96c-…log,
    // lines 1328, 1331, 1334 and 2614: decoded payloads as stream-json frames.
    const NARRATION: &str =
        include_str!("../../../../fixtures/harness/claude-summarized-narration.ndjson");

    fn lines(raw: &str) -> Vec<Value> {
        raw.lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .collect()
    }

    /// Folds a recording the way the bridge does: `send` has set `expecting`
    /// before the first line comes back.
    fn replay(lines: Vec<Value>) -> (Translator, Vec<HarnessEvent>) {
        let mut t = Translator::default();
        t.expecting.store(true, Ordering::SeqCst);
        let events = lines.iter().flat_map(|v| t.translate(v)).collect();
        (t, events)
    }

    /// The recordings' thinking is redacted to ""; filled in, it shows whose rows survive.
    fn with_thinking(mut lines: Vec<Value>) -> Vec<Value> {
        for v in &mut lines {
            if let Some(blocks) = v
                .pointer_mut("/message/content")
                .and_then(|c| c.as_array_mut())
            {
                for b in blocks {
                    if b.get("type").and_then(|t| t.as_str()) == Some("thinking") {
                        b["thinking"] = Value::from("hmm");
                    }
                }
            }
        }
        lines
    }

    fn positions(events: &[HarnessEvent], f: impl Fn(&HarnessEvent) -> bool) -> Vec<usize> {
        events
            .iter()
            .enumerate()
            .filter(|(_, e)| f(e))
            .map(|(i, _)| i)
            .collect()
    }

    fn texts(events: &[HarnessEvent]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantMessage { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn agent_finished(e: &HarnessEvent) -> bool {
        matches!(e, HarnessEvent::ToolFinished { id, .. } if id == AGENT)
    }

    #[test]
    fn summarized_narration_is_assistant_text_before_its_tool() {
        let frames = lines(NARRATION);
        let (_, events) = replay(frames.clone());
        assert!(matches!(events[0], HarnessEvent::TurnStarted { .. }));
        assert!(matches!(
            &events[1], HarnessEvent::Thinking { text, .. }
                if text == frames[0]["message"]["content"][0]["thinking"].as_str().unwrap()
        ));
        assert!(matches!(
            &events[2], HarnessEvent::AssistantMessage { text, .. }
                if text == frames[1]["message"]["content"][0]["thinking"].as_str().unwrap()
        ));
        assert!(matches!(events[3], HarnessEvent::ToolStarted { .. }));
        assert!(matches!(
            &events[4], HarnessEvent::AssistantMessage { text, .. }
                if text == frames[3]["message"]["content"][0]["thinking"].as_str().unwrap()
        ));
        assert_eq!(events.len(), 5);
    }

    #[test]
    fn completed_block_order_is_preserved_across_text_narration_reasoning_and_tools() {
        let frame = serde_json::json!({
            "type": "assistant",
            "narration_block_indexes": [1],
            "message": {"content": [
                {"type": "text", "text": "Before"},
                {"type": "thinking", "thinking": "Update"},
                {"type": "text", "text": "Between"},
                {"type": "thinking", "thinking": "Reasoning"},
                {"type": "text", "text": "Calling"},
                {"type": "tool_use", "id": "tool", "name": "Bash", "input": {}},
                {"type": "text", "text": "After"}
            ]}
        });
        let (_, events) = replay(vec![frame]);
        let actual: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantMessage { text, .. } => Some(("assistant", text.as_str())),
                HarnessEvent::Thinking { text, .. } => Some(("thinking", text.as_str())),
                HarnessEvent::ToolStarted { id, .. } => Some(("tool", id.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            actual,
            [
                ("assistant", "Before"),
                ("assistant", "Update"),
                ("assistant", "Between"),
                ("thinking", "Reasoning"),
                ("assistant", "Calling"),
                ("tool", "tool"),
                ("assistant", "After")
            ]
        );
    }

    #[test]
    fn empty_narration_is_a_delivery_gap_not_text_or_reasoning() {
        for empty in ["", " \n\t"] {
            let mut frame = lines(NARRATION).remove(1);
            frame["message"]["content"][0]["thinking"] = empty.into();
            let (_, events) = replay(vec![frame]);
            assert!(matches!(
                events.as_slice(),
                [
                    HarnessEvent::TurnStarted { .. },
                    // This recording's lines carry no uuid, so no block id.
                    HarnessEvent::DeliveryGap {
                        source: None,
                        cause: GapCause::EmptyNarration
                    }
                ]
            ));
        }
    }

    #[test]
    fn narration_indexes_take_precedence_over_signatures() {
        let frames = lines(NARRATION);
        let mut thinking = frames[0].clone();
        thinking["narration_block_indexes"] = serde_json::json!([0]);
        let mut narration = frames[1].clone();
        narration["narration_block_indexes"] = serde_json::json!([]);
        let (_, events) = replay(vec![thinking, narration]);
        assert!(matches!(events[1], HarnessEvent::AssistantMessage { .. }));
        assert!(matches!(events[2], HarnessEvent::Thinking { .. }));
    }

    #[test]
    fn signature_fallback_only_promotes_the_narration_kind() {
        let frames = lines(NARRATION);
        let narration = &frames[1]["message"]["content"][0]["signature"];
        let thinking = &frames[0]["message"]["content"][0]["signature"];
        for (signature, promotes) in [
            (narration.clone(), true),
            (thinking.clone(), false),
            (Value::from("garbage!"), false),
            (Value::from(""), false),
            (Value::Null, false),
            (
                Value::from(base64::engine::general_purpose::STANDARD.encode(b"narration")),
                false,
            ),
        ] {
            let frame = serde_json::json!({
                "type": "assistant",
                "message": {"content": [{
                    "type": "thinking", "thinking": "Summary", "signature": signature
                }]}
            });
            let (_, events) = replay(vec![frame]);
            assert_eq!(
                matches!(events[1], HarnessEvent::AssistantMessage { .. }),
                promotes
            );
            assert_eq!(
                matches!(events[1], HarnessEvent::Thinking { .. }),
                !promotes
            );
        }
    }

    #[test]
    fn signature_fallback_rejects_malformed_protobuf_without_panicking() {
        // A minimal 2 → 1 → 8 envelope, without optional trailing fields.
        let bytes = b"\x12\x0d\x0a\x0b\x42\x09narration";
        let complete = serde_json::json!({
            "signature": base64::engine::general_purpose::STANDARD.encode(bytes)
        });
        assert!(is_narration(&Value::Null, 0, &complete));
        for truncated in 0..bytes.len() {
            let block = serde_json::json!({
                "signature": base64::engine::general_purpose::STANDARD.encode(&bytes[..truncated])
            });
            assert!(!is_narration(&Value::Null, 0, &block));
        }
        for malformed in [&[0xff; 11][..], &[0x12, 0xff], &[0x0b], &[0]] {
            let block = serde_json::json!({
                "signature": base64::engine::general_purpose::STANDARD.encode(malformed)
            });
            assert!(!is_narration(&Value::Null, 0, &block));
        }
    }

    /// Recorded from `claude 2.1.267` asked to `ls` the library.
    #[test]
    fn folds_a_recorded_session() {
        let raw = include_str!("../../../../fixtures/harness/claude-ls.ndjson");
        let (_, events) = replay(lines(raw));

        let session = events.iter().find_map(|e| match e {
            HarnessEvent::SessionStarted {
                provider_session_id,
                ..
            } => Some(provider_session_id.clone()),
            _ => None,
        });
        assert!(session.is_some(), "session id from system/init");

        let tools: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolStarted { kind, title, .. } => Some((*kind, title.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(tools, vec![(ToolKind::Bash, "ls -a".to_string())]);

        let finished = events
            .iter()
            .filter(|e| matches!(e, HarnessEvent::ToolFinished { ok: true, .. }))
            .count();
        assert_eq!(finished, 1);

        let deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(
            message.as_deref(),
            Some(deltas.as_str()),
            "deltas add up to the message"
        );

        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status, .. }) if status == "completed")
        );
        assert!(events.iter().any(|e| matches!(
            e,
            HarnessEvent::Usage {
                cost_usd: Some(_),
                ..
            }
        )));
        assert!(events
            .iter()
            .any(|e| matches!(e, HarnessEvent::RateLimits { windows } if windows.len() == 2)));
    }

    /// Recorded: the stop's `result` calls itself an error, with zeroed usage.
    #[test]
    fn a_stopped_turn_is_not_an_error() {
        let raw = include_str!("../../../../fixtures/harness/claude-interrupt.ndjson");
        let (_, events) = replay(lines(raw));

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, HarnessEvent::Error { .. })),
            "the CLI's own diagnostic is not something the student did"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, HarnessEvent::Usage { .. })),
            "an interrupted result reports zeros; folding them in blanks the meter"
        );
        let deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text, .. } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(message.as_deref(), Some(deltas.trim()));
        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status, .. }) if status == "interrupted")
        );
    }

    /// Recorded from `claude 2.1.294`: a background `Agent` that finishes after
    /// the turn that launched it, and the turn the CLI then starts itself.
    #[test]
    fn a_background_agent_finishes_its_row_and_starts_an_unprompted_turn() {
        let (_, events) = replay(with_thinking(lines(BACKGROUND)));

        let tools: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolStarted { id, name, .. } => Some((id.as_str(), name.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            tools,
            vec![(AGENT, "Agent")],
            "the subagent's Bash is not ours"
        );
        assert_eq!(texts(&events), vec!["launched", "PONG"]);
        let thinking = positions(&events, |e| matches!(e, HarnessEvent::Thinking { .. }));
        assert_eq!(thinking.len(), 3, "only the parent's three thinking blocks");

        let started = positions(&events, |e| matches!(e, HarnessEvent::TurnStarted { .. }));
        let finished = positions(
            &events,
            |e| matches!(e, HarnessEvent::TurnFinished { status, .. } if status == "completed"),
        );
        assert_eq!(started.len(), 2);
        assert_eq!(finished.len(), 2);
        // The stream names no turn: each gets the next ordinal as it opens,
        // the CLI's own included, and ends under the one it opened as.
        let turns: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::TurnStarted { turn } => Some(("started", turn.as_deref())),
                HarnessEvent::TurnFinished { turn, .. } => Some(("finished", turn.as_deref())),
                _ => None,
            })
            .collect();
        assert_eq!(
            turns,
            vec![
                ("started", Some("1")),
                ("finished", Some("1")),
                ("started", Some("2")),
                ("finished", Some("2")),
            ]
        );
        assert_eq!(
            positions(&events, |e| matches!(e, HarnessEvent::TurnFinished { .. })).len(),
            2
        );

        let unprompted = positions(
            &events,
            |e| matches!(e, HarnessEvent::UnpromptedTurn { after } if after == &["Say pong"]),
        );
        assert_eq!(unprompted, vec![started[1] - 1]);
        let pong = positions(
            &events,
            |e| matches!(e, HarnessEvent::AssistantMessage { text, .. } if text == "PONG"),
        );
        assert!(pong[0] > unprompted[0]);

        let agent = positions(&events, agent_finished);
        assert_eq!(agent.len(), 1);
        assert!(
            agent[0] > finished[0] && agent[0] < unprompted[0],
            "the row runs past the turn that launched it"
        );
        assert!(matches!(
            &events[agent[0]],
            HarnessEvent::ToolFinished { ok: true, output, .. } if output == "PONG"
        ));
        assert!(
            !events[unprompted[0]..]
                .iter()
                .any(|e| matches!(e, HarnessEvent::TurnAnchor { .. })),
            "the anchor belongs to the student's question"
        );
    }

    /// Recorded: the background agent finishes while its launching turn still runs.
    #[test]
    fn a_background_agent_that_finishes_mid_turn_still_starts_its_own_turn() {
        let (_, events) = replay(with_thinking(lines(BACKGROUND_FAST)));
        let agent = positions(&events, |e| matches!(e, HarnessEvent::ToolFinished { .. }));
        let finished = positions(&events, |e| matches!(e, HarnessEvent::TurnFinished { .. }));
        let started = positions(&events, |e| matches!(e, HarnessEvent::TurnStarted { .. }));
        let unprompted = positions(
            &events,
            |e| matches!(e, HarnessEvent::UnpromptedTurn { after } if after == &["Say pong"]),
        );
        assert_eq!(agent.len(), 1);
        assert!(matches!(
            &events[agent[0]],
            HarnessEvent::ToolFinished { ok: true, output, .. } if output == "PONG"
        ));
        assert_eq!(finished.len(), 2);
        assert!(agent[0] < finished[0]);
        assert_eq!(unprompted, vec![started[1] - 1]);
        assert!(unprompted[0] > finished[0]);
        assert_eq!(texts(&events), vec!["launched", "PONG"]);
        assert_eq!(
            positions(&events, |e| matches!(e, HarnessEvent::Thinking { .. })).len(),
            3
        );
        assert!(!events[unprompted[0]..]
            .iter()
            .any(|e| matches!(e, HarnessEvent::TurnAnchor { .. })));
    }

    #[test]
    fn a_session_that_ends_fails_the_background_rows_still_running() {
        let all = lines(BACKGROUND);
        let first_result = all
            .iter()
            .position(|v| v.get("type").and_then(|t| t.as_str()) == Some("result"))
            .unwrap();
        let (mut t, events) = replay(all[..=first_result].to_vec());
        assert!(!events.iter().any(agent_finished), "still running");
        let closed = t.close();
        assert!(matches!(
            closed.as_slice(),
            [HarnessEvent::ToolFinished { id, ok: false, .. }] if id == AGENT
        ));
        assert!(t.close().is_empty(), "finished once");
    }
}
