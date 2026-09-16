//! The opencode bridge: HTTP and SSE against one `opencode serve`.
//!
//! Codex's shape, over a different wire. One server per app — started on
//! first use, killed on quit — and one *session* per thread inside it, with
//! a single server-wide event stream that the reader thread routes by
//! session id. A process per thread would buy nothing: the protocol already
//! carries the session on every event.
//!
//! Everything below was measured against opencode 1.18.2; the long form is
//! in the recon note this was written from. The parts that are easy to get
//! wrong, and were:
//!
//! - **`--port 0` does not mean "pick a free port".** It means "prefer
//!   4096", and it takes something else only when 4096 is busy. The real
//!   port is printed on **stdout** as `opencode server listening on
//!   http://127.0.0.1:<port>`, and that line is the only place it appears —
//!   `--print-logs` puts structured logs on stderr and never the port.
//! - **An instance bootstraps lazily and asynchronously.** The first request
//!   naming a directory returns *before* that directory's `opencode.json`
//!   has been read: `GET /api/agent` answers `{"data":[]}` and the model
//!   list is missing whatever the project declares. So the bridge asks for
//!   the agent list until `oculus` is in it before creating any session.
//! - **`session.idle` never fires.** Not on the happy path, not on a
//!   provider error, not on an interrupt, on either stream. A turn opens on
//!   `session.next.prompted` and closes on the first
//!   `session.next.step.ended` whose `finish` is not `tool-calls`, or on any
//!   `session.next.step.failed`. A turn is a *chain* of steps and a tool
//!   failure does not end it.
//! - **An interrupt is a `step.failed`**, with `error.message` exactly
//!   `Provider turn interrupted` — not an error the timeline should paint
//!   red. The half-written answer arrives first as a normal
//!   `text.ended`, so unlike Codex nothing has to be reconstructed.
//! - **The session id is at `data.sessionID`.** `properties.sessionID` is
//!   the *legacy* `GET /event` envelope; there are two streams with two
//!   shapes. Thirty of the eighty-eight event types carry no session at all
//!   (pty, workspace, tui, lsp, mcp, installation…), so they are translated
//!   and dispatched before the route lookup, into a sink that belongs to the
//!   harness — exactly what Codex does with its account-scoped rate limits.
//! - **Revert anchors on the message that *survives*.** Everything strictly
//!   after it is deleted, so rewinding "to this question" means staging on
//!   the message *before* it, which is looked up in `/context`.
//!
//! ## Containment, and what it does not buy
//!
//! The other two bridges lean on an OS sandbox: Claude's seatbelt profile
//! and Codex's `workspace-write` both make the cwd the only writable root
//! *at the kernel*, so a shell redirect out of it is refused by the system
//! rather than by the agent. **opencode has no sandbox.** Its permission
//! system is a rule list the runner checks before it calls a tool, and for
//! `bash` the rule is a glob over the command string. So:
//!
//! - `edit` (which governs `write`, `edit` and `apply_patch` together — there
//!   is no separate `write` key) is genuinely enforced per path, and that is
//!   what keeps `courses/`, `lectures/`, `canvas-session/` and `oculus.db`
//!   safe from the file tools. Measured, all three refuse.
//! - `bash` is allow-listed to `oculus …` and `ls …` and denied otherwise,
//!   which stops the obvious `rm -rf ../courses`. It is a speed bump, not a
//!   boundary: `oculus files X > ../oculus.db` matches the allowed prefix and
//!   opencode has nothing underneath it to refuse the redirect. This is the
//!   one place an opencode thread is weaker than a Claude or Codex one, and
//!   it is written down here rather than discovered later.
//!
//! Three rule shapes were measured and only one works, which is worth not
//! relearning (`templates/OPENCODE.template.json` is the result):
//!
//! - `{"*": "deny", "<cwd>/**": "allow"}` **denies everything**. The path
//!   check is deny-wins regardless of order, exactly as in Claude's rule
//!   syntax — so the siblings of `agents/` are named individually and the
//!   root is left allowed, which is the same shape `claude.rs` arrived at
//!   for the same reason.
//! - Whichever rule is **last** decides whether the tool is offered to the
//!   model at all. A trailing `"*": "deny"` does not just refuse calls, it
//!   deletes the tool from the request — `Unknown tool: write`. So the bash
//!   map's last entry has to be an allow, or bash disappears.
//! - A pattern for a path **inside** the session directory has to be
//!   *relative* to it; one for a path outside has to be absolute. An
//!   absolute `<agents>/opencode.json` silently matched nothing, while a
//!   bare `opencode.json` refused it. That last rule is what stops the agent
//!   rewriting the permissions it runs under.
//!
//! The prompt is the other difference. There is no `--append-system-prompt`
//! here: an agent's `prompt` **replaces** the whole system prompt (opencode
//! then appends its own `<env>` block, the date, `AGENTS.md` and a skills
//! index). So the rendered `HARNESS.template.md` *is* the `oculus` agent's
//! prompt, written into `agents/opencode.json`, and the per-thread part of
//! the brief — the subject, the lecture — rides the first prompt of the
//! session instead, since the config is one document for every thread.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::event::{cap_output, classify, HarnessEvent};
use super::{RawLog, Sink};

/// The server answers in well under a second cold; past this it is wedged.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// From `exec` to the listening line was 0.5s measured, warm.
const READY_TIMEOUT: Duration = Duration::from_secs(20);
/// How long to keep asking for the agent list while the instance boots.
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(20);
/// How long a prompt that has been promoted into the agent loop may go
/// without a `step.started` before the bridge calls it dead. Measured, that
/// step arrives within about a tenth of a second — this is not a slow-model
/// budget, it is the gap [`SessionState::awaiting_step`] describes.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// The agent id the sessions run as, defined in the rendered config.
pub const AGENT: &str = "oculus";
/// The throwaway agent a naming turn runs as: the same permissions, a
/// one-line prompt, and hidden so it never shows up in a picker.
pub const NAMING_AGENT: &str = "oculus-namer";

const CONFIG_TEMPLATE: &str = include_str!("../../templates/OPENCODE.template.json");
pub const CONFIG_NAME: &str = "opencode.json";

pub struct OpencodeSpawn {
    pub bin: PathBuf,
    /// The library's `agents/` folder. It is the session directory, the
    /// project root opencode reads `opencode.json` and `AGENTS.md` from, and
    /// the only place the agent may write.
    pub directory: PathBuf,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
    /// Takes the events that name no session — thirty of the eighty-eight
    /// types — the way Codex's account sink takes its rate limits.
    pub default_sink: Option<Sink>,
}

