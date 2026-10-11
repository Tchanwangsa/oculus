//! Folding Claude's stream-json lines into [`HarnessEvent`]s.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::Value;

use crate::harness::child::str_of;
use crate::harness::event::{cap_output, classify, HarnessEvent, Provider, RateWindow};

use super::transcript::{anchor_for, tool_result_text, transcript_path};

/// Per-process translation state. `started_tools` dedupes: a tool_use block
/// can appear in more than one `assistant` line of the same message.
#[derive(Default)]
pub(super) struct Translator {
    pub(super) started_tools: std::collections::HashSet<String>,
    /// Between the first stream event of a turn and its `result`.
    pub(super) turn_open: bool,
    pub(super) saw_result: bool,
    /// What the last request occupied; `result.usage` sums the turn (spend).
    pub(super) last_context_tokens: Option<u64>,
    pub(super) interrupting: Arc<AtomicBool>,
    pub(super) expecting: Arc<AtomicBool>,
    /// From the `init` line; together they locate the transcript.
    pub(super) session_id: String,
    pub(super) cwd: String,
    /// The open turn's first `assistant` uuid — see [`super::transcript::anchor_for`].
    pub(super) turn_first_assistant: Option<String>,
}

impl Translator {
    fn open_turn(&mut self, out: &mut Vec<HarnessEvent>) {
        if !self.turn_open {
            self.turn_open = true;
            out.push(HarnessEvent::TurnStarted);
        }
    }

    pub(super) fn translate(&mut self, v: &Value) -> Vec<HarnessEvent> {
        let mut out = Vec::new();
        let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match ty {
            "system" => {
                if v.get("subtype").and_then(|s| s.as_str()) == Some("init") {
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
                                        out.push(HarnessEvent::AssistantDelta { text: t });
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
                                        out.push(HarnessEvent::AssistantDelta { text: t });
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
                for block in content {
                    match block.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            let t = str_of(block, "text");
                            if !t.is_empty() {
                                if !text.is_empty() {
                                    text.push('\n');
                                }
                                text.push_str(&t);
                            }
                        }
                        Some("thinking") => {
                            let t = str_of(block, "thinking");
                            if !t.trim().is_empty() {
                                out.push(HarnessEvent::Thinking { text: t });
                            }
                        }
                        Some("tool_use") => {
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
                let text = text.trim().to_string();
                if !text.is_empty() {
                    out.push(HarnessEvent::AssistantMessage { text });
                }
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
                // move the landmark `anchor_for` walks back from.
                let first = self.turn_first_assistant.take();
                if let Some(path) = transcript_path(&self.cwd, &self.session_id) {
                    if let Some(anchor) = anchor_for(&path, first.as_deref()) {
                        out.push(HarnessEvent::TurnAnchor { anchor });
                    }
                }
                self.turn_open = false;
                self.expecting.store(false, Ordering::SeqCst);
                out.push(HarnessEvent::TurnFinished {
                    status: status.into(),
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

    /// Recorded from `claude 2.1.267` asked to `ls` the library.
    #[test]
    fn folds_a_recorded_session() {
        let raw = include_str!("../../../../fixtures/harness/claude-ls.ndjson");
        let mut t = Translator::default();
        let events: Vec<HarnessEvent> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .flat_map(|v| t.translate(&v))
            .collect();

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
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(
            message.as_deref(),
            Some(deltas.as_str()),
            "deltas add up to the message"
        );

        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "completed")
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
        let mut t = Translator::default();
        let events: Vec<HarnessEvent> = raw
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .flat_map(|v| t.translate(&v))
            .collect();

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
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(message.as_deref(), Some(deltas.trim()));
        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted")
        );
    }
}
