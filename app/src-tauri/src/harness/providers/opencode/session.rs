//! One session per thread: its route on the stream, its per-turn state, and
//! the calls that open, prompt, rewind and drop it.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde_json::json;

use crate::harness::event::HarnessEvent;
use crate::harness::Sink;

use super::models::split_model;
use super::server::OpencodeServer;
use super::types::OpencodeSessionOpts;

pub(super) struct SessionRoute {
    pub(super) sink: Sink,
    /// Sent on every prompt: the agent and model a session was created with
    /// are ignored, and a prompt that omits them runs as opencode's stock
    /// `build` agent with none of the containment (1.18.31).
    pub(super) agent: &'static str,
    pub(super) model: Option<String>,
    pub(super) variant: Option<String>,
    pub(super) state: Mutex<SessionState>,
}

#[derive(Default)]
pub(super) struct SessionState {
    /// Only [`super::events::close_turn`] clears this, so `TurnFinished` fires once per turn.
    pub(super) turn_open: bool,
    /// Text by part id, from the deltas. Whatever is left at close is a part
    /// that never got its closing snapshot, and is committed rather than lost.
    pub(super) text: HashMap<String, String>,
    pub(super) reasoning: HashMap<String, String>,
    /// `callID` → tool name.
    pub(super) tools: HashMap<String, String>,
    pub(super) open_tools: std::collections::HashSet<String>,
    /// `callID` → how much of a running tool's `state.metadata.output` has
    /// gone out as [`HarnessEvent::ToolOutputDelta`].
    pub(super) tool_output_len: HashMap<String, usize>,
    /// User message ids a turn has opened on; the user row is re-emitted after
    /// every step.
    pub(super) seen_user: std::collections::HashSet<String>,
    /// This turn's assistant messages (one per step) and whether each has
    /// `time.completed`.
    pub(super) assistants: Vec<(String, bool)>,
    /// An `Error` row has gone out, so a backstop close need not add one.
    pub(super) turn_errored: bool,
    /// An abort's `session.error` has been seen, so the next idle is not the
    /// close (see [`super::events::translate`]).
    pub(super) aborting: bool,
    /// Running totals: `step-finish` reports one step, not the session.
    pub(super) total_input: u64,
    pub(super) total_output: u64,
    pub(super) total_cost: f64,
    pub(super) context_window: Option<u64>,
    /// The per-thread brief, until the first prompt carries it.
    pub(super) pending_brief: Option<String>,
    /// Set when the user row opens a turn, cleared by the first assistant row.
    pub(super) awaiting_step: Option<Instant>,
}

impl OpencodeServer {
    pub(super) fn route(&self, session: &str, route: SessionRoute) {
        self.routes
            .lock()
            .unwrap()
            .insert(session.to_string(), Arc::new(route));
    }

    /// `POST /session`. The model is `{providerID, id}` here but
    /// `{providerID, modelID}` on a prompt — each a 400 the other way round.
    pub fn start_session(&self, opts: &OpencodeSessionOpts, sink: Sink) -> Result<String, String> {
        let mut body = json!({ "agent": opts.agent });
        if let Some((provider, id)) = opts.model.as_deref().and_then(split_model) {
            let mut m = json!({ "providerID": provider, "id": id });
            // Only a chosen level: a made-up one fails the session.
            if let Some(v) = &opts.variant {
                m["variant"] = json!(v);
            }
            body["model"] = m;
        }
        let r = self.post(&format!("/session?{}", self.directory_query()), body)?;
        let id = r["id"]
            .as_str()
            .ok_or("opencode /session: no session id")?
            .to_string();
        let window = opts.model.as_deref().and_then(|m| self.context_window(m));
        self.route(
            &id,
            SessionRoute {
                sink: sink.clone(),
                agent: opts.agent,
                model: opts.model.clone(),
                variant: opts.variant.clone(),
                state: Mutex::new(SessionState {
                    context_window: window,
                    pending_brief: (!opts.brief.trim().is_empty()).then(|| opts.brief.clone()),
                    ..Default::default()
                }),
            },
        );
        sink(HarnessEvent::SessionStarted {
            provider_session_id: id.clone(),
            model: opts.model.clone(),
            cwd: self.directory.display().to_string(),
        });
        Ok(id)
    }