/// How to open a session. The model is opencode's own spelling,
/// `providerID/id`, and the variant is a reasoning level the *model*
/// declared — there is no fixed vocabulary here, and in 1.18.2 every model
/// declares none, so this is almost always `None`.
#[derive(Default, Clone)]
pub struct OpencodeSessionOpts {
    pub model: Option<String>,
    pub variant: Option<String>,
    /// The scope and lecture sections of the thread's brief. The library-wide
    /// part is the agent's `prompt` in the config; this is the per-thread
    /// part, and since the config cannot carry it, it goes out ahead of the
    /// session's first message.
    pub brief: String,
    /// `oculus` for a conversation, `oculus-namer` for a naming turn.
    pub agent: &'static str,
}

/// One row of the catalogue, in the shape `app/src/lib/harness.ts`'s
/// `OpencodeModel` reads.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// `providerID/id`, the spelling `opencode models` prints and the API
    /// takes back. Free-form: an id can itself contain a slash, so it is
    /// split on the **first** one and nowhere else.
    pub id: String,
    pub display_name: String,
    pub description: String,
    /// The model's own reasoning levels. Empty for every model in 1.18.2,
    /// which is why the picker draws no level row for opencode — and why it
    /// will draw one with no code change if a later version fills them in.
    pub variants: Vec<String>,
    pub default_variant: Option<String>,
    pub is_default: bool,
}

struct SessionRoute {
    sink: Sink,
    state: Mutex<SessionState>,
}

#[derive(Default)]
struct SessionState {
    /// A turn of ours is open. `TurnFinished` fires exactly once per turn
    /// because every path that closes one goes through [`close_turn`], and
    /// that returns nothing when this is already false.
    turn_open: bool,
    /// Assistant text by `textID`, accumulated from the deltas. `text.ended`
    /// carries the whole thing and clears it; what is left when a turn ends
    /// is a part that never ended, which is committed rather than lost.
    text: HashMap<String, String>,
    reasoning: HashMap<String, String>,
    /// `callID` → the tool's name, learned from `tool.input.started` or
    /// `tool.called`, so a result for a call we never saw open can still
    /// synthesise one.
    tools: HashMap<String, String>,
    open_tools: std::collections::HashSet<String>,
    /// Running totals: `step.ended` reports one step, not the session.
    total_input: u64,
    total_output: u64,
    total_cost: f64,
    context_window: Option<u64>,
    /// The per-thread brief, until the first prompt carries it.
    pending_brief: Option<String>,
    /// Set when a prompt is promoted into the agent loop and cleared by the
    /// `step.started` that answers it.
    ///
    /// **A turn whose model cannot be resolved emits no event at all.**
    /// opencode logs `Failed to drain Session: ModelUnavailableError` to its
    /// own file and sends nothing — no `step.failed`, no `session.error`, no
    /// `session.idle` — so the turn would stay open for ever and the thread
    /// behind it would never take another message. Measured with a model id
    /// that does not exist; `POST /api/session/{id}/wait`, which the schema
    /// offers as the belt-and-braces answer, returned **503 immediately** in
    /// the same situation and is no use. So this is the watchdog, and it is
    /// narrow on purpose: it fires on the gap between `prompted` and the
    /// first `step.started`, never on a model that is merely thinking.
    awaiting_step: Option<Instant>,
}

pub struct OpencodeServer {
    child: Mutex<Child>,
    base: String,
    directory: PathBuf,
    /// Ordinary calls, with a timeout. The event stream gets its own agent
    /// with none: a read timeout would cut a healthy idle stream.
    api: ureq::Agent,
    stream: ureq::Agent,
    routes: Mutex<HashMap<String, Arc<SessionRoute>>>,
    default_sink: Option<Sink>,
    raw_log: Option<RawLog>,
    alive: Arc<AtomicBool>,
    stderr_tail: Arc<Mutex<Vec<String>>>,
}

impl OpencodeServer {
    pub fn spawn(cfg: OpencodeSpawn) -> Result<Arc<Self>, String> {
        let mut child = Command::new(&cfg.bin)
            .arg("serve")
            .args(["--port", "0"])
            .args(["--hostname", "127.0.0.1"])
            .arg("--print-logs")
            .env_clear()
            .envs(cfg.env.iter().map(|(k, v)| (k, v)))
            // Neither belongs in a server the app owns: an autoupdate would
            // swap the binary under a running thread, and sharing would put
            // a student's coursework conversation on the web.
            .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
            .env("OPENCODE_DISABLE_SHARE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", cfg.bin.display()))?;
        let stdout = child.stdout.take().ok_or("no stdout on opencode child")?;
        let stderr = child.stderr.take().ok_or("no stderr on opencode child")?;

