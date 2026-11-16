//! `session/update` notifications and the prompt's reply, folded into
//! [`HarnessEvent`]s. ACP's chunks carry no message id, so the translator
//! names each block of text or thought itself (`msg-<n>`, `thought-<n>`); a
//! block closes at the next tool call, the next block of the other kind, or
//! the prompt's reply. Only the reply says the stream is whole: a block cut
//! off by a dying process is kept as partial, with a delivery gap.

use std::collections::HashSet;
use std::sync::Arc;

use serde_json::Value;

use crate::harness::event::{
    cap_output, classify, GapCause, HarnessEvent, Provider, Source, TurnId, Turns,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Thought,
}

struct Block {
    kind: Kind,
    id: String,
    text: String,
}

#[derive(Default)]
pub(super) struct Translator {
    /// The session's turn ordinals, shared with `send`, which reserves one.
    pub(super) turns: Arc<Turns>,
    /// The open turn's ordinal, from its first update.
    turn: Option<TurnId>,
    block: Option<Block>,
    blocks: u64,
    started: HashSet<String>,
    finished: HashSet<String>,
    /// The latest `usage_update`: tokens in the context, and its size.
    context: Option<(u64, u64)>,
}

/// How a prompt ended, as the turn model names it.
pub(super) fn status(stop_reason: &str, interrupted: bool) -> Result<&'static str, String> {
    match stop_reason {
        _ if interrupted => Ok("interrupted"),
        "cancelled" => Ok("interrupted"),
        "end_turn" | "max_tokens" | "max_turn_requests" => Ok("completed"),
        "refusal" => Err("Antigravity refused to go on with this turn.".into()),
        other => Err(format!("Antigravity ended the turn with {other:?}")),
    }
}

impl Translator {
    pub(super) fn new(turns: Arc<Turns>) -> Self {
        Translator {
            turns,
            ..Default::default()
        }
    }

    fn open_turn(&mut self, out: &mut Vec<HarnessEvent>) {
        if self.turn.is_none() {
            let turn = self.turns.open();
            self.turn = Some(turn.clone());
            out.push(HarnessEvent::TurnStarted { turn: Some(turn) });
        }
    }

    /// Close the open block. `done`: the stream is known whole; else it is
    /// partial, with a gap unless the student stopped it.
    fn flush(&mut self, out: &mut Vec<HarnessEvent>, done: bool, stopped: bool) {
        let Some(block) = self.block.take() else {
            return;
        };
        if block.text.trim().is_empty() {
            return;
        }
        let source = match done {
            true => Source::of(block.id.clone()),
            false => Source::partial(block.id.clone()),
        };
        out.push(match block.kind {
            Kind::Text => HarnessEvent::AssistantMessage {
                text: block.text,
                source,
            },
            Kind::Thought => HarnessEvent::Thinking {
                text: block.text,
                ms: None,
                source,
            },
        });
        if !done && !stopped && block.kind == Kind::Text {
            out.push(HarnessEvent::DeliveryGap {
                source: Some(block.id),
                cause: GapCause::MissingSnapshot,
            });
        }
    }

    fn chunk(&mut self, kind: Kind, update: &Value, out: &mut Vec<HarnessEvent>) {
        let Some(text) = update.pointer("/content/text").and_then(Value::as_str) else {
            return;
        };
        if text.is_empty() {
            return;
        }
        if self.block.as_ref().is_some_and(|b| b.kind != kind) {
            // A block ends where the other kind starts: the stream is in order.
            self.flush(out, true, false);
        }
        let block = self.block.get_or_insert_with(|| {
            self.blocks += 1;
            let id = match kind {
                Kind::Text => format!("msg-{}", self.blocks),
                Kind::Thought => format!("thought-{}", self.blocks),
            };
            Block {
                kind,
                id,
                text: String::new(),
            }
        });
        block.text.push_str(text);
        out.push(match kind {
            Kind::Text => HarnessEvent::AssistantDelta {
                text: text.into(),
                item: Some(block.id.clone()),
            },
            Kind::Thought => HarnessEvent::ThinkingDelta { text: text.into() },
        });
    }

