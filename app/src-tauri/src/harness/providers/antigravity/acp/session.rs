//! One `agy acp` process per chat: spawn, open or load its session, send,
//! stop, and its permission requests.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use serde_json::{json, Value};

use crate::harness::child::{self, ChildProc, ThreadSpawn};
use crate::harness::event::{Bound, HarnessEvent, Provider, Turns};
use crate::harness::requests::LiveRoute;
use crate::harness::Sink;

use super::super::models::model_slug;
use super::client::Rpc;
use super::policy::{judge, Verdict};
use super::route::{outcome, AcpRoute, Options};
use super::updates::Translator;

/// A failed load heard no student prompt; the app can replace its binding
/// and retry once with a durable full handoff. Other setup errors fail the send.
#[derive(Debug)]
pub enum OpenError {
    Load(String),
    Setup(String),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load(e) | Self::Setup(e) => f.write_str(e),
        }
    }
}

impl From<String> for OpenError {
    fn from(e: String) -> Self {
        Self::Setup(e)
    }
}
impl From<&str> for OpenError {
    fn from(e: &str) -> Self {
        Self::Setup(e.into())
    }
}

/// What one `agy acp` process is started with.
pub struct AcpSpawn {
    /// `base.cwd` is the chat's folder and write root; `base.library` is
    /// added to the workspace so the agent reads it without asking.
    pub base: ThreadSpawn,
    /// Appended to the agent's system prompt through
    /// `_meta.antigravity.systemPrompt.append`, on a new session and on a
    /// load.
    pub brief: String,
    /// A turn outside any chat: nobody is asked, so a request Rust's policy
    /// does not allow is refused.
    pub one_off: bool,
}

#[derive(Default, Clone)]
pub(super) struct Models {
    /// What the student picked for the next turn.
    pub(super) wanted: Option<String>,
    /// What the agent was last told.
    pub(super) applied: Option<String>,
}

pub(super) struct Callback {
    id: Value,
    cancel: Value,
    pub(super) tool: Option<String>,
}

pub struct AcpSession {
    pub(in crate::harness) rpc: Rpc,
    pub(super) sink: Sink,
    pub(super) session_id: Mutex<String>,
    pub(super) translator: Mutex<Translator>,
    pub(super) turns: Arc<Turns>,
    /// A `session/prompt` is out and its reply has not come. Only then is a
    /// `session/update` the timeline's: a load replays the conversation as
    /// updates, which are history.
    pub(super) prompting: AtomicBool,
    /// Set by Stop or restart steering until the prompt's reply.
    pub(super) interrupting: AtomicBool,
    /// Killed, or its stdout ended: nothing more is answered.
    pub(super) closing: AtomicBool,
    /// Its session was opened, so its end is the chat's to hear.
    pub(super) opened: AtomicBool,
    /// Approvals and questions put to the student, by their id as JSON, with
    /// the id as sent and the generation they were asked under. A stop, a
    /// turn's end and teardown bump the generation and empty the map, so an
    /// answer is written only to a request of the live generation.
    pub(super) callbacks: Mutex<(u64, HashMap<String, Callback>)>,
    pub(super) one_off: bool,
    pub(super) library: PathBuf,
    pub(super) cwd: PathBuf,
    /// Fixed for the process: a change restarts the session.
    pub(super) effort: Option<String>,
    pub(super) models: Mutex<Models>,
    pub(super) me: Weak<AcpSession>,
}

/// The oldest `agy` this bridge was recorded against (stage 5's probe):
/// an older one may not ask before writes, or load a print-mode chat.
pub const MIN_VERSION: &str = "1.5.9";

/// Whether `version` (`1.5.9`) is older than [`MIN_VERSION`]. One that does
/// not read as numbers is too old: nothing says what it speaks.
pub(super) fn too_old(version: &str) -> bool {
    let parts =
        |v: &str| -> Option<Vec<u64>> { v.trim().split('.').map(|p| p.parse().ok()).collect() };
    match (parts(version), parts(MIN_VERSION)) {
        (Some(have), Some(need)) => have < need,
        _ => true,
    }
}

/// `agy`'s argv after the binary. `--sandbox` bounds its shell commands
/// (and lets `oculus` reach keyd's socket); never
/// `--dangerously-skip-permissions`, which would answer every request
/// before Rust sees it. `agy acp` takes no `--model` or `--effort`: they are
/// session settings ([`AcpSession::configure`]).
pub(super) fn launch_args(base: &ThreadSpawn) -> Vec<String> {
    vec![
        "acp".into(),
        "--sandbox".into(),
        "--add-dir".into(),
        base.library.display().to_string(),
    ]
}