        // stderr is the structured log — every plugin loading, every config
        // file read. Keep a tail for the exit message and drop the rest.
        let stderr_tail: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        {
            let tail = stderr_tail.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let mut t = tail.lock().unwrap();
                    if t.len() >= 20 {
                        t.remove(0);
                    }
                    t.push(line);
                }
            });
        }

        // The port comes off stdout, and the same thread then owns stdout
        // for the life of the process — its EOF is how the bridge learns the
        // server is gone. It parks on `srv_rx` until the server exists to be
        // told.
        let (port_tx, port_rx) = mpsc::channel::<Result<u16, String>>();
        let (srv_tx, srv_rx) = mpsc::channel::<Arc<OpencodeServer>>();
        std::thread::spawn(move || {
            let mut lines = BufReader::new(stdout).lines();
            let mut found = None;
            for line in lines.by_ref().map_while(Result::ok) {
                if let Some(p) = parse_port(&line) {
                    found = Some(p);
                    let _ = port_tx.send(Ok(p));
                    break;
                }
            }
            if found.is_none() {
                let _ = port_tx.send(Err("opencode serve exited before it said which port".into()));
            }
            drop(port_tx);
            let Ok(server) = srv_rx.recv() else { return };
            for _line in lines.map_while(Result::ok) {}
            server.on_exit();
        });

        let port = match port_rx.recv_timeout(READY_TIMEOUT) {
            Ok(r) => r?,
            Err(_) => {
                let _ = child.kill();
                return Err(format!(
                    "opencode serve did not start in {}s",
                    READY_TIMEOUT.as_secs()
                ));
            }
        };

        let server = Arc::new(OpencodeServer {
            child: Mutex::new(child),
            base: format!("http://127.0.0.1:{port}"),
            directory: cfg.directory.clone(),
            api: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout(REQUEST_TIMEOUT)
                .build(),
            // No timeout: this one holds the event stream open for hours.
            stream: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .build(),
            routes: Mutex::new(HashMap::new()),
            default_sink: cfg.default_sink,
            raw_log: cfg.raw_log,
            alive: Arc::new(AtomicBool::new(true)),
            stderr_tail,
        });
        let _ = srv_tx.send(server.clone());

        server.get("/api/health")?;
        server.await_bootstrap()?;
        {
            let s = server.clone();
            std::thread::spawn(move || s.read_events());
        }
        {
            let s = server.clone();
            std::thread::spawn(move || s.watch_drains());
        }
        Ok(server)
    }

    /// The first call naming a directory returns before that directory's
    /// config has been read, so the agent this app defines is simply not
    /// there yet. Ask until it is; a session created in the gap would run as
    /// a built-in agent with none of the containment.
    fn await_bootstrap(&self) -> Result<(), String> {
        let deadline = Instant::now() + BOOTSTRAP_TIMEOUT;
        let mut last;
        loop {
            match self.get(&format!("/api/agent?{}", self.location_query())) {
                Ok(v) => {
                    let found = v["data"]
                        .as_array()
                        .is_some_and(|a| a.iter().any(|x| x["id"].as_str() == Some(AGENT)));
                    if found {
                        return Ok(());
                    }
                    last = format!("`{AGENT}` is not in the agent list yet");
                }
                Err(e) => last = e,
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "opencode did not load {}: {last}",
                    self.directory.join(CONFIG_NAME).display()
                ));
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }

    fn location_query(&self) -> String {
        format!(
            "location%5Bdirectory%5D={}",
            urlencode(&self.directory.display().to_string())
        )
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    // ── HTTP ─────────────────────────────────────────────────────────────

    fn get(&self, path: &str) -> Result<Value, String> {
        self.finish(self.api.get(&format!("{}{path}", self.base)).call(), path)
    }

    /// `send_string` rather than `send_json`: ureq's `json` feature is not
    /// on in this crate, and a bridge is not a reason to turn one on.
    fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        self.finish(
            self.api
                .post(&format!("{}{path}", self.base))
                .set("Content-Type", "application/json")
                .send_string(&body.to_string()),
            path,
        )
    }

    fn delete(&self, path: &str) -> Result<Value, String> {
        self.finish(self.api.delete(&format!("{}{path}", self.base)).call(), path)
    }

    /// One place for "204 is success, a body may be empty, and an error body
    /// is opencode's `{_tag, message}` envelope". Anything that goes into an
    /// error string is scrubbed first: `GET /api/model` echoes provider API
    /// keys, and an error message is exactly the sort of thing that ends up
    /// in a log or a timeline row.
    fn finish(&self, r: Result<ureq::Response, ureq::Error>, path: &str) -> Result<Value, String> {
        match r {
            Ok(resp) => {
                let body = resp.into_string().unwrap_or_default();
                if body.trim().is_empty() {
                    return Ok(Value::Null);
                }
                serde_json::from_str(&body)
                    .map_err(|e| format!("opencode {path}: unreadable answer ({e})"))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp.into_string().unwrap_or_default();
                let msg = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| v["message"].as_str().map(String::from))
                    .unwrap_or_else(|| body.chars().take(400).collect());
                Err(format!("opencode {path}: HTTP {code} {}", scrub(&msg)))
            }
            Err(e) => Err(format!("opencode {path}: {}", scrub(&e.to_string()))),
        }
    }

    // ── Models ───────────────────────────────────────────────────────────

    /// The catalogue for the session directory, so a provider the project's
    /// own config declares is in it.
    ///
    /// Deprecated and disabled rows are dropped, which is what
    /// `opencode models` prints; the API returns everything. The response
    /// itself is never logged and never quoted into an error —
    /// `request.body.apiKey` on each row is the provider's real key.
    pub fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        let v = self.get(&format!("/api/model?{}", self.location_query()))?;
        let data = v["data"].as_array().ok_or("opencode /api/model: no data")?;
        let mut out = Vec::new();
        for m in data {
            let id = m["id"].as_str().unwrap_or("");
            let provider = m["providerID"].as_str().unwrap_or("");
            if id.is_empty() || provider.is_empty() {
                continue;
            }
            if m["enabled"].as_bool() == Some(false) {
                continue;
            }
            if m["status"].as_str() == Some("deprecated") {
                continue;
            }
            let variants: Vec<String> = m["variants"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x["id"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let name = m["name"].as_str().unwrap_or(id);
            out.push(ModelInfo {
                id: format!("{provider}/{id}"),
                display_name: name.to_string(),
                description: describe(m),
                default_variant: variants.first().cloned(),
                variants,
                is_default: false,
            });
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// The context window of one `providerID/id`, for the usage ring.
    fn context_window(&self, model: &str) -> Option<u64> {
        let (provider, id) = split_model(model)?;
        let v = self.get(&format!("/api/model?{}", self.location_query())).ok()?;
        v["data"].as_array()?.iter().find_map(|m| {
            (m["providerID"].as_str() == Some(provider.as_str()) && m["id"].as_str() == Some(id.as_str()))
                .then(|| m["limit"]["context"].as_u64())
                .flatten()
        })
    }

    // ── Sessions ─────────────────────────────────────────────────────────

    fn route(&self, session: &str, sink: Sink, state: SessionState) {
        self.routes.lock().unwrap().insert(
            session.to_string(),
            Arc::new(SessionRoute {
                sink,
                state: Mutex::new(state),
            }),
        );
    }

    pub fn start_session(&self, opts: &OpencodeSessionOpts, sink: Sink) -> Result<String, String> {
        let mut body = json!({
            "agent": opts.agent,
            "location": { "directory": self.directory },
        });
        if let Some((provider, id)) = opts.model.as_deref().and_then(split_model) {
            let mut m = json!({ "providerID": provider, "id": id });
            // Sent only when a level was actually chosen. opencode declares
            // no variants for any model in 1.18.2, so in practice this is
            // never set — and a made-up one would fail the session, not the
            // turn.
            if let Some(v) = &opts.variant {
                m["variant"] = json!(v);
            }
            body["model"] = m;
        }
        let r = self.post("/api/session", body)?;
        let id = r["data"]["id"]
            .as_str()
            .ok_or("opencode /api/session: no session id")?
            .to_string();
        let window = opts.model.as_deref().and_then(|m| self.context_window(m));
        self.route(
            &id,
            sink.clone(),
            SessionState {
                context_window: window,
                pending_brief: (!opts.brief.trim().is_empty()).then(|| opts.brief.clone()),
                ..Default::default()
            },
        );
        sink(HarnessEvent::SessionStarted {
            provider_session_id: id.clone(),
            model: opts.model.clone(),
            cwd: self.directory.display().to_string(),
        });
        Ok(id)
    }

    /// Take up a session this app created in an earlier run. The brief is
    /// not repeated: it is already in that session's own history, which the
    /// server kept.
    pub fn attach_session(
        &self,
        session: &str,
        opts: &OpencodeSessionOpts,
        sink: Sink,
    ) -> Result<(), String> {
        let r = self.get(&format!("/api/session/{session}"))?;
        let d = &r["data"];
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
            sink.clone(),
            SessionState {
                // Seeded from the server's own totals, so a thread reopened
                // tomorrow does not report its usage as having started over.
                total_input: d["tokens"]["input"].as_u64().unwrap_or(0)
                    + d["tokens"]["cache"]["read"].as_u64().unwrap_or(0)
                    + d["tokens"]["cache"]["write"].as_u64().unwrap_or(0),
                total_output: d["tokens"]["output"].as_u64().unwrap_or(0)
                    + d["tokens"]["reasoning"].as_u64().unwrap_or(0),
                total_cost: d["cost"].as_f64().unwrap_or(0.0),
                context_window: window,
                ..Default::default()
            },
        );
        sink(HarnessEvent::SessionStarted {
            provider_session_id: session.to_string(),
            model,
            cwd: self.directory.display().to_string(),
        });
        Ok(())
    }

    /// Put one turn in. The queue upstream is what guarantees there is only
    /// ever one — `delivery: "steer"` would have the server take a second,
    /// and the harness owns the `Queued`/`Unqueued` rows and the
    /// one-turn-at-a-time rule, which cannot be half-owned by a server.
    pub fn prompt(&self, session: &str, text: &str) -> Result<(), String> {
        let route = self.routes.lock().unwrap().get(session).cloned();
        let brief = route
            .as_ref()
            .and_then(|r| r.state.lock().unwrap().pending_brief.take());
        let body = match brief {
            Some(b) => format!("{}\n\n---\n\n{text}", b.trim()),
            None => text.to_string(),
        };
        let r = self.post(
            &format!("/api/session/{session}/prompt"),
            json!({ "prompt": { "text": body }, "delivery": "steer" }),
        )?;
        // The `msg_…` a revert anchors on comes back here, synchronously. It
        // also arrives later on `session.next.prompted`, but that can be a
        // whole turn away when something is already running, and the row
        // that has to carry it is being written now.
        if let (Some(id), Some(route)) = (r["data"]["id"].as_str(), route) {
            (route.sink)(HarnessEvent::TurnAnchor { anchor: id.into() });
        }
        Ok(())
    }

    pub fn interrupt(&self, session: &str) -> Result<(), String> {
        self.post(&format!("/api/session/{session}/interrupt"), json!({}))?;
        Ok(())
    }

    /// Drop a question and everything after it from the session's own
    /// history, so the agent's context matches the thread being read.
    ///
    /// Two things make this more than one call. opencode's anchor is the
    /// message that **survives** — everything strictly after it goes — so
    /// rewinding *to* a question means staging on the message before it, and
    /// that predecessor is looked up in `/context` rather than remembered,
    /// since nothing hands it back. And `files: false` is load-bearing: this
    /// rewind is conversational, and `files: true` rolls the working tree
    /// back at stage time. Measured in a directory that is not a git repo —
    /// which `agents/` is not — the conversational half works exactly as it
    /// does in one, returning no snapshot and touching nothing on disk.
    pub fn revert(&self, session: &str, anchor: &str) -> Result<(), String> {
        let ctx = self.get(&format!("/api/session/{session}/context"))?;
        let ids: Vec<&str> = ctx["data"]
            .as_array()
            .map(|a| a.iter().filter_map(|m| m["id"].as_str()).collect())
            .unwrap_or_default();
        let before = predecessor(&ids, anchor)?;
        self.post(
            &format!("/api/session/{session}/revert/stage"),
            json!({ "messageID": before, "files": false }),
        )?;
        self.post(&format!("/api/session/{session}/revert/commit"), json!({}))?;
        Ok(())
    }

    /// Delete the session and everything in it. Only the legacy path exists;
    /// there is no `/api` equivalent.
    pub fn delete_session(&self, session: &str) {
        self.routes.lock().unwrap().remove(session);
        let _ = self.delete(&format!("/session/{session}"));
    }

    /// Forget a session without touching the server's copy of it.
    pub fn detach(&self, session: &str) {
        self.routes.lock().unwrap().remove(session);
    }

    pub fn has_session(&self, session: &str) -> bool {
        self.routes.lock().unwrap().contains_key(session)
    }

    pub fn kill(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
    }

    // ── Inbound ──────────────────────────────────────────────────────────

    /// One connection to the server-wide stream for the whole app.
    ///
    /// The per-session stream cannot replace it: that one is durable-only
    /// and carries no `.delta` events at all, so a thread watched through it
    /// would show nothing until each block finished.
    ///
    /// A dropped stream fails whatever turns were open before reconnecting.
    /// The gap may have swallowed the `step.ended` that would have closed
    /// them, and a turn that never closes strands its thread for ever
    /// (`Queue` upstream releases a thread on `TurnFinished` and nothing
    /// else) — a turn wrongly marked failed is a row the student can see and
    /// retry, which is the better half to be wrong on.
    fn read_events(self: Arc<Self>) {
        let mut backoff = Duration::from_millis(200);
        while self.is_alive() {
            match self.stream.get(&format!("{}/api/event", self.base)).call() {
                Ok(resp) => {
                    backoff = Duration::from_millis(200);
                    for line in BufReader::new(resp.into_reader()).lines().map_while(Result::ok) {
                        let Some(payload) = line.strip_prefix("data: ") else {
                            // `: heartbeat` comment lines, and blank
                            // separators.
                            continue;
                        };
                        if let Some(log) = &self.raw_log {
                            log.write(payload);
                        }
                        let Ok(v) = serde_json::from_str::<Value>(payload) else {
                            continue;
                        };
                        self.dispatch(&v);
                    }
                }
                Err(_) => {
                    backoff = (backoff * 2).min(Duration::from_secs(5));
                }
            }
            if !self.is_alive() {
                break;
            }
            self.fail_open_turns("the opencode event stream dropped mid-turn");
            std::thread::sleep(backoff);
        }
    }

    /// Close turns the server silently never started. See
    /// [`SessionState::awaiting_step`] for what this is standing in for.
    fn watch_drains(self: Arc<Self>) {
        while self.is_alive() {
            std::thread::sleep(Duration::from_secs(2));
            let routes: Vec<Arc<SessionRoute>> =
                self.routes.lock().unwrap().values().cloned().collect();
            for r in routes {
                let events = {
                    let mut st = r.state.lock().unwrap();
                    let stalled = st
                        .awaiting_step
                        .is_some_and(|at| at.elapsed() > DRAIN_TIMEOUT);
                    if !stalled {
                        continue;
                    }
                    st.awaiting_step = None;
                    close_turn(&mut st, "failed")
                };
                if events.is_empty() {
                    continue;
                }
                (r.sink)(HarnessEvent::error(
                    "opencode accepted the message and then never started the turn — \
                     usually a model it cannot resolve. Check the model in the picker, \
                     and that `opencode auth login` covers its provider.",
                ));
                for ev in events {
                    (r.sink)(ev);
                }
            }
        }
    }

    fn dispatch(&self, v: &Value) {
        let Some(ty) = v["type"].as_str() else { return };
        let data = &v["data"];
        // Server-scoped first. Thirty of the event types name no session —
        // and `session.error`'s own id is optional — so the route lookup
        // below would silently eat them, which is the bug Codex's account
        // sink exists to prevent.
        let session = data["sessionID"]
            .as_str()
            .or_else(|| v["durable"]["aggregateID"].as_str());
        let Some(session) = session else {
            if let Some(sink) = &self.default_sink {
                for ev in translate_server(ty, data) {
                    sink(ev);
                }
            }
            return;
        };
        let route = self.routes.lock().unwrap().get(session).cloned();
        // Not ours: the student's own TUI, or a naming session already
        // detached.
        let Some(route) = route else { return };
        let events = {
            let mut st = route.state.lock().unwrap();
            translate(ty, data, &mut st)
        };
        for ev in events {
            (route.sink)(ev);
        }
    }

    /// Close every turn that is still open, with a reason. Used when the
    /// stream drops and when the process dies: both are moments after which
    /// no `step.ended` is ever coming.
    fn fail_open_turns(&self, why: &str) {
        let routes: Vec<Arc<SessionRoute>> = self.routes.lock().unwrap().values().cloned().collect();
        for r in routes {
            let events = {
                let mut st = r.state.lock().unwrap();
                close_turn(&mut st, "failed")
            };
            if events.is_empty() {
                continue;
            }
            (r.sink)(HarnessEvent::error(why));
            for ev in events {
                (r.sink)(ev);
            }
        }
    }

    fn on_exit(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let code = self.child.lock().unwrap().wait().ok().and_then(|s| s.code());
        let tail = self.stderr_tail.lock().unwrap().join("\n");
        self.fail_open_turns(&format!("opencode serve exited (code {code:?})\n{tail}"));
        let routes: Vec<Arc<SessionRoute>> = self.routes.lock().unwrap().drain().map(|(_, r)| r).collect();
        for r in routes {
            (r.sink)(HarnessEvent::Exited { code });
        }
    }
}

impl Drop for OpencodeServer {
    fn drop(&mut self) {
        if let Ok(mut c) = self.child.lock() {
            let _ = c.kill();
        }
    }
}

// ── The config document ──────────────────────────────────────────────────────

/// Write `agents/opencode.json`, the whole of what an opencode session is
/// allowed to do and the only way to give it a system prompt.
///
/// Rewritten on every server start, like Claude's inline `--settings`: the
/// prompt has the library's real paths and course folders in it, and a stale
/// permission list is a stale containment story. Paths and prompts go in
/// through `serde_json`, so a data directory with a quote or a backslash in
/// it cannot break the document.
pub fn write_config(directory: &Path, library: &Path, prompt: &str, naming_prompt: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory).map_err(|e| format!("cannot create {}: {e}", directory.display()))?;
    let path = directory.join(CONFIG_NAME);
    std::fs::write(&path, render_config(library, prompt, naming_prompt))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// Substitution over the template's *text*, and it has to stay that way.
/// The ruleset above is ordered — last rule wins, and a trailing wildcard
/// deny deletes the tool — while a JSON object is unordered by spec and
/// `serde_json`'s default map sorts its keys. Parse this template into a
/// `Value` and re-serialize it and the rules come back alphabetized, which
/// silently inverts the containment without changing a byte of the template
/// or failing to compile. The test below parses the *output* to prove it is
/// valid JSON; nothing in the write path may.
fn render_config(library: &Path, prompt: &str, naming_prompt: &str) -> String {
    CONFIG_TEMPLATE
        .replace("{{LIBRARY}}", &json_fragment(&library.display().to_string()))
        .replace("\"{{PROMPT}}\"", &json_string(prompt))
        .replace("\"{{NAMING_PROMPT}}\"", &json_string(naming_prompt))
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// The same escaping, without the quotes, for a value the template already
/// has quotes around.
fn json_fragment(s: &str) -> String {
    let q = json_string(s);
    q[1..q.len() - 1].to_string()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// The message opencode has to be told to keep so that `anchor` and
/// everything after it goes.
///
/// Its revert names the message that **survives**, so "rewind to this
/// question" is "commit on the one before it". Nothing precedes the first
/// message of a session and there is no revert-to-empty, so that case is a
/// refusal rather than a silent near-miss — the timeline then says the agent
/// still remembers, which is true.
fn predecessor<'a>(ids: &[&'a str], anchor: &str) -> Result<&'a str, String> {
    let at = ids
        .iter()
        .position(|id| *id == anchor)
        .ok_or_else(|| format!("opencode no longer has message {anchor}"))?;
    if at == 0 {
        return Err("opencode cannot revert past the first message of a session".into());
    }
    Ok(ids[at - 1])
}

