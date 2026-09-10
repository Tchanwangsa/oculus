//! CLI agents as the app's chat: Claude Code and Codex, driven as
//! subprocesses the user has already signed in to.
//!
//! This is the bb shape (get-bb/bb) with its plugin system taken out: one
//! bridge per provider that owns a process and folds its dialect into one
//! event stream ([`event::HarnessEvent`]), a manager that persists that
//! stream and forwards it to the webview, and a timeline that only ever
//! sees the normalized events. No API keys are involved — the CLIs carry
//! their own subscriptions, which is the whole reason for driving them
//! rather than the APIs (`docs/harness.md`).
//!
//! Every thread runs from the library's `agents/` folder, not the library
//! root. That one choice is the containment model: Claude's `acceptEdits`
//! only auto-approves edits inside the cwd and, with prompts routed to
//! `none`, refuses the rest; Codex's `workspace-write` sandbox makes the
//! cwd its only writable root at the OS level. Both were measured refusing
//! a write to `../courses/` and accepting one to `memories/`. The rest of
//! the library is readable through `..`, and the appended instructions
//! (`templates/HARNESS.template.md`) say where everything is.
//!
//! Every raw line a provider emits is also appended to
//! `agents/threads/<id>.ndjson`. It costs nothing, it is how a translation
//! bug gets diagnosed without re-running an agent, and the recordings under
//! `fixtures/harness/` that the bridge tests replay came from exactly this.

pub mod claude;
pub mod codex;
pub mod discover;
pub mod event;
pub mod store;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};

use serde::{Deserialize, Serialize};

use claude::{ClaudeSession, ClaudeSpawn};
use codex::{CodexServer, CodexSpawn, CodexThreadOpts, ModelInfo};
pub use event::{HarnessEvent, Provider};

/// Where a bridge hands its events. Called from the bridge's reader thread,
/// in stream order; must not block on the bridge.
pub type Sink = Arc<dyn Fn(HarnessEvent) + Send + Sync>;

const INSTRUCTIONS_TEMPLATE: &str = include_str!("../../templates/HARNESS.template.md");

/// The thread's working directory: the library's `agents/` folder. See the
/// module docs for why this and not the root.
pub fn thread_cwd(data_dir: &Path) -> PathBuf {
    crate::agents::agents_dir(data_dir)
}

/// The instructions appended to the provider's own system prompt, with the
/// library's real paths in them.
///
/// `scope` is the thread's subject folder, when it has one. It does not
/// narrow what the agent may reach — every thread reads the whole library and
/// writes only to `agents/` — it says which subject the questions are about,
/// so "what's due this week" has an answer. A general thread passes None and
/// gets the library-wide instructions unchanged.
pub fn instructions(data_dir: &Path, scope: Option<&str>) -> String {
    let mut courses: Vec<String> = std::fs::read_dir(data_dir.join("courses"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .filter(|n| !n.starts_with('.'))
                .collect()
        })
        .unwrap_or_default();
    courses.sort();
    let courses = if courses.is_empty() {
        "none synced yet".to_string()
    } else {
        courses.iter().map(|c| format!("`{c}`")).collect::<Vec<_>>().join(", ")
    };
    let base = INSTRUCTIONS_TEMPLATE
        .replace("{{DATA_DIR}}", &data_dir.display().to_string())
        .replace("{{COURSES}}", &courses);
    match scope {
        None => base,
        Some(code) => format!(
            "{base}\n\n## This conversation\n\n             It is scoped to **{code}** — the folder `../courses/{code}/`. Unless the              student names another subject, answer from that folder, and pass `{code}`              as the subject to the CLI. Read its `AGENTS.md` for the layout, and its              `agents/memories/` for what you have already learned about it; a fact worth              keeping from this conversation belongs there rather than in `./memories/`.\n"
        ),
    }
}

/// Append-only file of raw provider lines for one thread.
#[derive(Clone)]
pub struct RawLog(Arc<Mutex<std::fs::File>>);