impl AcpSession {
    pub(super) fn new(
        proc: ChildProc,
        first_id: i64,
        sink: Sink,
        base: &ThreadSpawn,
        one_off: bool,
    ) -> Arc<Self> {
        let turns = Arc::new(Turns::default());
        Arc::new_cyclic(|me| AcpSession {
            rpc: Rpc::new(proc, first_id),
            sink,
            session_id: Mutex::new(base.resume.clone().unwrap_or_default()),
            translator: Mutex::new(Translator::new(turns.clone())),
            turns,
            prompting: AtomicBool::new(false),
            interrupting: AtomicBool::new(false),
            closing: AtomicBool::new(false),
            opened: AtomicBool::new(false),
            callbacks: Mutex::new((0, HashMap::new())),
            one_off,
            library: base.library.clone(),
            cwd: base.cwd.clone(),
            effort: base.effort.clone(),
            models: Mutex::new(Models {
                wanted: base.model.clone(),
                applied: None,
            }),
            me: me.clone(),
        })
    }

    pub fn spawn(cfg: AcpSpawn, sink: Sink) -> Result<Arc<Self>, OpenError> {
        let base = cfg.base;
        let mut cmd = crate::runtime::platform::cli_command(&base.bin)?;
        cmd.args(launch_args(&base))
            .current_dir(&base.cwd)
            .env_clear()
            .envs(base.env.iter().map(|(k, v)| (k, v)));
        let (proc, stdout) = ChildProc::spawn("agy", &mut cmd, true)?;
        let session = Self::new(proc, 1, sink, &base, cfg.one_off);
        let reader = session.clone();
        let raw_log = base.raw_log.clone();
        std::thread::spawn(move || {
            child::read_json_lines(stdout, raw_log.as_ref(), |v| reader.inbound(&v));
            reader.closed();
        });
        if let Err(e) = session.open(&base, &cfg.brief) {
            session.kill();
            return Err(e);
        }
        Ok(session)
    }

    fn session_id(&self) -> String {
        self.session_id.lock().unwrap().clone()
    }

