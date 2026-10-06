//! The opencode bridge: HTTP and SSE against one `opencode serve` per app.
//!
//! One server (started on first use, killed on quit), one session per thread,
//! and one `GET /event` stream whose reader routes each event by session id.
//! Everything is opencode's v1 session API, scoped with `?directory=<agents>`
//! because an unscoped call binds to the server's own cwd. Not v2 (`/api/*`):
//! on 1.18.31 a v2 prompt on an `auth.json` provider fails inside the server
//! and emits nothing. The stream's rules are on [`translate`].
//!
//! opencode has no OS sandbox. Containment is its permission ruleset
//! (`templates/OPENCODE.template.json`, rule shapes in docs/harness.md), and
//! `bash` is a glob over the command string — a speed bump, not a boundary.
//! An agent's `prompt` replaces the whole system prompt, so the rendered
//! harness brief is the `oculus` agent's prompt and the per-thread part rides
//! the session's first message.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use super::child::{str_of as s, ChildProc};
use super::event::{cap_output, classify, HarnessEvent, Provider};
use super::{RawLog, Sink};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(20);
/// User row to first assistant row, which opencode creates at once: not a
/// slow-model budget ([`OpencodeServer::watch_drains`]).
const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// The server's argv after the binary. [`sweep`] recognises a stray by exactly
/// this list. `--port 0` means "prefer 4096", not "any free port".
const SERVE_ARGS: [&str; 6] = [
    "serve",
    "--port",
    "0",
    "--hostname",
    "127.0.0.1",
    "--print-logs",
];

/// The agent id the sessions run as, defined in the rendered config.
pub const AGENT: &str = "oculus";
/// The hidden agents the one-off turns run as (`Harness::one_off`): no tools,
/// and the turn's brief as the whole prompt.
pub const NAMING_AGENT: &str = "oculus-namer";
pub const WRITER_AGENT: &str = "oculus-writer";

const CONFIG_TEMPLATE: &str = include_str!("../../templates/OPENCODE.template.json");
pub const CONFIG_NAME: &str = "opencode.json";

pub struct OpencodeSpawn {
    pub bin: PathBuf,
    /// The library's `agents/` folder: session directory, opencode's project
    /// root, and the only place the agent may write.
    pub directory: PathBuf,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
    /// Takes the events that name no session.
    pub default_sink: Option<Sink>,
}

/// How to open a session. `model` is `providerID/id`; `variant` is a reasoning
/// level the model itself declared (none do as of 1.18.x).
#[derive(Default, Clone)]
pub struct OpencodeSessionOpts {
    pub model: Option<String>,
    pub variant: Option<String>,
    /// The per-thread part of the brief, sent ahead of the first message.
    pub brief: String,
    /// [`AGENT`], [`NAMING_AGENT`] or [`WRITER_AGENT`].
    pub agent: &'static str,
}

/// One row of the catalogue, in the shape `app/src/lib/harness.ts`'s
/// `OpencodeModel` reads.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// `providerID/id`; see [`split_model`].
    pub id: String,
    pub display_name: String,
    pub description: String,
    /// The model's own reasoning levels (empty for every model in 1.18.x).
    pub variants: Vec<String>,
    pub default_variant: Option<String>,
    pub is_default: bool,
    /// What the catalogue claims; `unusableReason` in
    /// `app/src/lib/opencodeCatalogue.ts` decides what that means.
    pub tool_call: bool,
    pub text_input: bool,
    pub text_output: bool,
}
    /// What Settings → opencode's model table shows.
    pub facts: ModelFacts,
}

/// A model's price, limits and capabilities as the catalogue states them —
/// `ModelFacts` in `app/src/lib/harness.ts`. An absent field is `None`:
/// unknown, which a price column must not draw as zero.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelFacts {
    pub cost: Option<ModelCost>,
    pub context: Option<u64>,
    pub max_output: Option<u64>,
    pub reasoning: bool,
    pub attachment: bool,
    /// Input modalities besides text, in a fixed order.
    pub inputs: Vec<String>,
    pub release_date: Option<String>,
    pub family: Option<String>,
}

/// USD per million tokens.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    pub cache_write: Option<f64>,

/// One row of Settings → AI's provider list, in the shape
/// `app/src/lib/opencodeAuth.ts` reads.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    /// `env` | `config` | `custom` | `api`. `config` is declared in an
    /// `opencode.json`, not signed in to, so it has no credential to remove.
    pub source: String,
    /// Env vars the provider reads a key from. A hint only: the harness
    /// strips provider keys from the child's environment.
    pub env: Vec<String>,
    pub model_count: usize,
    pub connected: bool,
    /// Never empty: [`default_method`] stands in for a provider that
    /// declares none.
    pub methods: Vec<AuthMethod>,
}

/// What a provider read answers: the list, and whether it is current.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderList {
    pub providers: Vec<ProviderInfo>,
    /// A refresh was skipped because a turn was running: the credential was
    /// written, but `connected` is still the state before it.
    pub stale: bool,
}