impl RawLog {
    pub fn open(data_dir: &Path, thread_id: i64) -> Option<RawLog> {
        let dir = thread_cwd(data_dir).join("threads");
        std::fs::create_dir_all(&dir).ok()?;
        let f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{thread_id}.ndjson")))
            .ok()?;
        Some(RawLog(Arc::new(Mutex::new(f))))
    }

    pub fn write(&self, line: &str) {
        use std::io::Write;
        if let Ok(mut f) = self.0.lock() {
            let _ = f.write_all(line.as_bytes()).and_then(|_| f.write_all(b"\n"));
        }
    }
}

/// What a send asks for beyond the text. Persisted on the thread once
/// chosen; a later send with a different model changes the thread's model
/// from then on (Claude honours it on the next process, Codex per turn).
#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendOptions {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// The subject the thread is scoped to, from the composer's picker. Only
    /// read when the send creates the thread — afterwards the thread's own
    /// row is the authority, because both CLIs bind the appended instructions
    /// at session start and a re-scope would not reach a live session.
    pub subject_id: Option<i64>,
    /// That subject's folder name, resolved from the thread row before the
    /// send reaches a bridge. Not part of the webview's payload: it is looked
    /// up here so the instructions can name a folder that exists.
    #[serde(skip)]
    pub scope: Option<String>,
}

/// Every reasoning level either CLI accepts, mirrored by `REASONING_LABELS`
/// in `app/src/lib/harness.ts`. Codex declares a subset per model and Claude
/// takes the five `--effort` names; the union is checked here so an unknown
/// string is rejected before it reaches an argv or a Codex config, where it
/// would fail the whole turn with a much worse message.
const REASONING_EFFORTS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

fn validate_effort(value: Option<String>) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(v) if REASONING_EFFORTS.contains(&v.as_str()) => Ok(Some(v)),
        Some(v) => Err(format!("unknown reasoning effort: {v}")),
    }
}

/// A running provider process bound to one thread.
enum Live {
    Claude {
        session: Arc<ClaudeSession>,
        /// What `--effort` this process was spawned with. Claude fixes it for
        /// the life of the process, so changing the level has to respawn.
        effort: Option<String>,
    },
    Codex {
        server: Arc<CodexServer>,
        thread_id: String,
        opts: CodexThreadOpts,
    },
}

impl Live {
    fn is_alive(&self) -> bool {
        match self {
            Live::Claude { session, .. } => session.is_alive(),
            Live::Codex { server, thread_id, .. } => server.is_alive() && server.has_thread(thread_id),
        }
    }

    /// The reasoning level this session is already running under.
    fn effort(&self) -> Option<&str> {
        match self {
            Live::Claude { effort, .. } => effort.as_deref(),
            Live::Codex { opts, .. } => opts.reasoning_effort.as_deref(),
        }
    }
}

/// The set of live sessions plus the shared Codex server. One per app.
pub struct Harness {
    data_dir: PathBuf,
    live: Mutex<HashMap<i64, Live>>,
    codex: Mutex<Option<Arc<CodexServer>>>,
}

impl Harness {
    pub fn new(data_dir: PathBuf) -> Self {
        Harness {
            data_dir,
            live: Mutex::new(HashMap::new()),
            codex: Mutex::new(None),
        }
    }

    /// The shared Codex server, started on first use.
    fn codex_server(&self) -> Result<Arc<CodexServer>, String> {
        let mut slot = self.codex.lock().unwrap();
        if let Some(s) = slot.as_ref().filter(|s| s.is_alive()) {
            return Ok(s.clone());
        }
        let bin = discover::binary(Provider::Codex)?;
        let server = CodexServer::spawn(CodexSpawn {
            bin,
            env: discover::child_env(),
            raw_log: RawLog::open(&self.data_dir, 0),
        })?;
        *slot = Some(server.clone());
        Ok(server)
    }

    pub fn codex_models(&self) -> Result<Vec<ModelInfo>, String> {
        self.codex_server()?.list_models()
    }