    /// Take up a session from an earlier run. The brief is already in its
    /// history; the model asked for wins over the stored one.
    pub fn attach_session(
        &self,
        session: &str,
        opts: &OpencodeSessionOpts,
        sink: Sink,
    ) -> Result<(), String> {
        let d = self.get(&format!("/session/{session}?{}", self.directory_query()))?;
        if d["id"].as_str() != Some(session) {
            return Err(format!("opencode has no session {session}"));
        }
        let stored = d["model"]["providerID"]
            .as_str()
            .zip(d["model"]["id"].as_str())
            .map(|(p, i)| format!("{p}/{i}"));
        let model = opts.model.clone().or(stored);
        let window = model.as_deref().and_then(|m| self.context_window(m));
        self.route(
            session,
            SessionRoute {
                sink: sink.clone(),
                agent: opts.agent,
                model: model.clone(),
                variant: opts.variant.clone(),
                state: Mutex::new(SessionState {
                    // Seeded so a reopened thread's usage does not restart.
                    total_input: d["tokens"]["input"].as_u64().unwrap_or(0)
                        + d["tokens"]["cache"]["read"].as_u64().unwrap_or(0)
                        + d["tokens"]["cache"]["write"].as_u64().unwrap_or(0),
                    total_output: d["tokens"]["output"].as_u64().unwrap_or(0)
                        + d["tokens"]["reasoning"].as_u64().unwrap_or(0),
                    total_cost: d["cost"].as_f64().unwrap_or(0.0),
                    context_window: window,
                    ..Default::default()
                }),
            },
        );
        sink(HarnessEvent::SessionStarted {
            provider_session_id: session.to_string(),
            model,
            cwd: self.directory.display().to_string(),
        });
        Ok(())
    }

    /// `POST /session/{id}/prompt_async`, naming agent and model every time
    /// ([`SessionRoute::agent`]). Answers 204 with no body: the turn's anchor
    /// arrives on the stream ([`super::events::translate`]). The harness queue guarantees
    /// one turn in flight.
    pub fn prompt(&self, session: &str, text: &str) -> Result<(), String> {
        let route = self
            .routes
            .lock()
            .unwrap()
            .get(session)
            .cloned()
            .ok_or_else(|| format!("opencode session {session} is not attached"))?;
        let brief = route.state.lock().unwrap().pending_brief.take();
        let text = match brief {
            Some(b) => format!("{}\n\n---\n\n{text}", b.trim()),
            None => text.to_string(),
        };
        let mut body = json!({
            "agent": route.agent,
            "parts": [{ "type": "text", "text": text }],
        });
        if let Some((provider, id)) = route.model.as_deref().and_then(split_model) {
            body["model"] = json!({ "providerID": provider, "modelID": id });
        }
        if let Some(v) = &route.variant {
            body["variant"] = json!(v);
        }
        self.post(
            &format!("/session/{session}/prompt_async?{}", self.directory_query()),
            body,
        )?;
        Ok(())
    }

    /// `POST /session/{id}/abort`; the stream's side is in [`super::events::translate`].
    pub fn interrupt(&self, session: &str) -> Result<(), String> {
        self.post_empty(&format!(
            "/session/{session}/abort?{}",
            self.directory_query()
        ))?;
        Ok(())
    }

    /// `POST /session/{id}/revert {messageID}` drops the named message *and*
    /// everything after it, so the anchor is the question's own id. Lazy: it
    /// applies on the next prompt, and the message list shows the reverted
    /// rows until then. 409 while a turn runs.
    pub fn revert(&self, session: &str, anchor: &str) -> Result<(), String> {
        self.post(
            &format!("/session/{session}/revert?{}", self.directory_query()),
            json!({ "messageID": anchor }),
        )?;
        Ok(())
    }

    /// Delete the session and everything in it, and forget its route.
    pub fn delete_session(&self, session: &str) {
        self.routes.lock().unwrap().remove(session);
        let _ = self.delete(&format!("/session/{session}?{}", self.directory_query()));
    }

    /// Forget a session without touching the server's copy of it.
    pub fn detach(&self, session: &str) {
        self.routes.lock().unwrap().remove(session);
    }

    pub fn has_session(&self, session: &str) -> bool {
        self.routes.lock().unwrap().contains_key(session)
    }
}