/// `opencode server listening on http://127.0.0.1:4096` — the only place the
/// port is ever said, and on stdout rather than in the logs.
fn parse_port(line: &str) -> Option<u16> {
    let rest = line.split("listening on").nth(1)?;
    let host = rest.trim().trim_end_matches('/');
    host.rsplit(':').next()?.trim().parse().ok()
}

/// `providerID/id`, split on the **first** slash only: an id can itself
/// contain one (`tss-nvidia-spark/nvidia/Qwen3.6-35B-A3B-NVFP4`).
pub fn split_model(model: &str) -> Option<(String, String)> {
    let (p, id) = model.split_once('/')?;
    (!p.is_empty() && !id.is_empty()).then(|| (p.to_string(), id.to_string()))
}

/// Never put a provider key in a string that might be logged. `/api/model`
/// echoes one per row in `request.body.apiKey`, and an error message is
/// exactly where a response body ends up.
fn scrub(s: &str) -> String {
    let mut out = s.to_string();
    for key in ["apiKey", "api_key", "Authorization", "authorization"] {
        if let Some(at) = out.find(key) {
            out.truncate(at);
            out.push_str("[redacted]");
        }
    }
    out
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// One line for the picker's row. The catalogue has no description field, so
/// it is built from what is there: the family and the context window.
fn describe(m: &Value) -> String {
    let mut bits: Vec<String> = Vec::new();
    if let Some(f) = m["family"].as_str() {
        bits.push(f.to_string());
    }
    if let Some(w) = m["limit"]["context"].as_u64() {
        bits.push(format!("{}K context", w / 1000));
    }
    if m["status"].as_str() == Some("beta") || m["status"].as_str() == Some("alpha") {
        bits.push(m["status"].as_str().unwrap_or("").to_string());
    }
    bits.join(" · ")
}

fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

/// The text of a tool's `content[]`, which is a list of `{type, text}` parts.
fn content_text(v: &Value) -> String {
    let Some(parts) = v["content"].as_array() else {
        return String::new();
    };
    parts
        .iter()
        .filter_map(|p| p["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Translation ──────────────────────────────────────────────────────────────

/// Events that name no session. Thirty of the eighty-eight types are like
/// this — pty, workspace, tui, lsp, mcp, installation, `server.connected` —
/// and none of them is about a conversation. The one that is worth hearing
/// is a `session.error` whose own `sessionID` is missing, which the schema
/// allows: dropping it would leave a failure with nowhere to be read.
fn translate_server(ty: &str, data: &Value) -> Vec<HarnessEvent> {
    if ty == "session.error" {
        let msg = data["error"]["data"]["message"]
            .as_str()
            .or_else(|| data["error"]["name"].as_str())
            .unwrap_or("opencode reported an error with no session");
        return vec![HarnessEvent::error(friendly(msg))];
    }
    Vec::new()
}

/// A turn finishes exactly once, and only here.
///
/// Every path that ends a turn — the step that says `stop`, the step that
/// failed, an interrupt, a dropped stream, a dead server — goes through this
/// function, and it answers with nothing when the turn is already closed. A
/// thread upstream is released for its next message by `TurnFinished` and
/// nothing else, so a second one would send the queued message twice and a
/// missing one would strand it for ever.
fn close_turn(st: &mut SessionState, status: &str) -> Vec<HarnessEvent> {
    if !st.turn_open {
        return Vec::new();
    }
    st.turn_open = false;
    st.awaiting_step = None;
    let mut out = Vec::new();
    // A text part that never got its `ended` — an interrupt usually does
    // send one, but a stream that dropped mid-answer does not, and live text
    // has no row behind it.
    let mut leftover: Vec<String> = st.text.drain().map(|(_, v)| v).collect();
    leftover.sort();
    for text in leftover {
        if !text.trim().is_empty() {
            out.push(HarnessEvent::AssistantMessage { text });
        }
    }
    st.reasoning.clear();
    // Any tool still open when the turn ends never reported a result.
    let open: Vec<String> = st.open_tools.drain().collect();
    for id in open {
        out.push(HarnessEvent::ToolFinished {
            id,
            ok: false,
            output: "the turn ended before this finished".into(),
        });
    }
    out.push(HarnessEvent::TurnFinished {
        status: status.into(),
    });
    out
}

/// opencode's own wording, where it would be read as something it is not.
fn friendly(msg: &str) -> String {
    if msg.contains("free tier can only be used in OpenCode") || msg.contains("MissingSessionID") {
        return "opencode's free Zen models only work inside opencode itself — its gateway \
                refuses the app's requests. Pick a model from a provider you have signed in \
                to with `opencode auth login`."
            .into();
    }
    msg.to_string()
}

fn translate(ty: &str, d: &Value, st: &mut SessionState) -> Vec<HarnessEvent> {
    let mut out = Vec::new();
    match ty {
        // `prompt.admitted` fires with the HTTP POST; `prompted` fires when
        // the input is promoted into the agent loop, which is 1:1 with a
        // turn. With something already running the two are a whole turn
        // apart.
        "session.next.prompted" => {
            st.text.clear();
            st.reasoning.clear();
            st.tools.clear();
            st.open_tools.clear();
            st.turn_open = true;
            st.awaiting_step = Some(Instant::now());
            out.push(HarnessEvent::TurnStarted);
        }
        // Only for the watchdog: the step itself has nothing to draw.
        "session.next.step.started" => {
            st.awaiting_step = None;
        }
        "session.next.text.started" => {
            st.text.insert(s(d, "textID"), String::new());
        }
        "session.next.text.delta" => {
            let text = s(d, "delta");
            if !text.is_empty() {
                st.text.entry(s(d, "textID")).or_default().push_str(&text);
                out.push(HarnessEvent::AssistantDelta { text });
            }
        }
        "session.next.text.ended" => {
            st.text.remove(&s(d, "textID"));
            let text = s(d, "text");
            if !text.trim().is_empty() {
                out.push(HarnessEvent::AssistantMessage { text });
            }
        }
        "session.next.reasoning.started" => {
            st.reasoning.insert(s(d, "reasoningID"), String::new());
        }
        "session.next.reasoning.delta" => {
            let text = s(d, "delta");
            if !text.is_empty() {
                st.reasoning.entry(s(d, "reasoningID")).or_default().push_str(&text);
                out.push(HarnessEvent::ThinkingDelta { text });
            }
        }
        "session.next.reasoning.ended" => {
            st.reasoning.remove(&s(d, "reasoningID"));
            let text = s(d, "text");
            if !text.trim().is_empty() {
                out.push(HarnessEvent::Thinking { text });
            }
        }
        // The name is on `input.started` and the arguments are still
        // streaming as raw JSON; nothing useful can be titled yet, so the row
        // opens on `tool.called`, which is also when the tool actually runs.
        "session.next.tool.input.started" => {
            st.tools.insert(s(d, "callID"), s(d, "name"));
        }
        // Naming trap: the tool's name is `name` above and `tool` here.
        "session.next.tool.called" => {
            let id = s(d, "callID");
            let name = s(d, "tool");
            st.tools.insert(id.clone(), name.clone());
            let input = d.get("input").cloned().unwrap_or(Value::Null);
            let (kind, title) = classify(&name, &input);
            st.open_tools.insert(id.clone());
            out.push(HarnessEvent::ToolStarted {
                id,
                kind,
                name,
                title,
                input,
            });
        }
        "session.next.tool.progress" => {
            let text = content_text(d);
            if !text.is_empty() {
                out.push(HarnessEvent::ToolOutputDelta {
                    id: s(d, "callID"),
                    text,
                });
            }
        }
        "session.next.tool.success" => {
            let id = s(d, "callID");
            ensure_open(&mut out, st, &id);
            st.open_tools.remove(&id);
            let mut output = content_text(d);
            if output.is_empty() {
                // `read` puts the file in `structured`, not in `content`.
                output = d["structured"]["content"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| match &d["structured"] {
                        Value::Null => String::new(),
                        v => v.to_string(),
                    });
            }
            out.push(HarnessEvent::ToolFinished {
                id,
                ok: true,
                output: cap_output(&output),
            });
        }
        // A tool that fails does **not** end the turn: the loop keeps going
        // and a later step closes it.
        "session.next.tool.failed" => {
            let id = s(d, "callID");
            ensure_open(&mut out, st, &id);
            st.open_tools.remove(&id);
            let msg = d["error"]["message"].as_str().unwrap_or("the tool failed");
            out.push(HarnessEvent::ToolFinished {
                id,
                ok: false,
                output: cap_output(msg),
            });
        }
        // A turn is a chain of steps. `tool-calls` means the model asked for
        // a tool and the loop continues; anything else is the end of it.
        "session.next.step.ended" => {
            let t = &d["tokens"];
            let n = |k: &str| t[k].as_u64().unwrap_or(0);
            // Measured: `input` excludes cached reads and `output` excludes
            // reasoning, so they are added rather than assumed included.
            let input = n("input") + t["cache"]["read"].as_u64().unwrap_or(0) + t["cache"]["write"].as_u64().unwrap_or(0);
            let output = n("output") + n("reasoning");
            st.total_input += input;
            st.total_output += output;
            st.total_cost += d["cost"].as_f64().unwrap_or(0.0);
            out.push(HarnessEvent::Usage {
                input_tokens: st.total_input,
                output_tokens: st.total_output,
                context_tokens: Some(input + output),
                context_window: st.context_window,
                cost_usd: (st.total_cost > 0.0).then_some(st.total_cost),
            });
            if d["finish"].as_str() != Some("tool-calls") {
                out.extend(close_turn(st, "completed"));
            }
        }
        "session.next.step.failed" => {
            let msg = d["error"]["message"].as_str().unwrap_or("");
            // An interrupt arrives here and nowhere else, and it is not a
            // failure: this exact string is what the stop button leaves
            // behind.
            if msg == "Provider turn interrupted" {
                out.extend(close_turn(st, "interrupted"));
            } else {
                out.push(HarnessEvent::error(friendly(msg)));
                out.extend(close_turn(st, "failed"));
            }
        }
        // Never observed in five runs, but the schema has it and a failure
        // with nowhere to go is worse than a duplicate row.
        "session.error" => {
            let msg = d["error"]["data"]["message"]
                .as_str()
                .or_else(|| d["error"]["name"].as_str())
                .unwrap_or("opencode error");
            out.push(HarnessEvent::error(friendly(msg)));
            out.extend(close_turn(st, "failed"));
        }
        // prompt.admitted (the POST's own echo), context.updated (the
        // re-rendered AGENTS.md, not context accounting), tool.input.delta
        // (partial JSON), retried, compaction.*, revert.*, agent/model
        // switched, session.idle (which never fires): not surfaced.
        _ => {}
    }
    out
}

/// A result for a call that never opened a row — a reconnect can land in the
/// middle of one. Give the timeline something to close.
fn ensure_open(out: &mut Vec<HarnessEvent>, st: &mut SessionState, id: &str) {
    if st.open_tools.contains(id) {
        return;
    }
    let name = st.tools.get(id).cloned().unwrap_or_else(|| "unknown".into());
    let (kind, title) = classify(&name, &Value::Null);
    st.open_tools.insert(id.to_string());
    out.push(HarnessEvent::ToolStarted {
        id: id.to_string(),
        kind,
        name,
        title,
        input: Value::Null,
    });
}

#[cfg(test)]
mod tests {
    use super::super::event::ToolKind;
    use super::*;

    /// The order the live dispatcher uses: server-scoped events are
    /// translated *before* the session lookup, so a replay that only called
    /// `translate` would go green on a stream the app routes differently.
    /// That is the lesson written at `codex.rs`'s own fixture test.
    fn replay(raw: &str) -> Vec<HarnessEvent> {
        let mut st = SessionState::default();
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(ty) = v["type"].as_str() else { continue };
            let data = &v["data"];
            let session = data["sessionID"]
                .as_str()
                .or_else(|| v["durable"]["aggregateID"].as_str());
            match session {
                None => events.extend(translate_server(ty, data)),
                Some(_) => events.extend(translate(ty, data, &mut st)),
            }
        }
        events
    }

    /// A recorded opencode 1.18.2 turn: reasoning, a tool call, a second
    /// step, an answer. Two steps, and only the second closes the turn.
    #[test]
    fn folds_a_recorded_turn() {
        let events = replay(include_str!("../../fixtures/harness/opencode-ls.ndjson"));
        assert!(matches!(events.first(), Some(HarnessEvent::TurnStarted)));

        let tools: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolStarted { kind, name, title, .. } => {
                    Some((*kind, name.clone(), title.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(tools.len(), 1, "one tool call in the recording");
        assert_eq!(tools[0].0, ToolKind::Read);
        assert_eq!(tools[0].1, "read");
        // Titled off `path`. Claude's arm would have read `file_path` and
        // left this empty, which is the bug this recording exists to catch.
        assert_eq!(tools[0].2, "w1.md");

        assert!(events
            .iter()
            .any(|e| matches!(e, HarnessEvent::ToolFinished { ok: true, .. })));
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Thinking { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, HarnessEvent::Usage { context_tokens: Some(n), .. } if *n > 0)));
        // The first step says `tool-calls`, which is not the end of a turn.
        let finished: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::TurnFinished { status } => Some(status.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(finished, vec!["completed"], "exactly one, at the end");
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })));
        // Spend, not a subscription: no plan windows are invented.
        assert!(!events.iter().any(|e| matches!(e, HarnessEvent::RateLimits { .. })));
    }

    /// A turn stopped mid-answer. `session.idle` never comes; what does is a
    /// `step.failed` carrying opencode's own interrupt string, and the half
    /// written answer arrives before it as an ordinary `text.ended`.
    #[test]
    fn a_stopped_turn_is_not_an_error() {
        let events = replay(include_str!("../../fixtures/harness/opencode-interrupt.ndjson"));
        let deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(!deltas.is_empty(), "the fixture streams an answer");
        let message = events.iter().find_map(|e| match e {
            HarnessEvent::AssistantMessage { text } => Some(text.clone()),
            _ => None,
        });
        assert_eq!(message.as_deref(), Some(deltas.as_str()), "the partial is a row");
        assert!(
            !events.iter().any(|e| matches!(e, HarnessEvent::Error { .. })),
            "stopping a turn is not an error"
        );
        assert!(
            matches!(events.last(), Some(HarnessEvent::TurnFinished { status }) if status == "interrupted")
        );
    }

    /// The invariant the queue upstream depends on: one `TurnFinished` per
    /// turn, whatever arrives afterwards.
    #[test]
    fn a_turn_finishes_exactly_once() {
        let mut st = SessionState::default();
        let go = |st: &mut SessionState, ty: &str, d: Value| translate(ty, &d, st);

        assert!(go(&mut st, "session.next.prompted", json!({})).len() == 1);
        // A step that wanted tools does not end anything.
        let mid = go(&mut st, "session.next.step.ended", json!({"finish": "tool-calls", "tokens": {}}));
        assert!(!mid.iter().any(|e| matches!(e, HarnessEvent::TurnFinished { .. })));
        // A tool that failed does not end anything either.
        let tool = go(&mut st, "session.next.tool.failed", json!({"callID": "c1", "error": {"message": "no"}}));
        assert!(!tool.iter().any(|e| matches!(e, HarnessEvent::TurnFinished { .. })));

        let end = go(&mut st, "session.next.step.ended", json!({"finish": "stop", "tokens": {}}));
        assert_eq!(
            end.iter().filter(|e| matches!(e, HarnessEvent::TurnFinished { .. })).count(),
            1
        );
        // Anything after the close is not a second turn ending.
        for ty in ["session.next.step.ended", "session.next.step.failed", "session.idle"] {
            let late = go(&mut st, ty, json!({"finish": "stop", "tokens": {}, "error": {"message": "x"}}));
            assert!(!late.iter().any(|e| matches!(e, HarnessEvent::TurnFinished { .. })), "{ty}");
        }
    }

    /// An interrupt is a `step.failed` with one particular message, and the
    /// difference between that and a real failure is the whole of what the
    /// timeline draws.
    #[test]
    fn an_interrupt_and_a_provider_error_are_told_apart() {
        for (msg, status, is_error) in [
            ("Provider turn interrupted", "interrupted", false),
            ("Provider request failed with HTTP 401", "failed", true),
        ] {
            let mut st = SessionState::default();
            translate("session.next.prompted", &json!({}), &mut st);
            let out = translate(
                "session.next.step.failed",
                &json!({ "error": { "type": "unknown", "message": msg } }),
                &mut st,
            );
            assert!(matches!(out.last(), Some(HarnessEvent::TurnFinished { status: s }) if s == status));
            assert_eq!(
                out.iter().any(|e| matches!(e, HarnessEvent::Error { .. })),
                is_error,
                "{msg}"
            );
        }
    }

    /// The free Zen models cannot be driven over the HTTP API at all, and a
    /// student picking one out of the list gets a bare HTTP 400 unless this
    /// says what happened.
    #[test]
    fn the_zen_free_tier_gets_a_sentence_rather_than_a_400() {
        let raw = "Provider request failed with HTTP 400: {\"type\":\"error\",\"error\":\
                   {\"type\":\"MissingSessionID\",\"message\":\"Error from provider (Console): \
                   OpenCode's free tier can only be used in OpenCode\"}}";
        let out = friendly(raw);
        assert!(out.contains("opencode auth login"));
        assert!(!out.contains("HTTP 400"));
    }

    /// Revert keeps the message it is given, so the anchor a rewind sends is
    /// the one *before* the question being dropped.
    #[test]
    fn a_rewind_anchors_on_the_message_before_the_question() {
        let ids = ["msg_a", "msg_a_reply", "msg_b", "msg_b_reply", "msg_c"];
        assert_eq!(predecessor(&ids, "msg_c").unwrap(), "msg_b_reply");
        assert_eq!(predecessor(&ids, "msg_b").unwrap(), "msg_a_reply");
        assert!(predecessor(&ids, "msg_a").is_err(), "nothing precedes the first message");
        assert!(predecessor(&ids, "msg_gone").is_err());
    }

    /// The picker reads these names off the wire (`OpencodeModel` in
    /// `app/src/lib/harness.ts`), so a rename on either side is a silently
    /// empty model list rather than a type error.
    #[test]
    fn a_model_row_is_spelled_the_way_the_picker_reads_it() {
        let json = serde_json::to_value(ModelInfo {
            id: "anthropic/claude-opus-4-5".into(),
            display_name: "Claude Opus 4.5 (latest)".into(),
            description: "claude-opus · 200K context".into(),
            variants: vec![],
            default_variant: None,
            is_default: false,
        })
        .unwrap();
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["defaultVariant", "description", "displayName", "id", "isDefault", "variants"]
        );
    }

    #[test]
    fn the_port_is_read_off_the_line_that_says_it() {
        assert_eq!(
            parse_port("opencode server listening on http://127.0.0.1:4096"),
            Some(4096)
        );
        assert_eq!(
            parse_port("opencode server listening on http://127.0.0.1:54888/"),
            Some(54888)
        );
        assert_eq!(parse_port("Warning: OPENCODE_SERVER_PASSWORD is not set"), None);
    }

    /// An id can itself contain a slash, so only the first one separates.
    #[test]
    fn a_model_is_split_on_its_first_slash_only() {
        assert_eq!(
            split_model("tss-nvidia-spark/nvidia/Qwen3.6-35B-A3B-NVFP4"),
            Some(("tss-nvidia-spark".into(), "nvidia/Qwen3.6-35B-A3B-NVFP4".into()))
        );
        assert_eq!(
            split_model("anthropic/claude-opus-4-5"),
            Some(("anthropic".into(), "claude-opus-4-5".into()))
        );
        assert_eq!(split_model("no-slash"), None);
    }

    /// The containment document has to parse, and it has to say the three
    /// things that were measured to matter: the siblings of `agents/` are
    /// denied individually rather than the root being denied; the bash map's
    /// last rule is an allow, or the tool disappears from the model's list
    /// entirely; and the agent's own config is named *relatively*, because an
    /// absolute pattern for a path inside the session directory matches
    /// nothing.
    #[test]
    fn the_rendered_config_denies_the_right_things() {
        let rendered = render_config(
            Path::new("/Users/x/Library/Application Support/oculus"),
            "# Working inside Oculus\n\n\"quoted\" \\ backslash",
            "You name conversations.",
        );
        let v: Value = serde_json::from_str(&rendered).expect("the template renders valid JSON");

        let edit = v["permission"]["edit"].as_object().unwrap();
        assert_eq!(edit["*"], "allow", "the root stays allowed; the siblings are named");
        for sib in ["courses/**", "lectures/**", "canvas-session/**", "oculus.db*"] {
            let key = format!("/Users/x/Library/Application Support/oculus/{sib}");
            assert_eq!(edit[&key], "deny", "{sib}");
        }
        assert_eq!(edit["opencode.json"], "deny", "relative, because it is inside the cwd");

        let bash: Vec<(&String, &Value)> = v["permission"]["bash"].as_object().unwrap().iter().collect();
        assert_eq!(bash.first().unwrap().1, "deny", "the default is no");
        assert_eq!(bash.last().unwrap().1, "allow", "or bash is removed from the tool list");

        // `deny` removes a tool entirely, which is what makes `question`
        // airtight: nothing can hang waiting for an answerer this app has
        // nowhere to put.
        for key in ["question", "webfetch", "websearch", "task"] {
            assert_eq!(v["permission"][key], "deny", "{key}");
        }
        assert_eq!(v["permission"]["read"], "allow", "the library stays readable");

        assert!(v["agent"][AGENT]["prompt"]
            .as_str()
            .unwrap()
            .contains("\"quoted\" \\ backslash"));
        assert_eq!(v["agent"][NAMING_AGENT]["hidden"], true);
    }
}