    /// Bring a thread's session up if it is not, then send. `resume` is the
    /// provider's session id from a previous process, if any.
    pub fn send(
        &self,
        thread_id: i64,
        provider: Provider,
        resume: Option<&str>,
        opts: &SendOptions,
        text: &str,
        sink: Sink,
    ) -> Result<(), String> {
        let mut live = self.live.lock().unwrap();
        // A live session is reused only if it is running under the level this
        // send asks for; both CLIs bind the level at session start, so a new
        // one means a new process (Claude) or a new thread (Codex).
        if let Some(l) = live
            .get(&thread_id)
            .filter(|l| l.is_alive() && l.effort() == opts.reasoning_effort.as_deref())
        {
            return match l {
                Live::Claude { session, .. } => session.send(text),
                Live::Codex {
                    server,
                    thread_id: tid,
                    opts,
                } => server.start_turn(tid, text, opts),
            };
        }
        live.remove(&thread_id);

        let cwd = thread_cwd(&self.data_dir);
        std::fs::create_dir_all(&cwd).map_err(|e| format!("cannot create {}: {e}", cwd.display()))?;
        let raw_log = RawLog::open(&self.data_dir, thread_id);
        let session = match provider {
            Provider::Claude => {
                let s = ClaudeSession::spawn(
                    ClaudeSpawn {
                        bin: discover::binary(provider)?,
                        cwd,
                        library: self.data_dir.clone(),
                        resume: resume.map(String::from),
                        model: opts.model.clone(),
                        effort: opts.reasoning_effort.clone(),
                        permission_mode: "acceptEdits".into(),
                        system_append: instructions(&self.data_dir, opts.scope.as_deref()),
                        env: discover::child_env(),
                        raw_log,
                    },
                    sink,
                )?;
                s.send(text)?;
                Live::Claude {
                    session: s,
                    effort: opts.reasoning_effort.clone(),
                }
            }
            Provider::Codex => {
                let server = self.codex_server()?;
                let topts = CodexThreadOpts {
                    cwd,
                    model: opts.model.clone(),
                    reasoning_effort: opts.reasoning_effort.clone(),
                    instructions: instructions(&self.data_dir, opts.scope.as_deref()),
                };
                let tid = match resume {
                    Some(id) => {
                        server.resume_thread(id, &topts, sink)?;
                        id.to_string()
                    }
                    None => server.start_thread(&topts, sink)?,
                };
                server.start_turn(&tid, text, &topts)?;
                Live::Codex {
                    server,
                    thread_id: tid,
                    opts: topts,
                }
            }
        };
        live.insert(thread_id, session);
        Ok(())
    }

    pub fn interrupt(&self, thread_id: i64) -> Result<(), String> {
        let live = self.live.lock().unwrap();
        match live.get(&thread_id) {
            Some(Live::Claude { session, .. }) => session.interrupt(),
            Some(Live::Codex { server, thread_id, .. }) => server.interrupt(thread_id),
            None => Ok(()),
        }
    }

    /// End the thread's process. The thread row stays; the next send
    /// resumes it by session id.
    pub fn close(&self, thread_id: i64) {
        if let Some(l) = self.live.lock().unwrap().remove(&thread_id) {
            match l {
                Live::Claude { session, .. } => session.kill(),
                Live::Codex { server, thread_id, .. } => server.detach(&thread_id),
            }
        }
    }

    pub fn is_live(&self, thread_id: i64) -> bool {
        self.live.lock().unwrap().get(&thread_id).map_or(false, |l| l.is_alive())
    }

    /// Everything, on quit.
    pub fn shutdown(&self) {
        let ids: Vec<i64> = self.live.lock().unwrap().keys().copied().collect();
        for id in ids {
            self.close(id);
        }
        if let Some(s) = self.codex.lock().unwrap().take() {
            s.kill();
        }
    }
}

// ── Headless ─────────────────────────────────────────────────────────────────