    /// `initialize`, then a new session or the chat's own loaded, then its
    /// settings. No client file or terminal service is offered (the agent's
    /// own tools do the work, and ask first). Chat offers form elicitation.
    pub(super) fn open(&self, base: &ThreadSpawn, brief: &str) -> Result<(), OpenError> {
        let mut capabilities = json!({
            "fs": { "readTextFile": false, "writeTextFile": false },
            "terminal": false,
        });
        if !self.one_off {
            capabilities["elicitation"] = json!({"form":{}});
        }
        let init = self.rpc.request(
            "initialize",
            json!({
                "protocolVersion": 1,
                "clientCapabilities": capabilities,
                "clientInfo": { "name": "oculus", "title": "Oculus", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        let version = init["agentInfo"]["version"].as_str().unwrap_or("");
        if too_old(version) {
            let have = match version {
                "" => "an unknown version".to_string(),
                v => v.to_string(),
            };
            return Err(format!(
                "Antigravity {MIN_VERSION} or newer is needed, and this is {have}. \
                 Update it in Settings → Agents."
            )
            .into());
        }
        let cwd = base.cwd.display().to_string();
        let mut params = json!({ "cwd": cwd, "mcpServers": [] });
        if !brief.trim().is_empty() {
            params["_meta"] = json!({ "antigravity": { "systemPrompt": { "append": brief } } });
        }
        let id = match &base.resume {
            Some(id) => {
                params["sessionId"] = json!(id);
                self.rpc
                    .request("session/load", params)
                    .map_err(OpenError::Load)?;
                id.clone()
            }
            None => self.rpc.request("session/new", params)?["sessionId"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("agy's session/new named no session")?
                .to_string(),
        };
        *self.session_id.lock().unwrap() = id.clone();
        self.configure()?;
        self.opened.store(true, Ordering::SeqCst);
        (self.sink)(HarnessEvent::SessionStarted {
            provider_session_id: id,
            model: base.model.clone(),
            cwd,
        });
        Ok(())
    }

    /// The session's model, level and mode. A load resets all three to
    /// agy's defaults, so this follows every new session and every load.
    /// The model and level go as one slug (`gemini-3.8-flash-low`); mode
    /// `default` asks before every write and command, which is what puts
    /// each one in front of Rust.
    pub(super) fn configure(&self) -> Result<(), String> {
        let id = self.session_id();
        let wanted = self.models.lock().unwrap().wanted.clone();
        match (&wanted, &self.effort) {
            (Some(model), effort) => {
                let slug = model_slug(model, effort.as_deref());
                self.rpc.request(
                    "session/set_model",
                    json!({ "sessionId": id, "modelId": slug }),
                )?;
            }
            (None, Some(effort)) => {
                self.rpc.request(
                    "session/set_config_option",
                    json!({ "sessionId": id, "configId": "thought_level", "value": effort }),
                )?;
            }
            (None, None) => {}
        }
        self.models.lock().unwrap().applied = wanted;
        self.rpc.request(
            "session/set_mode",
            json!({ "sessionId": id, "modeId": "default" }),
        )?;
        Ok(())
    }

    pub fn is_alive(&self) -> bool {
        !self.closing.load(Ordering::SeqCst) && self.rpc.proc.is_alive()
    }

    /// The model the next turn goes to; told to the agent with that turn.
    pub fn want_model(&self, model: Option<String>) {
        self.models.lock().unwrap().wanted = model;
    }

    /// One user turn. `bound` hears the turn's ordinal before `agy` hears
    /// the message; the prompt's reply ends the turn. Never two prompts at
    /// once: a second one sent mid-tool steers the turn, and the first
    /// reply then says `end_turn` while its tool still runs.
    pub fn send(&self, text: &str, bound: &Bound) -> Result<(), String> {
        if self.prompting.load(Ordering::SeqCst) {
            return Err("Antigravity is still answering the last message".into());
        }
        let models = self.models.lock().unwrap().clone();
        if let Some(model) = models.wanted.filter(|m| models.applied.as_ref() != Some(m)) {
            let slug = model_slug(&model, self.effort.as_deref());
            self.rpc.request(
                "session/set_model",
                json!({ "sessionId": self.session_id(), "modelId": slug }),
            )?;
            self.models.lock().unwrap().applied = Some(model);
        }
        bound(self.turns.reserve());
        self.interrupting.store(false, Ordering::SeqCst);
        self.prompting.store(true, Ordering::SeqCst);
        let me = self.me.clone();
        let params = json!({
            "sessionId": self.session_id(),
            "prompt": [{ "type": "text", "text": text }],
        });
        self.rpc
            .request_then(
                "session/prompt",
                params,
                Box::new(move |reply| {
                    if let Some(session) = me.upgrade() {
                        session.prompt_ended(reply);
                    }
                }),
            )
            .inspect_err(|_| {
                self.prompting.store(false, Ordering::SeqCst);
                self.turns.take_sent();
            })
    }

    fn emit(&self, events: Vec<HarnessEvent>) {
        for ev in events {
            (self.sink)(ev);
        }
    }

    fn prompt_ended(&self, reply: Result<Value, String>) {
        // The process's end already failed the turn.
        if !self.prompting.swap(false, Ordering::SeqCst) {
            return;
        }
        self.interrupting.store(false, Ordering::SeqCst);
        self.end_callbacks();
        let events = self.translator.lock().unwrap().finish(reply);
        self.emit(events);
    }

    /// Every callback still put to the student, taken out under
    /// a new generation: an answer to one of them is no longer admitted.
    fn end_callbacks(&self) -> Vec<Callback> {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks.0 += 1;
        callbacks.1.drain().map(|(_, callback)| callback).collect()
    }

    pub(super) fn register_callback(&self, id: &Value, cancel: Value, tool: Option<String>) -> u64 {
        let mut callbacks = self.callbacks.lock().unwrap();
        callbacks.1.insert(
            id.to_string(),
            Callback {
                id: id.clone(),
                cancel,
                tool,
            },
        );
        callbacks.0
    }

    /// One line from the agent.
    pub(in crate::harness) fn inbound(&self, v: &Value) {
        if self.rpc.resolve(v) {
            return;
        }
        let method = v["method"].as_str().unwrap_or("");
        match (method, v.get("id")) {
            ("session/update", None) => {
                if !self.prompting.load(Ordering::SeqCst) {
                    return;
                }
                let update = &v["params"]["update"];
                if update["sessionUpdate"] == "tool_call_update"
                    && matches!(update["status"].as_str(), Some("completed" | "failed"))
                {
                    if let Some(tool) = update["toolCallId"].as_str() {
                        self.retire_question(tool);
                    }
                }
                let events = self
                    .translator
                    .lock()
                    .unwrap()
                    .update(&v["params"]["update"]);
                self.emit(events);
            }
            ("session/request_permission", Some(id)) => self.permission(id, &v["params"]),
            ("elicitation/create", Some(id)) => self.question(id, &v["params"]),
            (method, Some(id)) => {
                elog!("[oculus] agy: refused its {method} request");
                let message = format!("Oculus does not answer {method}");
                if let Err(e) = self.rpc.refuse(id, -32601, &message) {
                    elog!("[oculus] agy: refusing {method}: {e}");
                }
            }
            _ => {}
        }
    }

    /// A `session/request_permission`: Rust's policy first
    /// ([`judge`]), then the student for what it leaves, unless nobody can
    /// be asked (a one-off turn, no prompt running, a stop on its way).
    fn permission(&self, id: &Value, params: &Value) {
        let options = Options::of(params);
        let reject = options.reject.clone();
        let tool = params["toolCall"]["name"].as_str().unwrap_or("a tool");
        let refuse = |why: &str| {
            elog!("[oculus] agy: declined a {tool} request without asking: {why}");
            if let Err(e) = self.rpc.answer(id, outcome(reject.as_deref())) {
                elog!("[oculus] agy: declining a request: {e}");
            }
        };
        let running = self.prompting.load(Ordering::SeqCst)
            && !self.interrupting.load(Ordering::SeqCst)
            && !self.closing.load(Ordering::SeqCst);
        if !running {
            return refuse("no turn is running");
        }
        let Some(allow) = options.allow.clone() else {
            return refuse("it offers no Allow");
        };
        match judge(params, &self.library, &self.cwd) {
            Verdict::Allow => {
                if let Err(e) = self.rpc.answer(id, outcome(Some(&allow))) {
                    elog!("[oculus] agy: allowing a request: {e}");
                }
            }
            Verdict::Deny(why) => refuse(&why),
            Verdict::Ask(_) if self.one_off => refuse("a one-off turn asks nobody"),
            Verdict::Ask(mut request) => {
                let wire = id.to_string();
                request.native_ref = wire.clone();
                let generation = self.register_callback(id, outcome(None), None);
                let route = AcpRoute {
                    session: self.me.clone(),
                    id: id.clone(),
                    wire,
                    generation,
                    options,
                };
                (self.sink)(HarnessEvent::RequestAsked {
                    request,
                    route: Some(LiveRoute(Arc::new(route))),
                });
            }
        }
    }

    /// Stop the turn: `session/cancel`, then, as ACP asks of a client, every
    /// permission request still open is answered `cancelled`; questions are
    /// declined. The prompt's
    /// reply (`cancelled`) ends the turn; the process lives on.
    pub fn interrupt(&self) -> Result<(), String> {
        if !self.is_alive() || !self.prompting.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.interrupting.store(true, Ordering::SeqCst);
        self.rpc
            .notify("session/cancel", json!({ "sessionId": self.session_id() }))?;
        for callback in self.end_callbacks() {
            if let Err(e) = self.rpc.answer(&callback.id, callback.cancel) {
                elog!("[oculus] agy: cancelling a request: {e}");
            }
        }
        Ok(())
    }

    /// The agent is waiting on a request nobody has answered.
    pub fn has_pending_request(&self) -> bool {
        !self.callbacks.lock().unwrap().1.is_empty()
    }

    /// Refuses: ACP has no way back to an earlier message, and the manager
    /// deletes rows on an `Ok`.
    pub fn rewind(&self, _anchor: &str) -> Result<(), String> {
        Err(
            "Antigravity cannot take a question back out of a conversation — \
             edit it in a new thread instead"
                .into(),
        )
    }

    pub fn kill(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.end_callbacks();
        self.rpc.proc.kill();
    }

    /// stdout reached EOF: keep a cut-off reply, fail the turn it died in,
    /// announce the exit. A process that died before its session opened
    /// has nothing to announce: its spawn fails instead.
    pub(super) fn closed(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.end_callbacks();
        let prompting = self.prompting.swap(false, Ordering::SeqCst);
        let (events, turn) = self.translator.lock().unwrap().close();
        self.rpc.fail_all("agy exited");
        if !self.opened.load(Ordering::SeqCst) {
            self.rpc.proc.reap();
            return;
        }
        self.emit(events);
        self.rpc.proc.finish(&self.sink, Provider::Antigravity, || {
            prompting
                .then(|| turn.or_else(|| self.turns.take_sent()))
                .flatten()
        });
    }
}

#[cfg(test)]
impl AcpSession {
    /// A reply to one of our requests, as the replay's transport gives it.
    pub(super) fn rpc_reply(&self, frame: Value) {
        self.rpc.resolve(&frame);
    }

    /// A replayed session starts open, as a recorded one was.
    pub(super) fn opened_for_replay(&self) {
        self.opened.store(true, Ordering::SeqCst);
    }
}