/// One way in, as opencode declares it: a form spec the dialog draws from.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethod {
    /// Position in the provider's `/provider/auth` array — the only name the
    /// OAuth endpoints have for a method, so [`parse_methods`] never reorders.
    pub index: usize,
    /// `oauth` | `api`.
    pub kind: String,
    pub label: String,
    /// Extra fields. An `api` method's key is not one of them.
    pub prompts: Vec<AuthPrompt>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthPrompt {
    /// `text` | `select`.
    pub kind: String,
    pub key: String,
    pub message: String,
    pub placeholder: Option<String>,
    /// Empty unless `kind` is `select`.
    pub options: Vec<AuthOption>,
    /// Shows this field only when another answer matches.
    pub when: Option<AuthWhen>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthOption {
    pub label: String,
    pub value: String,
    pub hint: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthWhen {
    pub key: String,
    /// `eq` | `neq`.
    pub op: String,
    pub value: String,
}

/// What `POST …/oauth/authorize` answers. `method` is `auto` when the server
/// finishes the flow itself, `code` when the student pastes something back.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Authorization {
    pub url: String,
    pub method: String,
    pub instructions: String,
}

struct SessionRoute {
    sink: Sink,
    /// Sent on every prompt: the agent and model a session was created with
    /// are ignored, and a prompt that omits them runs as opencode's stock
    /// `build` agent with none of the containment (1.18.31).
    agent: &'static str,
    model: Option<String>,
    variant: Option<String>,
    state: Mutex<SessionState>,
}

#[derive(Default)]
struct SessionState {
    /// Only [`close_turn`] clears this, so `TurnFinished` fires once per turn.
    turn_open: bool,
    /// Text by part id, from the deltas. Whatever is left at close is a part
    /// that never got its closing snapshot, and is committed rather than lost.
    text: HashMap<String, String>,
    reasoning: HashMap<String, String>,
    /// `callID` → tool name.
    tools: HashMap<String, String>,
    open_tools: std::collections::HashSet<String>,
    /// `callID` → how much of a running tool's `state.metadata.output` has
    /// gone out as [`HarnessEvent::ToolOutputDelta`].
    tool_output_len: HashMap<String, usize>,
    /// User message ids a turn has opened on; the user row is re-emitted after
    /// every step.
    seen_user: std::collections::HashSet<String>,
    /// This turn's assistant messages (one per step) and whether each has
    /// `time.completed`.
    assistants: Vec<(String, bool)>,
    /// An `Error` row has gone out, so a backstop close need not add one.
    turn_errored: bool,
    /// An abort's `session.error` has been seen, so the next idle is not the
    /// close (see [`translate`]).
    aborting: bool,
    /// Running totals: `step-finish` reports one step, not the session.
    total_input: u64,
    total_output: u64,
    total_cost: f64,
    context_window: Option<u64>,
    /// The per-thread brief, until the first prompt carries it.
    pending_brief: Option<String>,
    /// Set when the user row opens a turn, cleared by the first assistant row.
    awaiting_step: Option<Instant>,
}

pub struct OpencodeServer {
    proc: ChildProc,
    base: String,
    directory: PathBuf,
    /// With a timeout. `stream` has none: it would cut a healthy idle stream.
    api: ureq::Agent,
    stream: ureq::Agent,
    routes: Mutex<HashMap<String, Arc<SessionRoute>>>,
    default_sink: Option<Sink>,
    raw_log: Option<RawLog>,
}

impl OpencodeServer {
    pub fn spawn(cfg: OpencodeSpawn) -> Result<Arc<Self>, String> {
        let mut cmd = Command::new(&cfg.bin);
        cmd.args(SERVE_ARGS)
            .env_clear()
            .envs(cfg.env.iter().map(|(k, v)| (k, v)))
            // No binary swapped under a running thread, no coursework on the web.
            .env("OPENCODE_DISABLE_AUTOUPDATE", "1")
            .env("OPENCODE_DISABLE_SHARE", "1");
        // stderr is the structured log, kept only as a tail.
        let (proc, stdout) = ChildProc::spawn("opencode", &mut cmd, false)?;

        // The real port is printed only on stdout. This thread then owns
        // stdout for the process's life: its EOF is how the bridge learns the
        // server is gone.
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
                proc.kill();
                return Err(format!(
                    "opencode serve did not start in {}s",
                    READY_TIMEOUT.as_secs()
                ));
            }
        };

        let server = Arc::new(OpencodeServer {
            proc,
            base: format!("http://127.0.0.1:{port}"),
            directory: cfg.directory.clone(),
            api: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .timeout(REQUEST_TIMEOUT)
                .build(),
            stream: ureq::AgentBuilder::new()
                .timeout_connect(Duration::from_secs(5))
                .build(),
            routes: Mutex::new(HashMap::new()),
            default_sink: cfg.default_sink,
            raw_log: cfg.raw_log,
        });
        let _ = srv_tx.send(server.clone());

        server.get("/global/health")?;
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

    /// No session is created until `oculus` is in the directory's agent list:
    /// without it a session runs as a built-in agent with none of the
    /// containment. Normally passes on the first request.
    fn await_bootstrap(&self) -> Result<(), String> {
        let deadline = Instant::now() + BOOTSTRAP_TIMEOUT;
        let mut last;
        loop {
            match self.get(&format!("/agent?{}", self.directory_query())) {
                Ok(v) => {
                    let found = v
                        .as_array()
                        .is_some_and(|a| a.iter().any(|x| x["name"].as_str() == Some(AGENT)));
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

    /// On every instance-scoped call: an unscoped one binds to the server's
    /// own cwd, a different instance from ours.
    fn directory_query(&self) -> String {
        format!("directory={}", urlencode(&self.directory.display().to_string()))
    }

    pub fn is_alive(&self) -> bool {
        self.proc.is_alive()
    }

    // ── HTTP ─────────────────────────────────────────────────────────────

    fn get(&self, path: &str) -> Result<Value, String> {
        self.finish(self.api.get(&format!("{}{path}", self.base)).call(), path)
    }

    /// `send_string`: ureq's `json` feature is off in this crate.
    fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        self.finish(
            self.api
                .post(&format!("{}{path}", self.base))
                .set("Content-Type", "application/json")
                .send_string(&body.to_string()),
            path,
        )
    }

    /// A `POST` with no body, for endpoints that declare none (`/abort`).
    fn post_empty(&self, path: &str) -> Result<Value, String> {
        self.finish(self.api.post(&format!("{}{path}", self.base)).call(), path)
    }

    fn delete(&self, path: &str) -> Result<Value, String> {
        self.finish(self.api.delete(&format!("{}{path}", self.base)).call(), path)
    }

    /// 204 or an empty body is `Null`. Error text is [`scrub`]bed: response
    /// bodies can echo provider keys, and errors end up in logs and rows.
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
                let msg = match serde_json::from_str::<Value>(&body) {
                    Ok(v) => error_sentence(&v),
                    Err(_) => scrub(&body.chars().take(400).collect::<String>()),
                };
                Err(format!("opencode {path}: HTTP {code} {msg}"))
            }
            Err(e) => Err(format!("opencode {path}: {}", scrub(&e.to_string()))),
        }
    }

    // ── Models ───────────────────────────────────────────────────────────

    /// Every model opencode can reach, from `GET /config/providers` — what
    /// `opencode models` prints. Not `/api/model`: that lists only the
    /// providers the instance happened to instantiate, and omits signed-in
    /// ones like OpenRouter entirely (1.18.2). The response is never logged
    /// or quoted into an error: rows can carry the provider's real key.
    pub fn list_models(&self) -> Result<Vec<ModelInfo>, String> {
        parse_models(&self.get(&format!("/config/providers?{}", self.directory_query()))?)
    }

    /// One `providerID/id`'s context window, for the usage ring. Same source
    /// as [`Self::list_models`], so any model picked there is found here.
    fn context_window(&self, model: &str) -> Option<u64> {
        let (provider, id) = split_model(model)?;
        let v = self
            .get(&format!("/config/providers?{}", self.directory_query()))
            .ok()?;
        v["providers"]
            .as_array()?
            .iter()
            .find(|p| p["id"].as_str() == Some(provider.as_str()))?["models"]
            .as_object()?
            .values()
            .find(|m| m["id"].as_str() == Some(id.as_str()))?["limit"]["context"]
            .as_u64()
    }

    // ── Providers and credentials ────────────────────────────────────────
    //
    // This server is the supported door to opencode's `auth.json`, and runs
    // the OAuth loopback listener in-process. `connected` comes from instance
    // state that a credential write does not invalidate (1.18.2), so a write
    // is followed by [`Self::refresh`].

    /// True while any session has a turn open.
    pub fn busy(&self) -> bool {
        self.routes
            .lock()
            .unwrap()
            .values()
            .any(|r| r.state.lock().unwrap().turn_open)
    }

    /// `POST /instance/dispose`, which re-reads `auth.json`. Sessions and the
    /// stream survive it, but it releases the instance's resources, so it is
    /// skipped (and `false` returned) while a turn runs.
    pub fn refresh(&self) -> bool {
        if self.busy() {
            return false;
        }
        self.post(&format!("/instance/dispose?{}", self.directory_query()), json!({}))
            .is_ok()
    }

    /// Every provider opencode knows, whether it is connected, and its
    /// sign-in methods.
    pub fn list_providers(&self) -> Result<Vec<ProviderInfo>, String> {
        let v = self.get(&format!("/provider?{}", self.directory_query()))?;
        // Unreadable methods degrade every provider to the plain-key form.
        let methods = self
            .get(&format!("/provider/auth?{}", self.directory_query()))
            .unwrap_or(Value::Null);
        parse_providers(&v, &methods)
    }

    /// One method's form spec, read back from the server so what is sent is
    /// filtered against what opencode declares now, not a stale webview copy.
    pub fn auth_method(&self, provider: &str, index: usize) -> Result<AuthMethod, String> {
        let v = self.get(&format!("/provider/auth?{}", self.directory_query()))?;
        let methods = parse_methods(&v[provider]);
        methods
            .into_iter()
            .find(|m| m.index == index)
            .ok_or_else(|| format!("opencode has no sign-in method {index} for {provider}"))
    }

    /// Write an API key to opencode's store. Never held, and redacted out of
    /// any error on the way back.
    pub fn set_api_key(
        &self,
        provider: &str,
        key: &str,
        metadata: &std::collections::BTreeMap<String, String>,
    ) -> Result<(), String> {
        self.put_auth(provider, api_credential(key, metadata))
            .map_err(|e| redact(&e, key))
    }

    pub fn remove_auth(&self, provider: &str) -> Result<(), String> {
        self.delete(&format!("/auth/{}", urlencode(provider))).map(|_| ())
    }

    /// Start a browser flow. `method` is [`AuthMethod::index`].
    pub fn oauth_authorize(
        &self,
        provider: &str,
        method: usize,
        inputs: &std::collections::BTreeMap<String, String>,
    ) -> Result<Authorization, String> {
        let mut body = json!({ "method": method });
        if !inputs.is_empty() {
            body["inputs"] = json!(inputs);
        }
        let v = self.post(
            &format!("/provider/{}/oauth/authorize?{}", urlencode(provider), self.directory_query()),
            body,
        )?;
        Ok(Authorization {
            url: v["url"].as_str().unwrap_or_default().to_string(),
            method: v["method"].as_str().unwrap_or("auto").to_string(),
            instructions: v["instructions"].as_str().unwrap_or_default().to_string(),
        })
    }

    /// Finish a `code` flow with what the student pasted. An `auto` flow
    /// completes server-side and is noticed by refreshing.
    pub fn oauth_callback(&self, provider: &str, method: usize, code: Option<&str>) -> Result<(), String> {
        let mut body = json!({ "method": method });
        if let Some(c) = code {
            body["code"] = json!(c);
        }
        let v = self.post(
            &format!("/provider/{}/oauth/callback?{}", urlencode(provider), self.directory_query()),
            body,
        )?;
        if v.as_bool() == Some(false) {
            return Err("opencode rejected the code. It may have expired — try again.".into());
        }
        Ok(())
    }

    /// No `?directory=`: `/auth/{id}` is machine-wide, not instance-scoped.
    fn put_auth(&self, provider: &str, body: Value) -> Result<(), String> {
        let path = format!("/auth/{}", urlencode(provider));
        let v = self.finish(
            self.api
                .put(&format!("{}{path}", self.base))
                .set("Content-Type", "application/json")
                .send_string(&body.to_string()),
            &path,
        )?;
        if v.as_bool() == Some(false) {
            return Err("opencode would not accept that credential.".into());
        }
        Ok(())
    }

    // ── Sessions ─────────────────────────────────────────────────────────

    fn route(&self, session: &str, route: SessionRoute) {
        self.routes.lock().unwrap().insert(session.to_string(), Arc::new(route));
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
    /// arrives on the stream ([`translate`]). The harness queue guarantees
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

    /// `POST /session/{id}/abort`; the stream's side is in [`translate`].
    pub fn interrupt(&self, session: &str) -> Result<(), String> {
        self.post_empty(&format!("/session/{session}/abort?{}", self.directory_query()))?;
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

    pub fn kill(&self) {
        self.proc.kill();
    }

    // ── Inbound ──────────────────────────────────────────────────────────

    /// One `GET /event` for the whole app. A dropped stream fails the open
    /// turns before reconnecting: the gap may have eaten their close, and the
    /// queue releases a thread only on `TurnFinished`.
    fn read_events(self: Arc<Self>) {
        let mut backoff = Duration::from_millis(200);
        let url = format!("{}/event?{}", self.base, self.directory_query());
        while self.is_alive() {
            match self.stream.get(&url).call() {
                Ok(resp) => {
                    backoff = Duration::from_millis(200);
                    for line in BufReader::new(resp.into_reader()).lines().map_while(Result::ok) {
                        let Some(payload) = line.strip_prefix("data: ") else {
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

    /// Backstop: fail a turn whose user row arrived with no assistant row in
    /// [`DRAIN_TIMEOUT`]. The known failures close themselves; a server that
    /// goes silent instead would otherwise strand the thread for good.
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
                (r.sink)(HarnessEvent::error_for(
                    Provider::Opencode,
                    "opencode took the message and then never started answering it — \
                     usually a model it cannot resolve, or a provider it is not signed \
                     in to. Check the model in the picker and its provider in Settings.",
                ));
                for ev in events {
                    (r.sink)(ev);
                }
            }
        }
    }

    fn dispatch(&self, v: &Value) {
        let Some(ty) = v["type"].as_str() else { return };
        // Session-less events go to the harness sink rather than being lost
        // to the route lookup.
        let Some(session) = session_of(v) else {
            if let Some(sink) = &self.default_sink {
                for ev in translate_server(ty, &v["properties"]) {
                    sink(ev);
                }
            }
            return;
        };
        let route = self.routes.lock().unwrap().get(session).cloned();
        // Not ours: the student's own TUI, or a detached naming session.
        let Some(route) = route else { return };
        let events = {
            let mut st = route.state.lock().unwrap();
            translate(ty, &v["properties"], &mut st)
        };
        for ev in events {
            (route.sink)(ev);
        }
    }

    /// Close every open turn as failed, when the stream drops or the process
    /// dies.
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
            (r.sink)(HarnessEvent::error_for(Provider::Opencode, why));
            for ev in events {
                (r.sink)(ev);
            }
        }
    }

    fn on_exit(&self) {
        let code = self.proc.reap();
        self.fail_open_turns(&self.proc.with_tail(format!("opencode serve exited (code {code:?})")));
        let routes: Vec<Arc<SessionRoute>> = self.routes.lock().unwrap().drain().map(|(_, r)| r).collect();
        for r in routes {
            (r.sink)(HarnessEvent::Exited { code });
        }
    }
}

// ── Strays ───────────────────────────────────────────────────────────────────

/// How long a server gets to close its listener before it is killed outright.
const STRAY_GRACE: Duration = Duration::from_secs(2);

/// Kill the `opencode serve` processes a signalled app left behind (a
/// `tauri dev` relaunch, a force-quit or crash runs neither `Drop` nor
/// `RunEvent::Exit`), and answer with the pids. Called once at startup.
///
/// A stray is **our argv** ([`SERVE_ARGS`]) **and `ppid == 1`** (adopted by
/// launchd): a living app's server, such as a worktree build running beside
/// this one, is parented to that app and never touched. Same uid, too.
pub fn sweep() -> Vec<u32> {
    let uid = unsafe { libc::getuid() };
    let found = strays(&ps_listing(), uid);
    if found.is_empty() {
        return found;
    }
    send_signal(&found, libc::SIGTERM);
    // SIGKILL survivors, re-listing first in case a pid was reused.
    let sent = found.clone();
    std::thread::spawn(move || {
        std::thread::sleep(STRAY_GRACE);
        let left: Vec<u32> = strays(&ps_listing(), uid)
            .into_iter()
            .filter(|p| sent.contains(p))
            .collect();
        send_signal(&left, libc::SIGKILL);
    });
    found
}

fn ps_listing() -> String {
    Command::new("/bin/ps")
        .args(["-axww", "-o", "pid=,ppid=,uid=,command="])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn send_signal(pids: &[u32], sig: i32) {
    for pid in pids {
        unsafe { libc::kill(*pid as libc::pid_t, sig) };
    }
}

/// The stray pids in a `ps -axww -o pid=,ppid=,uid=,command=` listing. The
/// command is matched from its end so a binary path with a space resolves.
fn strays(listing: &str, uid: u32) -> Vec<u32> {
    let tail = SERVE_ARGS.join(" ");
    listing
        .lines()
        .filter_map(|line| {
            let mut rest = line;
            let pid: u32 = ps_field(&mut rest)?.parse().ok()?;
            let ppid: u32 = ps_field(&mut rest)?.parse().ok()?;
            let owner: u32 = ps_field(&mut rest)?.parse().ok()?;
            if ppid != 1 || owner != uid || pid == std::process::id() {
                return None;
            }
            let bin = rest.trim().strip_suffix(&tail)?.trim_end();
            (Path::new(bin).file_name()? == "opencode").then_some(pid)
        })
        .collect()
}

/// One space-delimited field off the front, advancing `rest` past it.
fn ps_field<'a>(rest: &mut &'a str) -> Option<&'a str> {
    let trimmed = rest.trim_start();
    let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let (head, tail) = trimmed.split_at(end);
    *rest = tail;
    (!head.is_empty()).then_some(head)
}

// ── The config document ──────────────────────────────────────────────────────

/// The hidden agents' prompts, one per [`NAMING_AGENT`] and [`WRITER_AGENT`].
pub struct OneOffPrompts<'a> {
    pub naming: &'a str,
    pub writer: &'a str,
}

/// Write `agents/opencode.json`: the permissions and the system prompts.
/// Rewritten on every server start, since both carry the library's paths.
pub fn write_config(
    directory: &Path,
    library: &Path,
    prompt: &str,
    one_off: &OneOffPrompts,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(directory).map_err(|e| format!("cannot create {}: {e}", directory.display()))?;
    let path = directory.join(CONFIG_NAME);
    std::fs::write(&path, render_config(library, prompt, one_off))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

/// Text substitution, never a parse/re-serialize: the ruleset is ordered
/// (last rule wins) and `serde_json`'s map sorts keys, which would silently
/// invert the containment. Values are JSON-escaped.
fn render_config(library: &Path, prompt: &str, one_off: &OneOffPrompts) -> String {
    CONFIG_TEMPLATE
        .replace("{{LIBRARY}}", &json_fragment(&library.display().to_string()))
        .replace("\"{{PROMPT}}\"", &json_string(prompt))
        .replace("\"{{NAMING_PROMPT}}\"", &json_string(one_off.naming))
        .replace("\"{{WRITER_PROMPT}}\"", &json_string(one_off.writer))
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// [`json_string`] without the quotes.
fn json_fragment(s: &str) -> String {
    let q = json_string(s);
    q[1..q.len() - 1].to_string()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// `opencode server listening on http://127.0.0.1:4096` (stdout only).
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

/// `GET /provider` and `GET /provider/auth`, merged into Settings' rows.
/// `/provider` echoes a connected provider's real key in `key`, which
/// [`scrub`] does not catch: that field is never read, and the response is
/// never quoted into an error.
fn parse_providers(all: &Value, methods: &Value) -> Result<Vec<ProviderInfo>, String> {
    let list = all["all"].as_array().ok_or("opencode /provider: no providers")?;
    let connected: std::collections::HashSet<&str> = all["connected"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut out: Vec<ProviderInfo> = list
        .iter()
        .filter_map(|p| {
            let id = p["id"].as_str()?.to_string();
            Some(ProviderInfo {
                name: p["name"].as_str().unwrap_or(&id).to_string(),
                source: p["source"].as_str().unwrap_or("custom").to_string(),
                env: p["env"]
                    .as_array()
                    .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
                    .unwrap_or_default(),
                model_count: p["models"].as_object().map_or(0, serde_json::Map::len),
                connected: connected.contains(id.as_str()),
                methods: parse_methods(&methods[&id]),
                id,
            })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// The body of `PUT /auth/{id}`. A method's extra fields are `metadata`,
/// omitted when empty to match what opencode's own store writes.
fn api_credential(key: &str, metadata: &std::collections::BTreeMap<String, String>) -> Value {
    let mut body = json!({ "type": "api", "key": key });
    if !metadata.is_empty() {
        body["metadata"] = json!(metadata);
    }
    body
}

/// What a provider with no `/provider/auth` entry takes: a plain API key,
/// which opencode accepts for any provider.
fn default_method() -> AuthMethod {
    AuthMethod {
        index: 0,
        kind: "api".into(),
        label: "API key".into(),
        prompts: Vec::new(),
    }
}

/// `/provider/auth`'s array for one provider, never dropped or reordered
/// ([`AuthMethod::index`]).
fn parse_methods(v: &Value) -> Vec<AuthMethod> {
    let Some(arr) = v.as_array() else {
        return vec![default_method()];
    };
    let out: Vec<AuthMethod> = arr
        .iter()
        .enumerate()
        .filter_map(|(index, m)| {
            let kind = m["type"].as_str()?;
            Some(AuthMethod {
                index,
                kind: kind.to_string(),
                label: m["label"].as_str().unwrap_or(kind).to_string(),
                prompts: m["prompts"]
                    .as_array()
                    .map(|a| a.iter().filter_map(parse_prompt).collect())
                    .unwrap_or_default(),
            })
        })
        .collect();
    if out.len() == arr.len() && !out.is_empty() {
        out
    } else {
        // An unreadable method would shift every index after it.
        vec![default_method()]
    }
}

fn parse_prompt(p: &Value) -> Option<AuthPrompt> {
    let kind = p["type"].as_str()?;
    if kind != "text" && kind != "select" {
        return None;
    }
    Some(AuthPrompt {
        kind: kind.to_string(),
        key: p["key"].as_str()?.to_string(),
        message: p["message"].as_str().unwrap_or_default().to_string(),
        placeholder: p["placeholder"].as_str().map(String::from),
        options: p["options"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|o| {
                        Some(AuthOption {
                            label: o["label"].as_str()?.to_string(),
                            value: o["value"].as_str()?.to_string(),
                            hint: o["hint"].as_str().map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        when: p["when"].as_object().and_then(|w| {
            Some(AuthWhen {
                key: w.get("key")?.as_str()?.to_string(),
                op: w.get("op")?.as_str()?.to_string(),
                value: w.get("value")?.as_str()?.to_string(),
            })
        }),
    })
}

/// Whether a prompt is on screen. An unanswered dependency reads as "".
fn prompt_visible(p: &AuthPrompt, answers: &std::collections::BTreeMap<String, String>) -> bool {
    let Some(w) = &p.when else { return true };
    let actual = answers.get(&w.key).map(String::as_str).unwrap_or("");
    match w.op.as_str() {
        "eq" => actual == w.value,
        "neq" => actual != w.value,
        _ => true,
    }
}

/// The answers that belong to a method's visible fields, applied on the way
/// out: a field hidden again still holds its value in the webview's state.
pub fn visible_answers(
    method: &AuthMethod,
    answers: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, String> {
    method
        .prompts
        .iter()
        .filter(|p| prompt_visible(p, answers))
        .filter_map(|p| {
            let v = answers.get(&p.key)?;
            (!v.is_empty()).then(|| (p.key.clone(), v.clone()))
        })
        .collect()
}

/// Take the secret the app is holding out of an error that may quote the
/// request back.
fn redact(s: &str, secret: &str) -> String {
    if secret.len() < 8 {
        return s.to_string();
    }
    s.replace(secret, "[redacted]")
}

/// Truncate at the first key-like field name, so a response body echoing a
/// provider key never reaches a log.
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

/// The rows of a `GET /config/providers`, minus disabled and deprecated
/// models, sorted by id.
fn parse_models(v: &Value) -> Result<Vec<ModelInfo>, String> {
    let list = v["providers"]
        .as_array()
        .ok_or("opencode /config/providers: no providers")?;
    let mut out = Vec::new();
    for p in list {
        let provider = p["id"].as_str().unwrap_or("");
        // A map keyed by id; the entry's own `id` is what is read.
        let Some(models) = p["models"].as_object() else {
            continue;
        };
        for m in models.values() {
            let id = m["id"].as_str().unwrap_or("");
            if id.is_empty() || provider.is_empty() {
                continue;
            }
            if m["enabled"].as_bool() == Some(false) {
                continue;
            }
            if m["status"].as_str() == Some("deprecated") {
                continue;
            }
            let variants = variant_ids(&m["variants"]);
            let name = m["name"].as_str().unwrap_or(id);
            let caps = &m["capabilities"];
            // Absent reads as capable, or a provider whose rows say nothing
            // would have its whole catalogue gated out.
            let flag = |v: &Value| v.as_bool() != Some(false);
            out.push(ModelInfo {
                id: format!("{provider}/{id}"),
                display_name: name.to_string(),
                description: describe(m),
                default_variant: default_variant(&variants),
                variants,
                is_default: false,
                tool_call: flag(&caps["toolcall"]),
                text_input: flag(&caps["input"]["text"]),
                text_output: flag(&caps["output"]["text"]),
            });
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
                facts: facts(m),
    Ok(out)
}

/// A model's reasoning levels: an object keyed by id, or an array of `{id}`.
fn variant_ids(v: &Value) -> Vec<String> {
    if let Some(a) = v.as_array() {
        return a.iter().filter_map(|x| x["id"].as_str().map(String::from)).collect();
/// The table's columns off one catalogue row. Absent stays `None`, never 0.
fn facts(m: &Value) -> ModelFacts {
    let caps = &m["capabilities"];
    let cost = m["cost"].as_object().map(|c| ModelCost {
        input: c.get("input").and_then(Value::as_f64),
        output: c.get("output").and_then(Value::as_f64),
        cache_read: m["cost"]["cache"]["read"].as_f64(),
        cache_write: m["cost"]["cache"]["write"].as_f64(),
    });
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(String::from);
    ModelFacts {
        cost,
        context: m["limit"]["context"].as_u64(),
        max_output: m["limit"]["output"].as_u64(),
        reasoning: caps["reasoning"].as_bool() == Some(true),
        attachment: caps["attachment"].as_bool() == Some(true),
        inputs: ["image", "pdf", "audio", "video"]
            .into_iter()
            .filter(|k| caps["input"][*k].as_bool() == Some(true))
            .map(String::from)
            .collect(),
        release_date: text(&m["release_date"]),
        family: text(&m["family"]),
    }
}

    }
    match v.as_object() {
        Some(o) => o.keys().cloned().collect(),
        None => Vec::new(),
    }
}

/// Where a fresh pick of this model lands: `high`, else the nearest level
/// below it, by name — the list's order is a JSON map's alphabetical one.
fn default_variant(variants: &[String]) -> Option<String> {
    ["high", "medium", "low", "minimal", "none"]
        .iter()
        .find_map(|p| variants.iter().find(|v| v.as_str() == *p).cloned())
        .or_else(|| variants.first().cloned())
}

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

// ── Translation ──────────────────────────────────────────────────────────────

/// The session id: `properties.sessionID` on most events, else inside `info`
/// or `part`. `info.id` is the session only on `session.created`/`updated`;
/// on a message event it is the message id.
fn session_of(v: &Value) -> Option<&str> {
    let p = &v["properties"];
    p["sessionID"]
        .as_str()
        .or_else(|| p["info"]["sessionID"].as_str())
        .or_else(|| p["part"]["sessionID"].as_str())
        .or_else(|| {
            matches!(v["type"].as_str(), Some("session.created" | "session.updated"))
                .then(|| p["info"]["id"].as_str())
                .flatten()
        })
}

/// Events that name no session (pty, lsp, mcp, `server.heartbeat`…). Only a
/// `session.error` whose optional `sessionID` is missing is worth surfacing.
fn translate_server(ty: &str, data: &Value) -> Vec<HarnessEvent> {
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
fn usage(st: &mut SessionState, t: &Value, cost: f64) -> HarnessEvent {
    let n = |k: &str| t[k].as_u64().unwrap_or(0);
    let input = n("input") + t["cache"]["read"].as_u64().unwrap_or(0) + t["cache"]["write"].as_u64().unwrap_or(0);
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
fn close_turn(st: &mut SessionState, status: &str) -> Vec<HarnessEvent> {
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
fn friendly(msg: &str) -> String {
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
fn error_sentence(e: &Value) -> String {
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
    let name = st.tools.get(id).cloned().unwrap_or_else(|| "unknown".into());
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
fn translate(ty: &str, p: &Value, st: &mut SessionState) -> Vec<HarnessEvent> {
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
                    let map = if thinking { &mut st.reasoning } else { &mut st.text };
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
                                state["error"].as_str().unwrap_or("the tool failed").to_string()
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
                    out.push(usage(st, &part["tokens"], part["cost"].as_f64().unwrap_or(0.0)));
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

#[cfg(test)]
mod tests {
    use super::super::event::ToolKind;
    use super::*;

    /// Routes like [`OpencodeServer::dispatch`]: session-less events first,
    /// then the first session the stream names is ours and any other is not.
    fn replay(raw: &str) -> Vec<HarnessEvent> {
        let mut st = SessionState::default();
        let mut ours: Option<String> = None;
        let mut events = Vec::new();
        for line in raw.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let Some(ty) = v["type"].as_str() else { continue };
            match session_of(&v) {
                None => events.extend(translate_server(ty, &v["properties"])),
                Some(id) => {
                    if ours.get_or_insert_with(|| id.to_string()) == id {
                        events.extend(translate(ty, &v["properties"], &mut st));
                    }
                }
            }
        }
        events
    }

    fn finishes(events: &[HarnessEvent]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::TurnFinished { status } => Some(status.as_str()),
                _ => None,
            })
            .collect()
    }

    fn errors(events: &[HarnessEvent]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::Error { message, .. } => Some(message.as_str()),
                _ => None,
            })
            .collect()
    }

    fn deltas(events: &[HarnessEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn messages(events: &[HarnessEvent]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::AssistantMessage { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn count(events: &[HarnessEvent], f: impl Fn(&HarnessEvent) -> bool) -> usize {
        events.iter().filter(|e| f(e)).count()
    }

    // ── Replayed from the 1.18.31 recordings ─────────────────────────────

    const V1_LS: &str = include_str!("../../fixtures/harness/opencode-v1-ls.ndjson");

    /// Two steps, one bash call with growing output snapshots, then a streamed
    /// answer. Opens on the user row (the anchor), closes once on `stop`.
    #[test]
    fn folds_a_recorded_v1_turn() {
        let events = replay(V1_LS);
        assert!(matches!(events.first(), Some(HarnessEvent::TurnStarted)));
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::TurnStarted)), 1, "user re-emits open nothing");
        let anchors: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::TurnAnchor { anchor } => Some(anchor.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(anchors, vec!["msg_0add5cba9001dBRhWoUADhRh8h"], "the user msg id, once");

        let tools: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolStarted { id, kind, name, title, input } => {
                    Some((id.clone(), *kind, name.clone(), title.clone(), input.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].0, "call_UnhBtW5By24FPWOwYzi30lZ6", "keyed by callID");
        assert_eq!(tools[0].1, ToolKind::Bash);
        assert_eq!(tools[0].2, "bash");
        assert!(tools[0].3.contains("ls"), "titled off the running snapshot's input, not pending's `{{}}`");
        assert_eq!(tools[0].4["command"], "ls");

        // Two empty `running` snapshots, then the whole listing: one delta.
        let out_deltas: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolOutputDelta { id, text } => {
                    assert_eq!(id, "call_UnhBtW5By24FPWOwYzi30lZ6");
                    Some(text.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(out_deltas, vec!["README-scratch.txt\nopencode.json\n"]);

        let finished_tools: Vec<(bool, &str)> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ToolFinished { ok, output, .. } => Some((*ok, output.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(finished_tools.len(), 1);
        assert!(finished_tools[0].0);
        assert!(finished_tools[0].1.contains("opencode.json"));

        let text = messages(&events);
        assert_eq!(text.len(), 1, "one answer, from the closing snapshot");
        assert_eq!(deltas(&events), text[0], "the deltas add up to the snapshot");
        assert!(!text[0].is_empty());
        assert!(!events.iter().any(|e| matches!(e, HarnessEvent::ThinkingDelta { .. } | HarnessEvent::Thinking { .. })));

        // One `Usage` per `step-finish`, accumulating fresh + cached input.
        let usage: Vec<(u64, u64, Option<u64>)> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::Usage { input_tokens, output_tokens, context_tokens, .. } => {
                    Some((*input_tokens, *output_tokens, *context_tokens))
                }
                _ => None,
            })
            .collect();
        assert_eq!(usage, vec![(5827, 79, Some(5906)), (5827 + 160 + 5760, 79 + 17, Some(5937))]);
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Usage { cost_usd: Some(c), .. } if *c > 0.003)));

        assert_eq!(finishes(&events), vec!["completed"]);
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })), "idle and the user re-emit after it add nothing");
        assert!(errors(&events).is_empty());
    }

    /// The smallest v1 turn: one step, one delta, `ok`.
    #[test]
    fn folds_a_recorded_v1_text_turn() {
        let events = replay(include_str!("../../fixtures/harness/opencode-v1-text.ndjson"));
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::TurnStarted)), 1);
        assert_eq!(messages(&events), vec!["ok"]);
        assert_eq!(deltas(&events), "ok");
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::Usage { .. })), 1);
        assert_eq!(finishes(&events), vec!["completed"]);
        assert!(errors(&events).is_empty());
        assert!(!events.iter().any(|e| matches!(e, HarnessEvent::ToolStarted { .. })));
    }

    /// Reasoning deltas say `field: "text"` too; only the opening snapshot's
    /// part type marks them as thinking.
    #[test]
    fn folds_a_recorded_v1_reasoning_turn() {
        let events = replay(include_str!("../../fixtures/harness/opencode-v1-reasoning.ndjson"));
        let thinking_deltas: String = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::ThinkingDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let thinking: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::Thinking { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(thinking.len(), 1);
        assert!(thinking[0].starts_with("The user is asking"));
        assert_eq!(thinking_deltas, thinking[0]);
        assert_eq!(messages(&events), vec!["ok"]);
        assert_eq!(deltas(&events), "ok", "the reasoning deltas were not read as answer text");
        let think_at = events.iter().position(|e| matches!(e, HarnessEvent::Thinking { .. })).unwrap();
        let answer_at = events.iter().position(|e| matches!(e, HarnessEvent::AssistantMessage { .. })).unwrap();
        assert!(think_at < answer_at);
        // 0 output + 31 reasoning.
        assert!(events.iter().any(|e| matches!(e, HarnessEvent::Usage { output_tokens: 31, .. })));
        assert_eq!(finishes(&events), vec!["completed"]);
        assert!(errors(&events).is_empty());
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })));
    }

    /// The first idle of an abort must not close the turn, or the partial
    /// answer that follows it is lost.
    #[test]
    fn a_v1_abort_is_interrupted_not_failed() {
        let events = replay(include_str!("../../fixtures/harness/opencode-v1-interrupt.ndjson"));
        let text = messages(&events);
        assert_eq!(text.len(), 1, "the partial answer is one row");
        assert!(!text[0].is_empty());
        assert_eq!(deltas(&events), text[0]);
        assert_eq!(finishes(&events), vec!["interrupted"]);
        assert!(errors(&events).is_empty(), "stopping a turn is not an error");
        let answer_at = events.iter().position(|e| matches!(e, HarnessEvent::AssistantMessage { .. })).unwrap();
        let finish_at = events.iter().position(|e| matches!(e, HarnessEvent::TurnFinished { .. })).unwrap();
        assert!(answer_at < finish_at, "the row lands before the turn closes");
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })), "the two idles add nothing");
    }

    /// An unknown model: no assistant row; the `session.error` closes the
    /// turn in the provider's words, and its stack-trace re-emit is dropped.
    #[test]
    fn a_v1_model_not_found_fails_once_with_the_providers_words() {
        let events = replay(include_str!("../../fixtures/harness/opencode-v1-error.ndjson"));
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::TurnStarted)), 1);
        let errs = errors(&events);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("Model not found"));
        assert!(errs[0].contains("Did you mean"));
        assert!(!errs[0].contains("\n    at "));
        assert_eq!(finishes(&events), vec!["failed"]);
        let err_at = events.iter().position(|e| matches!(e, HarnessEvent::Error { .. })).unwrap();
        let finish_at = events.iter().position(|e| matches!(e, HarnessEvent::TurnFinished { .. })).unwrap();
        assert!(err_at < finish_at);
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })), "idle and the trace add nothing");
        assert!(messages(&events).is_empty(), "the user's own text part is not an answer");
    }

    /// A repeated completed row and a late idle add no second finish.
    #[test]
    fn a_v1_turn_finishes_exactly_once() {
        let last_of = |ty: &str, role: Option<&str>| {
            V1_LS
                .lines()
                .filter(|l| {
                    let v: Value = serde_json::from_str(l).unwrap();
                    v["type"] == ty && role.is_none_or(|r| v["properties"]["info"]["role"] == r)
                })
                .last()
                .unwrap()
                .to_string()
        };
        let again = format!(
            "{V1_LS}\n{}\n{}\n",
            last_of("message.updated", Some("assistant")),
            last_of("session.idle", None)
        );
        let events = replay(&again);
        assert_eq!(finishes(&events), vec!["completed"]);
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::TurnStarted)), 1);
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::Usage { .. })), 2);
        assert!(errors(&events).is_empty());
    }

    /// A bad agent name: `session.error` twice and no idle, so the error closes.
    #[test]
    fn a_v1_bad_agent_closes_on_the_error_because_no_idle_follows() {
        let mut st = SessionState::default();
        let go = |st: &mut SessionState, ty: &str, p: Value| translate(ty, &p, st);
        let opened = go(&mut st, "message.updated", json!({"info": {"id": "msg_u1", "role": "user", "sessionID": "ses_1"}}));
        assert!(matches!(opened.as_slice(), [HarnessEvent::TurnStarted, HarnessEvent::TurnAnchor { anchor }] if anchor == "msg_u1"));
        let msg = "Agent not found: \"nope-agent\". Available agents: build, explore, general, oculus, plan";
        let out = go(&mut st, "session.error", json!({"sessionID": "ses_1", "error": {"name": "UnknownError", "data": {"message": msg}}}));
        assert_eq!(errors(&out), vec![msg]);
        assert_eq!(finishes(&out), vec!["failed"]);
        let trace = go(&mut st, "session.error", json!({"sessionID": "ses_1", "error": {"name": "UnknownError", "data": {"message": format!("AgentNotFoundError: {msg}\n    at <anonymous> (/$bunfs/root/x.js:1:1)")}}}));
        assert!(trace.is_empty(), "{trace:?}");
    }

    /// A tool that skips `running`, and an abort before any assistant row,
    /// both leave the timeline balanced.
    #[test]
    fn a_v1_tool_that_never_ran_still_opens_a_row_off_its_input() {
        let mut st = SessionState::default();
        let go = |st: &mut SessionState, ty: &str, p: Value| translate(ty, &p, st);
        go(&mut st, "message.updated", json!({"info": {"id": "msg_u1", "role": "user"}}));
        go(&mut st, "message.updated", json!({"info": {"id": "msg_a1", "role": "assistant", "parentID": "msg_u1", "time": {"created": 1}}}));
        let pending = go(&mut st, "message.part.updated", json!({"part": {"id": "prt_1", "messageID": "msg_a1", "type": "tool", "callID": "call_1", "tool": "read", "state": {"status": "pending", "input": {}}}}));
        assert!(pending.is_empty());
        let done = go(&mut st, "message.part.updated", json!({"part": {"id": "prt_1", "messageID": "msg_a1", "type": "tool", "callID": "call_1", "tool": "read", "state": {"status": "completed", "input": {"path": "../courses/COMP30026/w1.md"}, "output": "# Week 1", "title": "w1.md"}}}));
        assert!(matches!(&done[0], HarnessEvent::ToolStarted { id, kind: ToolKind::Read, name, title, .. } if id == "call_1" && name == "read" && title == "w1.md"));
        assert!(matches!(&done[1], HarnessEvent::ToolFinished { id, ok: true, output, .. } if id == "call_1" && output == "# Week 1"));
        assert_eq!(done.len(), 2);

        let mut st = SessionState::default();
        go(&mut st, "message.updated", json!({"info": {"id": "msg_u2", "role": "user"}}));
        let out = go(&mut st, "session.error", json!({"error": {"name": "MessageAbortedError", "data": {"message": "Aborted"}}}));
        assert_eq!(finishes(&out), vec!["interrupted"]);
        assert!(errors(&out).is_empty());
        assert!(go(&mut st, "session.idle", json!({"sessionID": "ses_1"})).is_empty());
    }

    #[test]
    fn the_zen_free_tier_gets_a_sentence_rather_than_a_400() {
        let raw = "Provider request failed with HTTP 400: {\"type\":\"error\",\"error\":\
                   {\"type\":\"MissingSessionID\",\"message\":\"Error from provider (Console): \
                   OpenCode's free tier can only be used in OpenCode\"}}";
        let out = friendly(raw);
        assert!(out.contains("opencode auth login"));
        assert!(!out.contains("HTTP 400"));
    }

    /// An assistant row created and never completed, then idle: idle fails
    /// the turn, once.
    #[test]
    fn a_v1_idle_with_the_assistant_still_open_fails_the_turn_once() {
        let cut: Vec<&str> = V1_LS.lines().take(7).collect();
        let last: Value = serde_json::from_str(cut[6]).unwrap();
        assert_eq!(last["type"], "message.updated");
        assert_eq!(last["properties"]["info"]["role"], "assistant");
        assert!(last["properties"]["info"]["time"]["completed"].is_null(), "the bare row, not a completed one");
        let raw = format!(
            "{}\n{{\"type\":\"session.idle\",\"properties\":{{\"sessionID\":\"ses_f522a3477ffeV1Sb54a1xq018p\"}}}}\n",
            cut.join("\n")
        );
        let events = replay(&raw);
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::TurnStarted)), 1);
        assert_eq!(finishes(&events), vec!["failed"]);
        let errs = errors(&events);
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].contains("without answering"));
        assert!(matches!(events.last(), Some(HarnessEvent::TurnFinished { .. })));
        let mut st = SessionState::default();
        for line in raw.lines() {
            let v: Value = serde_json::from_str(line).unwrap();
            if let Some(ty) = v["type"].as_str() {
                if session_of(&v).is_some() {
                    translate(ty, &v["properties"], &mut st);
                }
            }
        }
        assert!(translate("session.idle", &json!({"sessionID": "ses_1"}), &mut st).is_empty());
    }

    #[test]
    fn a_v1_user_row_opens_on_its_id_not_on_the_absence_of_summary() {
        let mut st = SessionState::default();
        let go = |st: &mut SessionState, p: Value| translate("message.updated", &p, st);
        let first = go(&mut st, json!({"info": {"id": "msg_u1", "role": "user", "summary": {"diffs": []}}}));
        assert!(matches!(first.as_slice(), [HarnessEvent::TurnStarted, HarnessEvent::TurnAnchor { anchor }] if anchor == "msg_u1"));
        assert!(go(&mut st, json!({"info": {"id": "msg_u1", "role": "user", "summary": {"diffs": []}}})).is_empty());
        assert!(go(&mut st, json!({"info": {"id": "msg_u1", "role": "user"}})).is_empty());
    }

    #[test]
    fn a_model_s_capabilities_come_off_the_row_and_default_to_capable() {
        let v = json!({
            "providers": [{ "id": "openrouter", "models": {
                "a": { "id": "a", "name": "A", "capabilities": {
                    "toolcall": false,
                    "input": { "text": true }, "output": { "text": true } } },
                "b": { "id": "b", "name": "B", "capabilities": {
                    "toolcall": true,
                    "input": { "text": true }, "output": { "text": false } } },
                "c": { "id": "c", "name": "C" }
            }}]
        });
        let models = parse_models(&v).unwrap();
        assert_eq!(models.len(), 3);
        assert!(!models[0].tool_call, "a: toolcall false is read");
        assert!(models[0].text_input && models[0].text_output);
        assert!(models[1].tool_call);
        assert!(!models[1].text_output, "b: an output it cannot write in text");
        assert!(
            models[2].tool_call && models[2].text_input && models[2].text_output,
            "c: no capabilities block at all reads as capable, never as refused"
        );
    }

    /// `OpencodeModel` in `app/src/lib/harness.ts` reads these names; a rename
    /// is a silently empty model list, not a type error.
    #[test]
    fn a_model_row_is_spelled_the_way_the_picker_reads_it() {
    #[test]
    fn a_model_s_facts_come_off_the_row_and_an_unpriced_row_is_unknown_not_free() {
        let v = json!({
            "providers": [{ "id": "openrouter", "models": {
                "a": { "id": "a", "name": "A", "family": "qwen", "release_date": "2026-05-21",
                    "cost": { "input": 1.475, "output": 4.425, "cache": { "read": 0.1 } },
                    "limit": { "context": 1000000, "output": 131072 },
                    "capabilities": { "reasoning": true, "attachment": true,
                        "input": { "text": true, "image": true, "pdf": true, "audio": false } } },
                "b": { "id": "b", "name": "B", "cost": { "input": 0, "output": 0 } },
                "c": { "id": "c", "name": "C", "family": "" }
            }}]
        });
        let models = parse_models(&v).unwrap();
        let a = &models[0].facts;
        let cost = a.cost.as_ref().unwrap();
        assert_eq!((cost.input, cost.output), (Some(1.475), Some(4.425)));
        assert_eq!((cost.cache_read, cost.cache_write), (Some(0.1), None));
        assert_eq!((a.context, a.max_output), (Some(1_000_000), Some(131_072)));
        assert!(a.reasoning && a.attachment);
        assert_eq!(a.inputs, ["image", "pdf"]);
        assert_eq!(a.release_date.as_deref(), Some("2026-05-21"));
        assert_eq!(a.family.as_deref(), Some("qwen"));
        assert_eq!(models[1].facts.cost.as_ref().unwrap().input, Some(0.0), "b: a stated zero is zero");
        assert_eq!(models[2].facts, ModelFacts::default(), "c: nothing stated is all unknown");
    }

    /// `ModelFacts` in `app/src/lib/harness.ts` reads these names.
    #[test]
    fn a_model_s_facts_are_spelled_the_way_the_table_reads_them() {
        let json = serde_json::to_value(ModelFacts {
            cost: Some(ModelCost { input: None, output: None, cache_read: None, cache_write: None }),
            ..ModelFacts::default()
        })
        .unwrap();
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            ["attachment", "context", "cost", "family", "inputs", "maxOutput", "reasoning", "releaseDate"]
        );
        let mut cost: Vec<&str> = json["cost"].as_object().unwrap().keys().map(|k| k.as_str()).collect();
        cost.sort_unstable();
        assert_eq!(cost, ["cacheRead", "cacheWrite", "input", "output"]);
    }

        let json = serde_json::to_value(ModelInfo {
            id: "anthropic/claude-opus-4-5".into(),
            display_name: "Claude Opus 4.5 (latest)".into(),
            description: "claude-opus · 200K context".into(),
            variants: vec![],
            default_variant: None,
            is_default: false,
            tool_call: true,
            text_input: true,
            text_output: true,
        })
        .unwrap();
        let mut keys: Vec<&str> = json.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
            facts: ModelFacts::default(),
        assert_eq!(
            keys,
            [
                "defaultVariant",
                "description",
                "displayName",
                "id",
                "isDefault",
                "textInput",
                "textOutput",
                "facts",
                "toolCall",
                "variants"
            ]
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

    /// Pins the rule shapes that work: siblings of `agents/` denied one by one,
    /// the bash map ending on an allow, and in-directory paths relative.
    #[test]
    fn the_rendered_config_denies_the_right_things() {
        let rendered = render_config(
            Path::new("/Users/x/Library/Application Support/oculus"),
            "# Working inside Oculus\n\n\"quoted\" \\ backslash",
            &OneOffPrompts {
                naming: "You name conversations.",
                writer: "You complete \"notes\".",
            },
        );
        let v: Value = serde_json::from_str(&rendered).expect("the template renders valid JSON");

        let edit = v["permission"]["edit"].as_object().unwrap();
        assert_eq!(edit["*"], "allow", "the root stays allowed; the siblings are named");
        for sib in ["courses/**", "lectures/**", "canvas-session/**", "oculus.db*"] {
            let key = format!("/Users/x/Library/Application Support/oculus/{sib}");
            assert_eq!(edit[&key], "deny", "{sib}");
        }
        for inside in ["opencode.json", "skills/**", ".opencode/**"] {
            assert_eq!(edit[inside], "deny", "{inside} is relative, being inside the cwd");
        }
        assert_eq!(v["skills"]["paths"], json!(["skills"]));

        let bash: Vec<(&String, &Value)> = v["permission"]["bash"].as_object().unwrap().iter().collect();
        assert_eq!(bash.first().unwrap().1, "deny", "the default is no");
        assert_eq!(bash.last().unwrap().1, "allow", "or bash is removed from the tool list");

        // `deny` removes the tool, so nothing waits on an answerer.
        for key in ["question", "task"] {
            assert_eq!(v["permission"][key], "deny", "{key}");
        }
        // Parity with Claude and Codex.
        for key in ["webfetch", "websearch"] {
            assert_eq!(v["permission"][key], "allow", "{key}");
        }
        assert_eq!(v["permission"]["read"], "allow", "the library stays readable");

        assert!(v["agent"][AGENT]["prompt"]
            .as_str()
            .unwrap()
            .contains("\"quoted\" \\ backslash"));
        // The one-off agents are hidden, tool-less (a trailing `*` deny is the
        // last rule for every tool) and carry their own prompt.
        for (agent, prompt) in [(NAMING_AGENT, "You name conversations."), (WRITER_AGENT, "You complete \"notes\".")] {
            assert_eq!(v["agent"][agent]["hidden"], true, "{agent}");
            assert_eq!(v["agent"][agent]["permission"]["*"], "deny", "{agent}");
            assert_eq!(v["agent"][agent]["prompt"], prompt, "{agent}");
        }
    }

    // ── Provider credentials (JSON verbatim from opencode 1.18.2) ─────────

    /// `openai`'s three ways in, as `/provider/auth` declares them.
    const OPENAI_METHODS: &str = r#"[
      { "type": "oauth", "label": "ChatGPT Pro/Plus (browser)" },
      { "type": "oauth", "label": "ChatGPT Pro/Plus (headless)" },
      { "type": "api",   "label": "Manually enter API Key" }
    ]"#;

    /// `github-copilot`: a select, and a text field shown for one answer.
    const COPILOT_METHODS: &str = r#"[
      { "type": "oauth", "label": "Login with GitHub Copilot", "prompts": [
        { "type": "select", "key": "deploymentType", "message": "Select GitHub deployment type",
          "options": [
            { "label": "GitHub.com", "value": "github.com", "hint": "Public" },
            { "label": "GitHub Enterprise", "value": "enterprise", "hint": "Data residency or self-hosted" }
          ] },
        { "type": "text", "key": "enterpriseUrl", "message": "Enter your GitHub Enterprise URL or domain",
          "placeholder": "company.ghe.com", "when": { "key": "deploymentType", "op": "eq", "value": "enterprise" } }
      ] }
    ]"#;

    fn method(json: &str, index: usize) -> AuthMethod {
        parse_methods(&serde_json::from_str::<Value>(json).unwrap())
            .into_iter()
            .find(|m| m.index == index)
            .expect("method")
    }

    fn answers(pairs: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn method_indices_are_positions_in_the_array() {
        let ms = parse_methods(&serde_json::from_str::<Value>(OPENAI_METHODS).unwrap());
        assert_eq!(ms.len(), 3);
        assert_eq!(
            ms.iter().map(|m| (m.index, m.kind.as_str())).collect::<Vec<_>>(),
            vec![(0, "oauth"), (1, "oauth"), (2, "api")],
        );
        assert_eq!(ms[2].label, "Manually enter API Key");
        assert!(ms[0].prompts.is_empty(), "the browser flow asks nothing up front");
    }

    #[test]
    fn an_unreadable_method_gives_up_the_list_rather_than_renumbering() {
        let ms = parse_methods(&serde_json::from_str::<Value>(
            r#"[{ "label": "no type here" }, { "type": "api", "label": "Key" }]"#,
        ).unwrap());
        assert_eq!(ms, vec![default_method()]);
    }

    #[test]
    fn a_provider_with_no_declared_method_takes_a_plain_key() {
        for v in [Value::Null, json!({}), json!([])] {
            let ms = parse_methods(&v["anthropic"]);
            assert_eq!(ms.len(), 1);
            assert_eq!(ms[0].kind, "api");
            assert!(ms[0].prompts.is_empty());
        }
    }

    #[test]
    fn a_select_and_its_dependent_field_survive_parsing() {
        let m = method(COPILOT_METHODS, 0);
        assert_eq!(m.prompts.len(), 2);
        assert_eq!(m.prompts[0].kind, "select");
        assert_eq!(m.prompts[0].options.len(), 2);
        assert_eq!(m.prompts[0].options[1].hint.as_deref(), Some("Data residency or self-hosted"));
        assert_eq!(m.prompts[1].kind, "text");
        assert_eq!(m.prompts[1].placeholder.as_deref(), Some("company.ghe.com"));
        let when = m.prompts[1].when.clone().expect("a condition");
        assert_eq!((when.key.as_str(), when.op.as_str(), when.value.as_str()),
                   ("deploymentType", "eq", "enterprise"));
    }

    #[test]
    fn hidden_answers_are_dropped_on_the_way_out() {
        let m = method(COPILOT_METHODS, 0);

        let enterprise = answers(&[("deploymentType", "enterprise"), ("enterpriseUrl", "acme.ghe.com")]);
        assert_eq!(visible_answers(&m, &enterprise), enterprise);

        let switched_back = answers(&[("deploymentType", "github.com"), ("enterpriseUrl", "acme.ghe.com")]);
        assert_eq!(visible_answers(&m, &switched_back), answers(&[("deploymentType", "github.com")]));

        // Unanswered reads as empty, so `eq` hides and the field waits.
        assert!(visible_answers(&m, &answers(&[])).is_empty());
    }

    #[test]
    fn answers_the_method_never_asked_for_do_not_travel() {
        let m = method(COPILOT_METHODS, 0);
        let padded = answers(&[("deploymentType", "github.com"), ("key", "sk-something"), ("blank", "")]);
        assert_eq!(visible_answers(&m, &padded), answers(&[("deploymentType", "github.com")]));
    }

    #[test]
    fn the_credential_body_is_the_key_and_nothing_else() {
        assert_eq!(
            api_credential("sk-live", &answers(&[])),
            json!({ "type": "api", "key": "sk-live" }),
        );
        assert_eq!(
            api_credential("cf-token", &answers(&[("accountId", "abc123")])),
            json!({ "type": "api", "key": "cf-token", "metadata": { "accountId": "abc123" } }),
        );
    }

    #[test]
    fn an_error_cannot_carry_the_key_back_out() {
        let msg = redact(
            "opencode /auth/openai: HTTP 400 invalid key sk-proj-abcdef123456",
            "sk-proj-abcdef123456",
        );
        assert!(!msg.contains("sk-proj"), "{msg}");
        assert!(msg.contains("[redacted]"));
        assert_eq!(redact("cannot reach opencode", "abc"), "cannot reach opencode");
    }

    #[test]
    fn a_connected_providers_key_never_leaves_rust() {
        let all = json!({
            "connected": ["xai"],
            "default": {},
            "all": [{ "id": "xai", "name": "xAI", "source": "api",
                      "env": ["XAI_API_KEY"], "key": "xai-SECRETVALUE12345",
                      "options": {}, "models": { "grok": {} } }]
        });
        let rows = parse_providers(&all, &Value::Null).unwrap();
        assert!(rows[0].connected);
        assert_eq!(rows[0].source, "api");
        let wire = serde_json::to_string(&rows).unwrap();
        assert!(!wire.contains("SECRETVALUE"), "{wire}");
    }

    #[test]
    fn providers_merge_their_connected_state_and_their_forms() {
        let all = json!({
            "connected": ["opencode", "tss-nvidia-spark"],
            "default": {},
            "all": [
                { "id": "openai", "name": "OpenAI", "source": "custom",
                  "env": ["OPENAI_API_KEY"], "options": {}, "models": { "a": {}, "b": {} } },
                { "id": "tss-nvidia-spark", "name": "TSS NVIDIA Spark", "source": "config",
                  "env": [], "options": {}, "models": { "a": {} } },
                { "id": "anthropic", "name": "Anthropic", "source": "custom",
                  "env": ["ANTHROPIC_API_KEY"], "options": {}, "models": {} }
            ]
        });
        let methods: Value = serde_json::from_str(&format!("{{\"openai\": {OPENAI_METHODS}}}")).unwrap();
        let rows = parse_providers(&all, &methods).unwrap();

        assert_eq!(rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
                   vec!["anthropic", "openai", "tss-nvidia-spark"]);

        let openai = &rows[1];
        assert!(!openai.connected);
        assert_eq!(openai.model_count, 2);
        assert_eq!(openai.methods.len(), 3);

        let anthropic = &rows[0];
        assert_eq!(anthropic.methods, vec![default_method()], "no entry means a plain key");

        let spark = &rows[2];
        assert!(spark.connected);
        assert_eq!(spark.source, "config", "declared, not signed in to");
    }

    /// `/config/providers` as 1.18.2 answers: providers, each with a map of
    /// models keyed by id.
    #[test]
    fn the_model_list_is_every_configured_provider_s_models() {
        let v = json!({
            "default": {},
            "providers": [
                { "id": "openrouter", "models": {
                    "aion-labs/aion-2.0": {
                        "id": "aion-labs/aion-2.0", "name": "Aion-2.0",
                        "status": "active", "limit": { "context": 131072 }, "variants": {} },
                    "old/thing": {
                        "id": "old/thing", "name": "Old", "status": "deprecated",
                        "limit": { "context": 8192 }, "variants": {} },
                }},
                { "id": "tss-nvidia-spark", "models": {
                    "nvidia/Qwen3.6-35B-A3B-NVFP4": {
                        "id": "nvidia/Qwen3.6-35B-A3B-NVFP4", "name": "Qwen3.6 35B",
                        "limit": { "context": 262144 },
                        "variants": { "high": {}, "low": {} } },
                }},
                // No models at all: skipped, not an error.
                { "id": "anthropic", "models": {} },
            ]
        });

        let models = parse_models(&v).unwrap();

        assert_eq!(
            models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            vec![
                "openrouter/aion-labs/aion-2.0",
                "tss-nvidia-spark/nvidia/Qwen3.6-35B-A3B-NVFP4",
            ],
            "deprecated is dropped, and an id keeps every slash it came with",
        );
        assert_eq!(models[0].display_name, "Aion-2.0");
        assert_eq!(models[0].description, "131K context");
        assert!(models[0].variants.is_empty(), "an empty object is no levels");

        assert_eq!(models[1].variants, vec!["high".to_string(), "low".to_string()]);
        assert_eq!(models[1].default_variant.as_deref(), Some("high"));
    }

    #[test]
    fn the_default_level_is_high_or_the_nearest_below_it() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        assert_eq!(default_variant(&v(&["low", "medium", "high"])).as_deref(), Some("high"));
        assert_eq!(default_variant(&v(&["low", "medium", "xhigh"])).as_deref(), Some("medium"));
        assert_eq!(default_variant(&v(&["max", "xhigh"])).as_deref(), Some("max"));
        assert_eq!(default_variant(&[]), None);
    }


    // ── The real thing ───────────────────────────────────────────────────────

    struct Kill(Arc<OpencodeServer>);

    impl Drop for Kill {
        fn drop(&mut self) {
            self.0.kill();
        }
    }

    fn one_turn(rx: &mpsc::Receiver<HarnessEvent>, budget: Duration) -> Vec<HarnessEvent> {
        let deadline = Instant::now() + budget;
        let mut out = Vec::new();
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(ev) => {
                    let done = matches!(ev, HarnessEvent::TurnFinished { .. });
                    out.push(ev);
                    if done {
                        return out;
                    }
                }
                Err(_) => panic!("no TurnFinished within {}s; got {out:#?}", budget.as_secs()),
            }
        }
    }

    /// The endpoints themselves: a tool turn, a rewind, a prompt on another
    /// model, and a stop. Needs opencode, OpenRouter signed in, and about a
    /// cent of tokens:
    ///
    /// ```text
    /// cargo test --lib harness::opencode::tests::a_real_opencode_runs -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs opencode on PATH and OpenRouter signed in — see the doc comment"]
    fn a_real_opencode_runs_a_v1_thread_end_to_end() {
        const TOOL_MODEL: &str = "openrouter/openai/gpt-4.1-mini";
        const TEXT_MODEL: &str = "openrouter/openai/gpt-4.1-nano";
        let bin = match super::super::discover::binary(Provider::Opencode) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skipped: no opencode binary — {e}");
                return;
            }
        };
        let dir = crate::test_support::Scratch::new("opencode-thread");
        let directory = dir.join("agents");
        write_config(
            &directory,
            &dir,
            "You are a test agent. Do exactly what the message asks, briefly.",
            &OneOffPrompts {
                naming: "You name conversations.",
                writer: "You complete notes.",
            },
        )
        .expect("the agent config");
        let server = OpencodeServer::spawn(OpencodeSpawn {
            bin,
            directory,
            env: super::super::discover::child_env(),
            raw_log: None,
            default_sink: None,
        })
        .expect("opencode serve");
        let _kill = Kill(server.clone());

        let (tx, rx) = mpsc::channel::<HarnessEvent>();
        let sink: Sink = Arc::new(move |ev| {
            let _ = tx.send(ev);
        });
        let opts = |model: &str| OpencodeSessionOpts {
            model: Some(model.to_string()),
            variant: None,
            brief: String::new(),
            agent: AGENT,
        };

        // 1. A tool turn.
        let session = server.start_session(&opts(TOOL_MODEL), sink.clone()).expect("a session");
        assert!(session.starts_with("ses_"), "{session}");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(1)), Ok(HarnessEvent::SessionStarted { .. })));
        let started = Instant::now();
        server
            .prompt(&session, "Use the bash tool to run `ls`, then say one file name you saw.")
            .expect("prompt_async");
        let events = one_turn(&rx, Duration::from_secs(120));
        eprintln!("tool turn: {} events in {:.1}s", events.len(), started.elapsed().as_secs_f64());
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::TurnStarted)), 1);
        let anchors: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                HarnessEvent::TurnAnchor { anchor } => Some(anchor.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(anchors.len(), 1, "{anchors:?}");
        assert!(anchors[0].starts_with("msg_"), "{anchors:?}");
        let anchor = anchors[0].to_string();
        assert_eq!(
            count(&events, |e| matches!(e, HarnessEvent::ToolStarted { name, .. } if name == "bash")),
            1,
            "{events:#?}"
        );
        assert_eq!(count(&events, |e| matches!(e, HarnessEvent::ToolFinished { ok: true, .. })), 1);
        assert!(!messages(&events).is_empty(), "{events:#?}");
        assert_eq!(finishes(&events), vec!["completed"]);
        assert!(errors(&events).is_empty(), "{:?}", errors(&events));
        assert!(!server.busy(), "the turn is closed");
        eprintln!("  anchor {anchor}, answer {:?}", messages(&events));

        // 2. Rewind to that question (inclusive).
        server.revert(&session, &anchor).expect("revert");
        eprintln!("  revert {anchor}: ok");

        // 3. Another model: the one asked for on attach wins.
        server.attach_session(&session, &opts(TEXT_MODEL), sink.clone()).expect("attach");
        assert!(matches!(rx.recv_timeout(Duration::from_secs(1)), Ok(HarnessEvent::SessionStarted { model: Some(m), .. }) if m == TEXT_MODEL));
        let started = Instant::now();
        server.prompt(&session, "Reply with the single word: ok").expect("prompt_async");
        let events = one_turn(&rx, Duration::from_secs(60));
        eprintln!("text turn after revert: {} events in {:.1}s, answer {:?}", events.len(), started.elapsed().as_secs_f64(), messages(&events));
        assert_eq!(finishes(&events), vec!["completed"]);
        assert!(errors(&events).is_empty(), "{:?}", errors(&events));

        // 4. Stop a long answer mid-stream.
        let started = Instant::now();
        server.prompt(&session, "Count from 1 to 300, one per line").expect("prompt_async");
        let mut before = Vec::new();
        loop {
            let ev = rx.recv_timeout(Duration::from_secs(60)).expect("the turn to start streaming");
            let go = matches!(ev, HarnessEvent::AssistantDelta { .. } | HarnessEvent::ToolStarted { .. });
            let over = matches!(ev, HarnessEvent::TurnFinished { .. });
            before.push(ev);
            assert!(!over, "the turn finished before it could be stopped: {before:#?}");
            if go {
                break;
            }
        }
        server.interrupt(&session).expect("abort");
        let mut events = before;
        events.extend(one_turn(&rx, Duration::from_secs(60)));
        eprintln!("interrupted turn: {} events in {:.1}s", events.len(), started.elapsed().as_secs_f64());
        assert_eq!(finishes(&events), vec!["interrupted"], "{events:#?}");
        assert!(errors(&events).is_empty(), "stopping is not an error: {:?}", errors(&events));
        assert!(!server.busy());

        server.delete_session(&session);
    }

    #[test]
    fn a_stray_is_our_own_argv_that_launchd_has_adopted() {
        let ours = format!("serve {}", SERVE_ARGS[1..].join(" "));
        let listing = format!(
            "\
  4011     1   501 /Users/s/.opencode/bin/opencode {ours}
  4012 54983   501 /Users/s/.opencode/bin/opencode {ours}
  4013     1   501 /Users/s/.opencode/bin/opencode serve
  4014     1   501 /Users/s/.opencode/bin/opencode serve --port 4096
  4015     1   501 /opt/homebrew/bin/opencode tui
  4016     1     0 /Users/root/.opencode/bin/opencode {ours}
  4017     1   501 /Users/some one/.opencode/bin/opencode {ours}
  4018     1   501 /Users/s/.bun/bin/opencodex {ours}
"
        );
        assert_eq!(strays(&listing, 501), vec![4011, 4017]);
    }
}