/// One prompt, one turn, events to `on_event`, then the process is gone.
/// What `oculus agent` runs; also the smallest end-to-end test of a bridge.
pub fn run_once(
    data_dir: &Path,
    provider: Provider,
    opts: &SendOptions,
    prompt: &str,
    on_event: impl Fn(&HarnessEvent) + Send + Sync + 'static,
) -> Result<(), String> {
    let harness = Harness::new(data_dir.to_path_buf());
    let (tx, rx) = mpsc::channel::<HarnessEvent>();
    let sink: Sink = Arc::new(move |ev| {
        let _ = tx.send(ev);
    });
    // Thread id 0 in the log dir: a headless run is not a thread.
    harness.send(0, provider, None, opts, prompt, sink)?;

    let mut failed: Option<String> = None;
    for ev in rx {
        on_event(&ev);
        match &ev {
            HarnessEvent::Error { message } => failed = Some(message.clone()),
            HarnessEvent::TurnFinished { .. } => break,
            HarnessEvent::Exited { code } => {
                failed.get_or_insert(format!("provider exited (code {code:?}) before finishing"));
                break;
            }
            _ => {}
        }
    }
    harness.shutdown();
    match failed {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// ── Tauri ────────────────────────────────────────────────────────────────────


pub mod app {
    use super::*;
    use sqlx::SqlitePool;
    use tauri::{AppHandle, Emitter, Manager, State};

    /// What the webview gets on `harness-event`: the thread and the event,
    /// plus the row id when the event became a row.
    #[derive(Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Envelope {
        thread_id: i64,
        item_id: Option<i64>,
        event: HarnessEvent,
    }

    pub struct HarnessState {
        pub harness: Arc<Harness>,
        bus: mpsc::Sender<(i64, Provider, HarnessEvent)>,
    }

    /// One consumer thread folds every event, from every thread, in order:
    /// a row is written before the webview hears about it, and a tool's
    /// finish can never overtake its start.
    pub fn init(app: &AppHandle) -> HarnessState {
        let (tx, rx) = mpsc::channel::<(i64, Provider, HarnessEvent)>();
        let handle = app.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            let mut pool: Option<SqlitePool> = None;
            for (thread_id, provider, ev) in rx {
                if pool.is_none() {
                    pool = rt.block_on(crate::llm::open_pool()).ok();
                }
                let mut item_id = None;
                if let Some(p) = &pool {
                    if thread_id > 0 {
                        match rt.block_on(store::apply(p, thread_id, &ev)) {
                            Ok(id) => item_id = id,
                            Err(e) => eprintln!("[oculus] harness store: {e}"),
                        }
                    }
                    if let Err(e) = rt.block_on(store::save_rate_limits(p, provider, &ev)) {
                        eprintln!("[oculus] harness rate limits: {e}");
                    }
                }
                let _ = handle.emit(
                    "harness-event",
                    Envelope {
                        thread_id,
                        item_id,
                        event: ev,
                    },
                );
            }
        });
        HarnessState {
            harness: Arc::new(Harness::new(crate::paths::data_dir())),
            bus: tx,
        }
    }

    impl HarnessState {
        fn sink(&self, thread_id: i64, provider: Provider) -> Sink {
            let bus = self.bus.clone();
            Arc::new(move |ev| {
                let _ = bus.send((thread_id, provider, ev));
            })
        }
    }

    #[tauri::command]
    pub async fn harness_health() -> Vec<discover::BridgeHealth> {
        tokio::task::spawn_blocking(|| {
            discover::forget();
            vec![discover::health(Provider::Claude), discover::health(Provider::Codex)]
        })
        .await
        .unwrap_or_default()
    }

    #[tauri::command]
    pub async fn harness_codex_models(state: State<'_, HarnessState>) -> Result<Vec<ModelInfo>, String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.codex_models())
            .await
            .map_err(|e| e.to_string())?
    }

    /// Send a message; creates the thread when `thread_id` is null. Returns
    /// the thread id. The user's row is written by the event path like every
    /// other row, so the timeline sees it in order with what follows.
    #[tauri::command]
    pub async fn harness_send(
        state: State<'_, HarnessState>,
        thread_id: Option<i64>,
        provider: String,
        text: String,
        options: Option<SendOptions>,
    ) -> Result<i64, String> {
        let provider = Provider::parse(&provider).ok_or_else(|| format!("unknown provider {provider}"))?;
        let mut opts = options.unwrap_or_default();
        opts.reasoning_effort = validate_effort(opts.reasoning_effort)?;
        let pool = crate::llm::open_pool().await?;

        let (id, resume) = match thread_id {
            Some(id) => {
                let row = store::thread(&pool, id).await?;
                if row.provider != provider {
                    return Err(format!("thread {id} is a {} thread", row.provider.label()));
                }
                if opts.model.is_some() && opts.model != row.model {
                    store::set_model(&pool, id, opts.model.as_deref()).await?;
                    // A different model means a different Claude process.
                    if provider == Provider::Claude {
                        state.harness.close(id);
                    }
                }
                (id, row.provider_session_id)
            }
            None => (
                store::create_thread(&pool, provider, opts.model.as_deref(), opts.subject_id, &text)
                    .await?,
                None,
            ),
        };
        // The row, not the payload, decides both: an open thread keeps the
        // model it was last set to and the subject it was created with.
        let row = store::thread(&pool, id).await?;
        let opts = SendOptions {
            model: opts.model.or(row.model),
            scope: row.subject_code,
            ..opts
        };

        let sink = state.sink(id, provider);
        sink(HarnessEvent::UserMessage { text: text.clone() });

        let h = state.harness.clone();
        let sink2 = sink.clone();
        let res = tokio::task::spawn_blocking(move || h.send(id, provider, resume.as_deref(), &opts, &text, sink2))
            .await
            .map_err(|e| e.to_string())?;
        if let Err(e) = res {
            sink(HarnessEvent::error(e.clone()));
            sink(HarnessEvent::TurnFinished {
                status: "failed".into(),
            });
            return Err(e);
        }
        Ok(id)
    }

    #[tauri::command]
    pub async fn harness_interrupt(state: State<'_, HarnessState>, thread_id: i64) -> Result<(), String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.interrupt(thread_id))
            .await
            .map_err(|e| e.to_string())?
    }

    #[tauri::command]
    pub async fn harness_delete_thread(state: State<'_, HarnessState>, thread_id: i64) -> Result<(), String> {
        state.harness.close(thread_id);
        let pool = crate::llm::open_pool().await?;
        store::delete_thread(&pool, thread_id).await
    }

    /// Startup: nothing survives a restart as `running`.
    pub fn reconcile(app: &AppHandle) {
        let _ = app;
        tauri::async_runtime::spawn(async {
            if let Ok(pool) = crate::llm::open_pool().await {
                if let Ok(n) = store::reconcile(&pool).await {
                    if n > 0 {
                        eprintln!("[oculus] harness: marked {n} interrupted thread(s) idle");
                    }
                }
            }
        });
    }

    pub fn shutdown(app: &AppHandle) {
        if let Some(s) = app.try_state::<HarnessState>() {
            s.harness.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scope is appended, not substituted: a scoped thread gets the whole
    /// library brief *and* the subject it is about, because it still reads
    /// across `courses/` and still writes only to `agents/`.
    #[test]
    fn a_scoped_thread_keeps_the_library_brief_and_names_its_folder() {
        let root = std::env::temp_dir().join("oculus-harness-scope");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        let general = instructions(&root, None);
        assert!(general.contains("`COMP30026_2026_SM2`"), "the course list is filled in");
        assert!(!general.contains("This conversation"), "no scope section on a general thread");

        let scoped = instructions(&root, Some("COMP30026_2026_SM2"));
        assert!(scoped.starts_with(&general), "the scope is appended to the same brief");
        assert!(scoped.contains("`../courses/COMP30026_2026_SM2/`"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