    /// One `session/update`'s `update`, during a prompt.
    pub(super) fn update(&mut self, update: &Value) -> Vec<HarnessEvent> {
        let mut out = Vec::new();
        match update["sessionUpdate"].as_str().unwrap_or("") {
            "agent_message_chunk" => {
                self.open_turn(&mut out);
                self.chunk(Kind::Text, update, &mut out);
            }
            "agent_thought_chunk" => {
                self.open_turn(&mut out);
                self.chunk(Kind::Thought, update, &mut out);
            }
            "tool_call" | "tool_call_update" => {
                self.open_turn(&mut out);
                self.tool(update, &mut out);
            }
            "usage_update" => {
                let n = |k: &str| update[k].as_u64();
                if let (Some(used), Some(size)) = (n("used"), n("size")) {
                    self.context = Some((used, size));
                }
            }
            // The echo of our own prompt, the agent's title for the session,
            // its commands, settings and mode: nothing for the timeline.
            _ => {}
        }
        out
    }

    fn tool(&mut self, update: &Value, out: &mut Vec<HarnessEvent>) {
        let Some(id) = update["toolCallId"].as_str().map(String::from) else {
            return;
        };
        if self.finished.contains(&id) {
            return;
        }
        if self.started.insert(id.clone()) {
            self.flush(out, true, false);
            let name = update["name"].as_str().unwrap_or("").to_string();
            let input = match &update["rawInput"] {
                Value::Null => update["args"].clone(),
                raw => raw.clone(),
            };
            let (kind, mut title) = classify(&name, &input);
            if title.is_empty() {
                title = update["title"].as_str().unwrap_or("").to_string();
            }
            out.push(HarnessEvent::ToolStarted {
                id: id.clone(),
                kind,
                name,
                title,
                input,
            });
        }
        let ok = match update["status"].as_str() {
            // A command's `completed` is the call's lifecycle, not its
            // success: `false` completes with `exit_code: 1`.
            Some("completed") => update
                .pointer("/rawOutput/exit_code")
                .and_then(Value::as_i64)
                .is_none_or(|code| code == 0),
            Some("failed") => false,
            _ => return,
        };
        self.finished.insert(id.clone());
        let output = output_of(update).unwrap_or_else(|| match ok {
            true => String::new(),
            false => "tool failed".into(),
        });
        out.push(HarnessEvent::ToolFinished {
            id,
            ok,
            output: cap_output(&output),
            title: None,
        });
    }

    /// The prompt's reply: its stop reason, or the error it failed with.
    /// `interrupted`: the student asked for the stop.
    pub(super) fn finish(
        &mut self,
        reply: Result<Value, String>,
        interrupted: bool,
    ) -> Vec<HarnessEvent> {
        let mut out = Vec::new();
        let (status, error) = match &reply {
            Ok(result) => match status(result["stopReason"].as_str().unwrap_or(""), interrupted) {
                Ok(s) => (s, None),
                Err(e) => ("failed", Some(e)),
            },
            Err(_) if interrupted => ("interrupted", None),
            Err(e) => ("failed", Some(e.clone())),
        };
        self.flush(&mut out, status == "completed", status == "interrupted");
        let usage = reply.as_ref().ok().map(|r| &r["usage"]);
        let n = |k: &str| usage.and_then(|u| u[k].as_u64());
        if usage.is_some_and(|u| u.is_object()) || self.context.is_some() {
            out.push(HarnessEvent::Usage {
                input_tokens: n("inputTokens").unwrap_or(0),
                output_tokens: n("outputTokens").unwrap_or(0),
                context_tokens: self.context.map(|(used, _)| used),
                context_window: self.context.map(|(_, size)| size),
                cost_usd: None,
            });
        }
        if let Some(e) = error {
            out.push(HarnessEvent::error_for(Provider::Antigravity, e));
        }
        let turn = self.turn.take().or_else(|| self.turns.take_sent());
        self.started.clear();
        self.finished.clear();
        out.push(HarnessEvent::TurnFinished {
            status: status.into(),
            turn,
        });
        out
    }

    /// The process is gone mid-turn: a block still streaming is kept as
    /// partial, and the turn it was in is the caller's to fail.
    pub(super) fn close(&mut self) -> (Vec<HarnessEvent>, Option<TurnId>) {
        let mut out = Vec::new();
        self.flush(&mut out, false, false);
        (out, self.turn.take())
    }
}

/// What a finished call printed: a command's `rawOutput.output`, else the
/// text of its content blocks.
fn output_of(update: &Value) -> Option<String> {
    if let Some(s) = update.pointer("/rawOutput/output").and_then(Value::as_str) {
        return Some(s.to_string());
    }
    let text: Vec<&str> = update["content"]
        .as_array()?
        .iter()
        .filter_map(|c| c.pointer("/content/text").and_then(Value::as_str))
        .collect();
    (!text.is_empty()).then(|| text.join("\n"))
}
