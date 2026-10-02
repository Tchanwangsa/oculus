//! CLI agents as the app's chat: Claude Code, Codex, opencode and
//! Antigravity (`agy`), driven as subprocesses the user has signed in to.
//!
//! One bridge per provider folds its dialect into one event stream
//! ([`event::HarnessEvent`]); the [`Harness`] owns the live sessions, and
//! [`app`] persists the stream and forwards it to the webview.
//!
//! Every thread runs from the library's `agents/` folder — that is the
//! containment model; see `docs/harness.md` for what each bridge adds.
//!
//! Every raw line a provider emits is appended to
//! `agents/threads/<id>.ndjson`; the replay fixtures under
//! `fixtures/harness/` came from there.

pub mod antigravity;
pub mod antigravity_rules;
pub mod attach;
mod child;
pub mod claude;
pub mod codex;
pub mod discover;
pub mod event;
pub mod install;
pub mod jobs;
pub mod opencode;
mod protected;
pub mod signin;
pub mod store;
mod suggest;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use serde::{Deserialize, Serialize};

use antigravity::{AntigravitySession, AntigravitySpawn};
use child::ThreadSpawn;
use claude::{ClaudeSession, ClaudeSpawn};
use codex::{CodexServer, CodexSpawn, CodexThreadOpts, ModelInfo};
pub use event::{HarnessEvent, Provider, ToolKind};
use opencode::{OpencodeServer, OpencodeSessionOpts, OpencodeSpawn, ProviderList};

/// Where a bridge hands its events. Called from the bridge's reader thread,
/// in stream order; must not block on the bridge.
pub type Sink = Arc<dyn Fn(HarnessEvent) + Send + Sync>;

const INSTRUCTIONS_TEMPLATE: &str = include_str!("../../templates/HARNESS.template.md");

/// The thread's working directory: the library's `agents/` folder.
pub fn thread_cwd(data_dir: &Path) -> PathBuf {
    crate::agents::agents_dir(data_dir)
}

/// The instructions appended to the provider's own system prompt, with the
/// library's real paths in them. `scope` (the subject folder) and `lecture`
/// only say what the questions are about — they never narrow what the agent
/// may reach — and are appended to the library-wide brief, not substituted.
pub fn instructions(
    data_dir: &Path,
    scope: Option<&str>,
    lecture: Option<&LectureBrief>,
) -> String {
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
        courses
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let base = INSTRUCTIONS_TEMPLATE
        .replace("{{DATA_DIR}}", &data_dir.display().to_string())
        .replace("{{COURSES}}", &courses);
    format!("{base}{}", thread_sections(scope, lecture))
}

/// The part of the brief about *this thread* (subject, lecture). Split out
/// for the bridges with no per-thread system prompt — opencode and
/// Antigravity — which send it ahead of the session's first message.
pub fn thread_sections(scope: Option<&str>, lecture: Option<&LectureBrief>) -> String {
    let mut out = String::new();
    if let Some(code) = scope {
        out.push_str(&format!(
            "\n\n## This conversation\n\n\
             It is scoped to **{code}** — the folder `../courses/{code}/`. Unless the \
             student names another subject, answer from that folder, and pass `{code}` \
             as the subject to the CLI. Read its `AGENTS.md` for the layout, and run \
             `oculus memory list -s {code}` for what you already know about it — that \
             bucket, and not the cross-subject one, is where a fact from this \
             conversation belongs, so `-s {code}` rides every `oculus memory write` \
             you make here.\n"
        ));
    }
    if let Some(lec) = lecture {
        out.push_str(&lecture_section(lec, scope));
    }
    out
}

/// What the agent is told about the recording the student is watching.
/// Paths are relative to `agents/`, like the rest of the brief. Chapters are
/// inlined (short; fetching them costs a tool call); the transcript is only
/// named. The date, the VTT's shape and the deck-finding commands with their
/// real flags are spelled out because an agent otherwise spends its turn
/// rediscovering them.
fn lecture_section(lec: &LectureBrief, scope: Option<&str>) -> String {
    let dir = format!("../lectures/{}", lec.id);
    let mut s = format!(
        "\n## The lecture being watched\n\n\
         The student is watching **{}**, recorded **{}**. Its recording folder is \
         `{dir}/`. Lecture titles here come from the timetable, so the date is what \
         says which one this is.\n\n",
        lec.title, lec.date
    );
    if lec.has_transcript {
        s.push_str(&format!(
            "- `{dir}/transcript.vtt` — the whole transcript, WebVTT, with timestamps. \
             It is long (an hour of speech) and every cue is followed by a `NOTE CONF` \
             line of recogniser confidence numbers, which is noise — skip those. Do not \
             read it from the top: find the span you want by its timestamp \
             (`grep -n \"00:14:\" {dir}/transcript.vtt` gives you the line number, then \
             read from there).\n"
        ));
    }
    if let Some(code) = scope {
        s.push_str(&format!(
            "- `../courses/{code}/` — the course folder: the slide deck for this lecture, \
             and everything else the subject has.\n"
        ));
        s.push_str(&format!(
            "\nThe deck is not linked to the recording, so finding it is a step: \
             `oculus files {code} --type pdf` lists them, and the date above is what \
             picks the week out of `Lecture_1`, `Lecture_2`… Then `oculus grep \"<a phrase \
             off the slide>\" -s {code}` says which deck and page it is on, and \
             `oculus read <FILE> --pages N` prints that page. Note the flags: `oculus \
             files` takes the subject code as a bare argument, every other command takes \
             it as `-s`, and a file as its only argument.\n"
        ));
    }
    if !lec.chapters.is_empty() {
        s.push_str("\nIts chapters:\n\n");
        for c in &lec.chapters {
            s.push_str(&format!(
                "- {} — {} ({})\n",
                crate::chapters::hms(c.start_seconds),
                c.title,
                c.summary
            ));
        }
    }
    s.push_str(
        "\nThe student is watching this lecture, and a message may carry the moment it was \
         sent at — a timestamp, the last minute of transcript, and a frame of every stream \
         the capture has — appended under a heading after their own words. When it is \
         there, \"this\", \"that slide\" and \"what he just said\" mean that moment. It is \
         usually enough on its own: open every frame and read the transcript it carries \
         before going looking for more, and go to the deck when the question needs the \
         exact notation rather than as a matter of course.\n\
         \nTwo frames are two cameras on the same second, not two moments. Echo360 \
         numbers the streams rather than naming them and either can be the one being \
         taught from: a whiteboard derivation is often only on the room camera while the \
         screen capture holds the theatre's idle splash for the hour, and the slides are \
         only on the capture. Look at all of them before saying a frame shows nothing.\n",
    );
    s
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
            let _ = f
                .write_all(line.as_bytes())
                .and_then(|_| f.write_all(b"\n"));
        }
    }
}

/// What a send asks for beyond the text. The model is persisted on the
/// thread; a later send with a different one changes it from then on.
#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendOptions {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Read only when the send creates the thread; afterwards the row is the
    /// authority, since the brief is bound at session start.
    pub subject_id: Option<i64>,
    /// That subject's folder name, resolved from the thread row.
    #[serde(skip)]
    pub scope: Option<String>,
    /// The recording a dock conversation is about. Read only on creation,
    /// and it also decides the subject (`store::create_thread`).
    pub lecture_id: Option<String>,
    /// That lecture, resolved from the thread row.
    #[serde(skip)]
    pub lecture: Option<LectureBrief>,
    /// The player's moment (timestamp, transcript tail, frames). Appended to
    /// the prompt after the student's text; never part of the row.
    pub context: Option<String>,
    /// The playhead's second at send; see [`HarnessEvent::UserMessage`].
    pub at: Option<i64>,
    /// Antigravity only: the student's approved rules, for the spawn to write
    /// (`antigravity_rules::install`). `None` keeps the last ones written.
    #[serde(skip)]
    pub antigravity_rules: Option<Vec<String>>,
}

/// What a lecture thread's brief says about the recording, assembled from
/// the thread's row and `store::chapters`.
#[derive(Clone)]
pub struct LectureBrief {
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD`: the title is the timetable's, so the date is what says
    /// which week (and slide deck) this is.
    pub date: String,
    pub has_transcript: bool,
    pub chapters: Vec<crate::chapters::Chapter>,
}

/// Every reasoning level any CLI accepts, mirrored by `REASONING_LABELS` in
/// `app/src/lib/harness.ts`. Checked up front so an unknown string never
/// reaches an argv or a Codex config.
const REASONING_EFFORTS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

fn validate_effort(value: Option<String>) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(v) if REASONING_EFFORTS.contains(&v.as_str()) => Ok(Some(v)),
        Some(v) => Err(format!("unknown reasoning effort: {v}")),
    }
}

// ── One turn at a time ───────────────────────────────────────────────────────

/// A message typed while a turn was running, waiting its own.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueuedMessage {
    pub id: String,
    pub text: String,
}

fn next_queue_id() -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    format!("q{}", N.fetch_add(1, Ordering::SeqCst))
}

/// Which threads have a turn open, and what is waiting behind each.
///
/// The CLIs mishandle a message sent mid-turn (`claude` silently chains a
/// second turn; `codex` folds it into the running one), so one turn per
/// thread runs and the rest wait here. A pending message has no row until it
/// goes out, which is why it can still be edited or dropped.
#[derive(Default)]
pub struct Queue {
    threads: HashMap<i64, ThreadQueue>,
}

#[derive(Default)]
struct ThreadQueue {
    /// A turn of ours is open. Released by its `TurnFinished` — every bridge
    /// emits exactly one per message it accepts.
    busy: bool,
    pending: VecDeque<(QueuedMessage, SendOptions)>,
}

impl Queue {
    /// Take the thread for a send. False when a turn already has it.
    pub fn try_claim(&mut self, thread_id: i64) -> bool {
        let q = self.threads.entry(thread_id).or_default();
        if q.busy {
            return false;
        }
        q.busy = true;
        true
    }

    /// Fall in behind the turn that has the thread.
    pub fn push(&mut self, thread_id: i64, text: &str, opts: &SendOptions) -> QueuedMessage {
        let msg = QueuedMessage {
            id: next_queue_id(),
            text: text.to_string(),
        };
        self.threads
            .entry(thread_id)
            .or_default()
            .pending
            .push_back((msg.clone(), opts.clone()));
        msg
    }

    /// The turn ended: the next message waiting, if there is one. The thread
    /// stays claimed when one is handed back — it is about to be sent — and
    /// goes idle when nothing is.
    pub fn next(&mut self, thread_id: i64) -> Option<(QueuedMessage, SendOptions)> {
        let q = self.threads.entry(thread_id).or_default();
        match q.pending.pop_front() {
            Some(next) => Some(next),
            None => {
                q.busy = false;
                None
            }
        }
    }

    /// Everything still waiting, dropped — what stop does. Handed back so
    /// the composer can return them to the student.
    pub fn clear(&mut self, thread_id: i64) -> Vec<QueuedMessage> {
        match self.threads.get_mut(&thread_id) {
            Some(q) => q.pending.drain(..).map(|(m, _)| m).collect(),
            None => Vec::new(),
        }
    }

    /// Drop one pending message.
    pub fn remove(&mut self, thread_id: i64, id: &str) -> bool {
        let Some(q) = self.threads.get_mut(&thread_id) else {
            return false;
        };
        let before = q.pending.len();
        q.pending.retain(|(m, _)| m.id != id);
        q.pending.len() != before
    }

    /// Rewrite one that has not gone out yet.
    pub fn edit(&mut self, thread_id: i64, id: &str, text: &str) -> Option<QueuedMessage> {
        let q = self.threads.get_mut(&thread_id)?;
        let (m, _) = q.pending.iter_mut().find(|(m, _)| m.id == id)?;
        m.text = text.to_string();
        Some(m.clone())
    }

    pub fn list(&self, thread_id: i64) -> Vec<QueuedMessage> {
        match self.threads.get(&thread_id) {
            Some(q) => q.pending.iter().map(|(m, _)| m.clone()).collect(),
            None => Vec::new(),
        }
    }

    /// A turn of ours is open on this thread, or something is waiting behind
    /// one. Anything that rewrites the thread's rows has to wait for both.
    pub fn is_busy(&self, thread_id: i64) -> bool {
        self.threads.get(&thread_id).is_some_and(|q| q.busy)
    }

    pub fn forget(&mut self, thread_id: i64) {
        self.threads.remove(&thread_id);
    }
}

// ── Naming a thread ──────────────────────────────────────────────────────────

/// Long enough for a cold CLI start, short enough that a wedged one does not
/// leave a thread forever "being named".
const NAMING_TIMEOUT_SECS: u64 = 90;

const NAMING_INSTRUCTIONS: &str =
    "You name conversations. Reply with the name alone — never a sentence about it.";

/// How much of the exchange the namer sees. A name comes from what was asked
/// and the shape of the answer; the rest is tokens.
const NAMING_CLIP: usize = 800;

fn naming_prompt(first_message: &str, reply: &str) -> String {
    let clip = |s: &str| -> String {
        let t: String = s.chars().take(NAMING_CLIP).collect();
        if s.chars().count() > NAMING_CLIP {
            format!("{t}…")
        } else {
            t
        }
    };
    format!(
        "Name this conversation between a university student and their study assistant.\n\n\
         Reply with the name and nothing else: three to six words, sentence case, no quotes and \
         no full stop. Name what the conversation is *about* — the topic, the subject, the \
         artefact — not what happened in it. Do not write \"the student asks\" or \"discussion \
         of\".\n\n\
         <student>\n{}\n</student>\n\n<assistant>\n{}\n</assistant>",
        clip(first_message.trim()),
        clip(reply.trim()),
    )
}

/// What survives from a naming reply: the first non-empty line, stripped of
/// labels and quoting. Anything long enough to be prose is refused, so the
/// first-line title stays instead.
fn clean_title(raw: &str) -> Option<String> {
    let line = raw.lines().find(|l| !l.trim().is_empty())?.trim();
    let line = line
        .strip_prefix("Title:")
        .or_else(|| line.strip_prefix("Name:"))
        .unwrap_or(line);
    let line = line
        .trim()
        .trim_matches(|c| matches!(c, '"' | '\'' | '`' | '*' | '#'))
        .trim();
    let line = line.trim_end_matches(['.', '!']).trim();
    if line.is_empty() || line.chars().count() > 60 {
        return None;
    }
    Some(line.to_string())
}

/// One provider session: a process per thread (Claude, Antigravity) or a
/// thread/session id on a shared server (Codex, opencode). Cheap to clone,
/// so it can be lifted out of the live map and talked to without the lock.
#[derive(Clone)]
enum Handle {
    Claude(Arc<ClaudeSession>),
    Codex(Arc<CodexServer>, String, Arc<CodexThreadOpts>),
    Opencode(Arc<OpencodeServer>, String),
    Antigravity(Arc<AntigravitySession>),
}

impl Handle {
    fn provider(&self) -> Provider {
        match self {
            Handle::Claude(_) => Provider::Claude,
            Handle::Codex(..) => Provider::Codex,
            Handle::Opencode(..) => Provider::Opencode,
            Handle::Antigravity(_) => Provider::Antigravity,
        }
    }

    fn is_alive(&self) -> bool {
        match self {
            Handle::Claude(s) => s.is_alive(),
            Handle::Codex(server, tid, _) => server.is_alive() && server.has_thread(tid),
            Handle::Opencode(server, ses) => server.is_alive() && server.has_session(ses),
            Handle::Antigravity(s) => s.is_alive(),
        }
    }

    fn send(&self, text: &str) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.send(text),
            Handle::Codex(server, tid, opts) => server.start_turn(tid, text, opts),
            Handle::Opencode(server, ses) => server.prompt(ses, text),
            Handle::Antigravity(s) => s.send(text),
        }
    }

    /// Antigravity refuses: its protocol has no way back to an earlier message.
    fn rewind(&self, anchor: &str) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.rewind(anchor),
            Handle::Codex(server, tid, _) => server.revert(tid, anchor),
            Handle::Opencode(server, ses) => server.revert(ses, anchor),
            Handle::Antigravity(s) => s.rewind(anchor),
        }
    }

    fn interrupt(&self) -> Result<(), String> {
        match self {
            Handle::Claude(s) => s.interrupt(),
            Handle::Codex(server, tid, _) => server.interrupt(tid),
            Handle::Opencode(server, ses) => server.interrupt(ses),
            Handle::Antigravity(s) => s.interrupt(),
        }
    }

    /// Stop a one-off turn as fast as the provider allows: a process per turn
    /// is killed (its reader closes the turn), a server's turn interrupted.
    fn cancel(&self) -> Result<(), String> {
        match self {
            Handle::Claude(s) => {
                s.kill();
                Ok(())
            }
            Handle::Antigravity(s) => {
                s.kill();
                Ok(())
            }
            Handle::Codex(..) | Handle::Opencode(..) => self.interrupt(),
        }
    }

    /// End this session; a shared server stays up. With `delete`, an opencode
    /// session is removed from the server rather than detached.
    fn close(&self, delete: bool) {
        match self {
            Handle::Claude(s) => s.kill(),
            Handle::Codex(server, tid, _) => server.detach(tid),
            Handle::Opencode(server, ses) if delete => server.delete_session(ses),
            Handle::Opencode(server, ses) => server.detach(ses),
            Handle::Antigravity(s) => s.kill(),
        }
    }
}

/// A thread's live session and the reasoning level it was started under.
/// Every CLI binds the level at session start, so a different one respawns.
struct Live {
    handle: Handle,
    effort: Option<String>,
}

impl Live {
    fn is_alive(&self) -> bool {
        self.handle.is_alive()
    }
}

/// The live sessions plus the shared Codex and opencode servers. One per app.
pub struct Harness {
    data_dir: PathBuf,
    live: Mutex<HashMap<i64, Live>>,
    codex: Mutex<Option<Arc<CodexServer>>>,
    opencode: Mutex<Option<Arc<OpencodeServer>>>,
    /// Codex events with no thread — the account's rate-limit windows.
    /// `None` headless, where nothing is listening.
    codex_account_sink: Mutex<Option<Sink>>,
    /// opencode events that name no session; same role, same thread id 0.
    opencode_default_sink: Mutex<Option<Sink>>,
    /// Claude Code's last catalogue, keyed by the binary's resolved path and
    /// mtime (the list only moves when the CLI does). A sign-in drops it.
    claude_models: Mutex<Option<ClaudeCatalogue>>,
    /// The document editor's suggestion turn in flight and its warm spare.
    suggest: Mutex<suggest::Suggestions>,
}

struct ClaudeCatalogue {
    bin: PathBuf,
    modified: Option<std::time::SystemTime>,
    models: Vec<claude::ModelInfo>,
}

impl Harness {
    pub fn new(data_dir: PathBuf) -> Self {
        Harness {
            data_dir,
            live: Mutex::new(HashMap::new()),
            codex: Mutex::new(None),
            opencode: Mutex::new(None),
            codex_account_sink: Mutex::new(None),
            opencode_default_sink: Mutex::new(None),
            claude_models: Mutex::new(None),
            suggest: Mutex::new(suggest::Suggestions::default()),
        }
    }

    /// Where Codex's account-scoped events go, set once at startup.
    pub fn set_codex_account_sink(&self, sink: Sink) {
        *self.codex_account_sink.lock().unwrap() = Some(sink);
    }

    /// The same, for opencode's events that name no session.
    pub fn set_opencode_default_sink(&self, sink: Sink) {
        *self.opencode_default_sink.lock().unwrap() = Some(sink);
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
            account_sink: self.codex_account_sink.lock().unwrap().clone(),
        })?;
        *slot = Some(server.clone());
        // Seed the rate-limit meter now rather than at the first turn, off
        // the caller's thread (this runs inside the first send).
        {
            let s = server.clone();
            std::thread::spawn(move || {
                if let Err(e) = s.read_rate_limits() {
                    eprintln!("[oculus] codex rate limits: {e}");
                }
            });
        }
        Ok(server)
    }

    /// Re-read the windows on a server that is already up. Never starts one:
    /// a page visit is not a reason to spawn a CLI.
    pub fn refresh_codex_rate_limits(&self) {
        let server = {
            let slot = self.codex.lock().unwrap();
            slot.as_ref().filter(|s| s.is_alive()).cloned()
        };
        if let Some(s) = server {
            if let Err(e) = s.read_rate_limits() {
                eprintln!("[oculus] codex rate limits: {e}");
            }
        }
    }

    pub fn codex_models(&self) -> Result<Vec<ModelInfo>, String> {
        self.codex_server()?.list_models()
    }

    /// Antigravity's catalogue: one short-lived `agy models`, nothing spent.
    pub fn antigravity_models(&self) -> Result<Vec<antigravity::ModelInfo>, String> {
        let bin = discover::binary(Provider::Antigravity)?;
        antigravity::list_models(&bin, &discover::child_env())
    }

    /// Claude Code's catalogue, from a short-lived CLI run the way a thread's
    /// is (`claude::list_models`), cached per binary. The lock is held across
    /// the probe so concurrent callers start one CLI; failures are not cached.
    pub fn claude_models(&self) -> Result<Vec<claude::ModelInfo>, String> {
        let bin = discover::binary(Provider::Claude)?;
        let resolved = std::fs::canonicalize(&bin).unwrap_or_else(|_| bin.clone());
        let modified = std::fs::metadata(&resolved).and_then(|m| m.modified()).ok();
        let mut slot = self.claude_models.lock().unwrap();
        if let Some(c) = slot.as_ref().filter(|c| c.bin == resolved && c.modified == modified) {
            return Ok(c.models.clone());
        }
        let cwd = thread_cwd(&self.data_dir);
        std::fs::create_dir_all(&cwd)
            .map_err(|e| format!("cannot create {}: {e}", cwd.display()))?;
        let models = claude::list_models(&bin, &cwd, &discover::child_env())?;
        *slot = Some(ClaudeCatalogue { bin: resolved, modified, models: models.clone() });
        Ok(models)
    }

    /// A sign-in may change the account, and with it the catalogue.
    pub fn forget_claude_models(&self) {
        *self.claude_models.lock().unwrap() = None;
    }

    /// The shared opencode server, started on first use. Its config (brief
    /// and containment ruleset, `opencode::write_config`) is re-rendered
    /// before every start so neither is stale.
    fn opencode_server(&self) -> Result<Arc<OpencodeServer>, String> {
        let mut slot = self.opencode.lock().unwrap();
        if let Some(s) = slot.as_ref().filter(|s| s.is_alive()) {
            return Ok(s.clone());
        }
        let bin = discover::binary(Provider::Opencode)?;
        let directory = thread_cwd(&self.data_dir);
        opencode::write_config(
            &directory,
            &self.data_dir,
            &instructions(&self.data_dir, None, None),
            &opencode::OneOffPrompts {
                naming: NAMING_INSTRUCTIONS,
                writer: suggest::INSTRUCTIONS,
            },
        )?;
        let server = OpencodeServer::spawn(OpencodeSpawn {
            bin,
            directory,
            env: discover::child_env(),
            raw_log: RawLog::open(&self.data_dir, 0),
            default_sink: self.opencode_default_sink.lock().unwrap().clone(),
        })?;
        *slot = Some(server.clone());
        Ok(server)
    }

    pub fn opencode_models(&self) -> Result<Vec<opencode::ModelInfo>, String> {
        self.opencode_server()?.list_models()
    }

    // ── opencode credentials ─────────────────────────────────────────────
    //
    // Through the server's own auth endpoints, so the credential lands in
    // opencode's store and nowhere else. Each starts the server if it is
    // down: they answer a button press, never a page opening.

    pub fn opencode_providers(&self, refresh: bool) -> Result<ProviderList, String> {
        let server = self.opencode_server()?;
        // Refused during a running turn: the list may then predate a
        // credential written since.
        let stale = refresh && !server.refresh();
        Ok(ProviderList {
            providers: server.list_providers()?,
            stale,
        })
    }

    /// The key is never stored or logged here.
    pub fn opencode_set_api_key(
        &self,
        provider: &str,
        method: usize,
        key: &str,
        answers: &BTreeMap<String, String>,
    ) -> Result<ProviderList, String> {
        let server = self.opencode_server()?;
        let spec = server.auth_method(provider, method)?;
        server.set_api_key(provider, key, &opencode::visible_answers(&spec, answers))?;
        self.opencode_providers(true)
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
        self.ensure(&mut live, thread_id, provider, resume, opts, sink)?;
        live.get(&thread_id).ok_or("no session")?.handle.send(text)
    }

    pub fn opencode_disconnect(&self, provider: &str) -> Result<ProviderList, String> {
        self.opencode_server()?.remove_auth(provider)?;
        self.opencode_providers(true)
    }

    pub fn opencode_oauth_authorize(
        &self,
        provider: &str,
        method: usize,
        answers: &BTreeMap<String, String>,
    ) -> Result<opencode::Authorization, String> {
        let server = self.opencode_server()?;
        let spec = server.auth_method(provider, method)?;
        server.oauth_authorize(provider, method, &opencode::visible_answers(&spec, answers))
    }

    pub fn opencode_oauth_callback(
        &self,
        provider: &str,
        method: usize,
        code: Option<&str>,
    ) -> Result<ProviderList, String> {
        self.opencode_server()?
            .oauth_callback(provider, method, code)?;
        self.opencode_providers(true)
    }

    /// Take a question and everything after it out of the provider's own
    /// session, so the agent's context matches the timeline. `anchor` is the
    /// provider's handle for that question, kept on its row. A thread whose
    /// process has gone is resumed for this without starting a turn.
    pub fn rewind(
        &self,
        thread_id: i64,
        provider: Provider,
        resume: Option<&str>,
        opts: &SendOptions,
        anchor: &str,
        sink: Sink,
    ) -> Result<(), String> {
        // Rewound outside the lock: it waits on the CLI, and holding the map
        // would stall every other thread's next message.
        let handle = {
            let mut live = self.live.lock().unwrap();
            // Any live session will do — the reasoning level is irrelevant to
            // a control-channel instruction, so no `ensure` respawn.
            if !live.get(&thread_id).is_some_and(|l| l.is_alive()) {
                self.ensure(&mut live, thread_id, provider, resume, opts, sink)?;
            }
            live.get(&thread_id).ok_or("no session")?.handle.clone()
        };
        handle.rewind(anchor)
    }

    /// Make sure this thread has a session that can be talked to, spawning or
    /// resuming one when it has none. A live session is reused only if it
    /// runs under the reasoning level asked for.
    fn ensure(
        &self,
        live: &mut HashMap<i64, Live>,
        thread_id: i64,
        provider: Provider,
        resume: Option<&str>,
        opts: &SendOptions,
        sink: Sink,
    ) -> Result<(), String> {
        if live
            .get(&thread_id)
            .is_some_and(|l| l.is_alive() && l.effort == opts.reasoning_effort)
        {
            return Ok(());
        }
        live.remove(&thread_id);

        let cwd = thread_cwd(&self.data_dir);
        std::fs::create_dir_all(&cwd)
            .map_err(|e| format!("cannot create {}: {e}", cwd.display()))?;
        let raw_log = RawLog::open(&self.data_dir, thread_id);
        let base = |cwd: PathBuf| -> Result<ThreadSpawn, String> {
            Ok(ThreadSpawn {
                bin: discover::binary(provider)?,
                cwd,
                library: self.data_dir.clone(),
                resume: resume.map(String::from),
                model: opts.model.clone(),
                effort: opts.reasoning_effort.clone(),
                env: discover::child_env(),
                raw_log,
            })
        };
        let handle = match provider {
            Provider::Claude => Handle::Claude(ClaudeSession::spawn(
                ClaudeSpawn {
                    base: base(cwd)?,
                    oculus: discover::oculus_cli(),
                    permission_mode: "acceptEdits".into(),
                    system_append: instructions(
                        &self.data_dir,
                        opts.scope.as_deref(),
                        opts.lecture.as_ref(),
                    ),
                    one_off: false,
                },
                sink,
            )?),
            Provider::Codex => {
                let server = self.codex_server()?;
                let topts = CodexThreadOpts {
                    cwd,
                    // Existing files only: Codex fails the whole turn on a
                    // writable root it cannot stat ("failed to inspect
                    // Seatbelt writable root"), e.g. an absent WAL sidecar.
                    writable_files: crate::paths::db_write_paths(&self.data_dir)
                        .into_iter()
                        .filter(|p| p.exists())
                        .collect(),
                    model: opts.model.clone(),
                    reasoning_effort: opts.reasoning_effort.clone(),
                    instructions: instructions(
                        &self.data_dir,
                        opts.scope.as_deref(),
                        opts.lecture.as_ref(),
                    ),
                    ephemeral: false,
                };
                let tid = match resume {
                    Some(id) => {
                        server.resume_thread(id, &topts, sink)?;
                        id.to_string()
                    }
                    None => server.start_thread(&topts, sink)?,
                };
                Handle::Codex(server, tid, Arc::new(topts))
            }
            Provider::Opencode => {
                let server = self.opencode_server()?;
                let sopts = OpencodeSessionOpts {
                    model: opts.model.clone(),
                    // Set only when the model declared levels.
                    variant: opts.reasoning_effort.clone(),
                    brief: thread_sections(opts.scope.as_deref(), opts.lecture.as_ref()),
                    agent: opencode::AGENT,
                };
                let ses = match resume {
                    Some(id) => {
                        server.attach_session(id, &sopts, sink)?;
                        id.to_string()
                    }
                    None => server.start_session(&sopts, sink)?,
                };
                Handle::Opencode(server, ses)
            }
            Provider::Antigravity => Handle::Antigravity(AntigravitySession::spawn(
                AntigravitySpawn {
                    base: base(cwd)?,
                    // `agy` reads the library-wide brief (`agents/AGENTS.md`)
                    // itself; only the per-thread half is sent.
                    brief: thread_sections(opts.scope.as_deref(), opts.lecture.as_ref()),
                    approved: opts.antigravity_rules.clone(),
                },
                sink,
            )?),
        };
        live.insert(
            thread_id,
            Live {
                handle,
                effort: opts.reasoning_effort.clone(),
            },
        );
        Ok(())
    }

    /// Ask the provider to name a thread from its first exchange, on the
    /// agent and model the `threadNaming` job is configured with (`jobs.rs`).
    /// It runs in a throwaway session of its own, so the question never
    /// reaches the thread's timeline or context.
    pub fn name_thread(
        &self,
        sel: &jobs::JobSelection,
        first_message: &str,
        reply: &str,
    ) -> Result<String, String> {
        let turn = self.one_off(sel, NAMING_INSTRUCTIONS, opencode::NAMING_AGENT)?;
        let timeout = Some(std::time::Duration::from_secs(NAMING_TIMEOUT_SECS));
        let answer = turn
            .handle
            .send(&naming_prompt(first_message, reply))
            .map(|()| turn.wait(timeout, "naming the thread"));
        turn.close();
        let answer = answer?;
        let text = answer.message_or_streamed();
        match (clean_title(text), &answer.failed) {
            (Some(t), _) => Ok(t),
            (None, Some(e)) => Err(e.clone()),
            (None, None) => Err(format!("no usable name in the reply: {text:?}")),
        }
    }

    /// A tool-less session outside any thread, on `sel`'s agent, model and
    /// level, not yet prompted. `instructions` is its brief — appended to
    /// Claude's system prompt, Codex's developer instructions, ahead of agy's
    /// first message — except on opencode, whose `agent` carries it as its
    /// prompt. Nothing is persisted or raw-logged.
    fn one_off(
        &self,
        sel: &jobs::JobSelection,
        instructions: &str,
        agent: &'static str,
    ) -> Result<OneOff, String> {
        let provider = sel.provider;
        let (tx, rx) = mpsc::channel::<HarnessEvent>();
        let sink: Sink = Arc::new(move |ev| {
            let _ = tx.send(ev);
        });
        let cwd = thread_cwd(&self.data_dir);
        std::fs::create_dir_all(&cwd)
            .map_err(|e| format!("cannot create {}: {e}", cwd.display()))?;
        let base = |cwd: PathBuf| -> Result<ThreadSpawn, String> {
            Ok(ThreadSpawn {
                bin: discover::binary(provider)?,
                cwd,
                library: self.data_dir.clone(),
                resume: None,
                model: Some(sel.model.clone()),
                effort: sel.reasoning_effort.clone(),
                env: discover::child_env(),
                raw_log: None,
            })
        };

        let handle = match provider {
            Provider::Claude => Handle::Claude(ClaudeSession::spawn(
                ClaudeSpawn {
                    base: base(cwd)?,
                    oculus: discover::oculus_cli(),
                    // `default` auto-allows no tool; with prompts routed to
                    // `none` a stray call is refused rather than hanging.
                    permission_mode: "default".into(),
                    system_append: instructions.to_string(),
                    one_off: true,
                },
                sink,
            )?),
            Provider::Codex => {
                let server = self.codex_server()?;
                let opts = CodexThreadOpts {
                    cwd,
                    writable_files: Vec::new(),
                    model: Some(sel.model.clone()),
                    reasoning_effort: sel.reasoning_effort.clone(),
                    instructions: instructions.to_string(),
                    ephemeral: true,
                };
                let tid = server.start_thread(&opts, sink)?;
                Handle::Codex(server, tid, Arc::new(opts))
            }
            Provider::Opencode => {
                let server = self.opencode_server()?;
                // opencode has no per-session instructions, so the brief is
                // the hidden agent's prompt (`opencode::write_config`).
                let sopts = OpencodeSessionOpts {
                    model: Some(sel.model.clone()),
                    variant: sel.reasoning_effort.clone(),
                    brief: String::new(),
                    agent,
                };
                let ses = server.start_session(&sopts, sink)?;
                Handle::Opencode(server, ses)
            }
            Provider::Antigravity => Handle::Antigravity(AntigravitySession::spawn(
                AntigravitySpawn {
                    base: base(cwd)?,
                    brief: instructions.to_string(),
                    // No database here: keep the approvals last written.
                    approved: None,
                },
                sink,
            )?),
        };
        Ok(OneOff { handle, rx })
    }

    pub fn interrupt(&self, thread_id: i64) -> Result<(), String> {
        let live = self.live.lock().unwrap();
        match live.get(&thread_id) {
            Some(l) => l.handle.interrupt(),
            None => Ok(()),
        }
    }

    /// End the thread's process. The thread row stays; the next send
    /// resumes it by session id.
    pub fn close(&self, thread_id: i64) {
        if let Some(l) = self.live.lock().unwrap().remove(&thread_id) {
            l.handle.close(false);
        }
    }

    /// The threads with a live process of this provider's.
    pub fn live_threads(&self, provider: Provider) -> Vec<i64> {
        self.live
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, l)| l.handle.provider() == provider)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Everything, on quit.
    pub fn shutdown(&self) {
        self.drop_suggestions();
        let ids: Vec<i64> = self.live.lock().unwrap().keys().copied().collect();
        for id in ids {
            self.close(id);
        }
        if let Some(s) = self.codex.lock().unwrap().take() {
            s.kill();
        }
        if let Some(s) = self.opencode.lock().unwrap().take() {
            s.kill();
        }
    }
}

/// A session [`Harness::one_off`] opened, and the events it answers on.
struct OneOff {
    handle: Handle,
    rx: mpsc::Receiver<HarnessEvent>,
}

/// What a one-off turn said. `streamed` is the deltas, which keep the leading
/// whitespace Claude's committed message trims off.
struct OneOffReply {
    message: String,
    streamed: String,
    failed: Option<String>,
}

impl OneOffReply {
    fn message_or_streamed(&self) -> &str {
        if self.message.trim().is_empty() {
            &self.streamed
        } else {
            &self.message
        }
    }
}

impl OneOff {
    /// Wait out the turn a prompt started.
    fn wait(&self, timeout: Option<std::time::Duration>, doing: &str) -> OneOffReply {
        let (mut message, mut streamed) = (String::new(), String::new());
        let failed = drain_turn(&self.rx, timeout, doing, |ev| match ev {
            HarnessEvent::AssistantMessage { text } => message.push_str(text),
            HarnessEvent::AssistantDelta { text } => streamed.push_str(text),
            _ => {}
        });
        OneOffReply { message, streamed, failed }
    }

    /// Deleted rather than detached: a session left on the opencode server
    /// would sit in the student's own session list.
    fn close(&self) {
        self.handle.close(true);
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
    let failed = drain_turn(&rx, None, "finishing", &on_event);
    harness.shutdown();
    match failed {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Wait out one turn on `rx`, handing every event to `on_event`, and answer
/// its failure if it had one. With a `timeout`, that long a silence fails it.
fn drain_turn(
    rx: &mpsc::Receiver<HarnessEvent>,
    timeout: Option<std::time::Duration>,
    doing: &str,
    mut on_event: impl FnMut(&HarnessEvent),
) -> Option<String> {
    let mut failed: Option<String> = None;
    loop {
        let ev = match timeout {
            Some(t) => match rx.recv_timeout(t) {
                Ok(ev) => ev,
                Err(_) => {
                    failed.get_or_insert_with(|| format!("timed out {doing}"));
                    break;
                }
            },
            None => match rx.recv() {
                Ok(ev) => ev,
                Err(_) => break,
            },
        };
        on_event(&ev);
        match ev {
            HarnessEvent::Error { message, .. } => failed = Some(message),
            HarnessEvent::TurnFinished { .. } => break,
            HarnessEvent::Exited { code } => {
                failed.get_or_insert(format!("provider exited (code {code:?}) before {doing}"));
                break;
            }
            _ => {}
        }
    }
    failed
}

// ── Tauri ────────────────────────────────────────────────────────────────────

pub mod app {
    use super::*;
    use sqlx::SqlitePool;
    use tauri::{AppHandle, Emitter, Manager, State};

    /// What the webview gets on `harness-event`, plus the row id when the
    /// event became a row. Account-scoped events arrive with `threadId` 0,
    /// hence the provider.
    #[derive(Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Envelope {
        thread_id: i64,
        provider: Provider,
        item_id: Option<i64>,
        event: HarnessEvent,
    }

    /// Where every bridge's events go, tagged with the thread they belong
    /// to. One consumer reads it; see [`init`].
    type Bus = mpsc::Sender<(i64, Provider, HarnessEvent)>;

    pub struct HarnessState {
        pub harness: Arc<Harness>,
        bus: Bus,
        /// Which threads have a turn open, and what is waiting behind each.
        queue: Arc<Mutex<Queue>>,
    }

    fn sink_for(bus: &Bus, thread_id: i64, provider: Provider) -> Sink {
        let bus = bus.clone();
        Arc::new(move |ev| {
            let _ = bus.send((thread_id, provider, ev));
        })
    }

    /// Run blocking work off the async runtime; a failed join is an error.
    async fn blocking<T: Send + 'static>(
        f: impl FnOnce() -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        tokio::task::spawn_blocking(f).await.map_err(|e| e.to_string())?
    }

    /// One consumer thread folds every event, from every thread, in order:
    /// a row is written before the webview hears about it, and a tool's
    /// finish can never overtake its start.
    pub fn init(app: &AppHandle) -> HarnessState {
        let (tx, rx) = mpsc::channel::<(i64, Provider, HarnessEvent)>();
        let handle = app.clone();
        let harness = Arc::new(Harness::new(crate::paths::data_dir()));
        let queue: Arc<Mutex<Queue>> = Arc::new(Mutex::new(Queue::default()));
        // Thread-less events go out as thread 0, the id their raw logs use.
        {
            let bus = tx.clone();
            harness.set_codex_account_sink(Arc::new(move |ev| {
                let _ = bus.send((0, Provider::Codex, ev));
            }));
        }
        {
            let bus = tx.clone();
            harness.set_opencode_default_sink(Arc::new(move |ev| {
                let _ = bus.send((0, Provider::Opencode, ev));
            }));
        }
        // The naming turn's answer comes back in as an event like any other,
        // so it is written and forwarded by this same loop.
        let (namer, bus, queued) = (harness.clone(), tx.clone(), queue.clone());
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
            let mut pool: Option<SqlitePool> = None;
            for (thread_id, provider, ev) in rx {
                if pool.is_none() {
                    pool = rt.block_on(crate::store::open_pool()).ok();
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
                    // A finished first exchange is when a thread is named. The
                    // claim is atomic (asks at most once); the naming turn runs
                    // off this loop, which every thread's events wait behind.
                    if thread_id > 0
                        && matches!(&ev, HarnessEvent::TurnFinished { status } if status == "completed")
                    {
                        match rt.block_on(store::claim_naming(p, thread_id)) {
                            Ok(Some(seed)) => {
                                let sel = rt.block_on(jobs::selection(p, jobs::Job::ThreadNaming));
                                let (namer, bus) = (namer.clone(), bus.clone());
                                std::thread::spawn(move || {
                                    match namer.name_thread(&sel, &seed.first_message, &seed.reply)
                                    {
                                        Ok(title) => {
                                            let _ = bus.send((
                                                thread_id,
                                                provider,
                                                HarnessEvent::ThreadTitled { title },
                                            ));
                                        }
                                        Err(e) => eprintln!("[oculus] harness title: {e}"),
                                    }
                                });
                            }
                            Ok(None) => {}
                            Err(e) => eprintln!("[oculus] harness title: {e}"),
                        }
                    }
                }
                // A closed turn releases the next queued message; its send is
                // spawned, since a send may start a process.
                if thread_id > 0 && matches!(&ev, HarnessEvent::TurnFinished { .. }) {
                    let next = queued.lock().unwrap().next(thread_id);
                    if let Some((msg, opts)) = next {
                        let (h, b) = (namer.clone(), bus.clone());
                        let _ =
                            b.send((thread_id, provider, HarnessEvent::Unqueued { id: msg.id }));
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) =
                                dispatch(h, b, thread_id, provider, opts, msg.text).await
                            {
                                eprintln!("[oculus] harness queued send: {e}");
                            }
                        });
                    }
                }

                let _ = handle.emit(
                    "harness-event",
                    Envelope {
                        thread_id,
                        provider,
                        item_id,
                        event: ev,
                    },
                );
            }
        });
        HarnessState {
            harness,
            bus: tx,
            queue,
        }
    }

    impl HarnessState {
        fn sink(&self, thread_id: i64, provider: Provider) -> Sink {
            sink_for(&self.bus, thread_id, provider)
        }
    }

    /// A send the queue has let go. A failure still closes the turn (an error
    /// row, then `TurnFinished`), so nothing queued behind it is stranded.
    async fn dispatch(
        harness: Arc<Harness>,
        bus: Bus,
        thread_id: i64,
        provider: Provider,
        opts: SendOptions,
        text: String,
    ) -> Result<(), String> {
        let sink = sink_for(&bus, thread_id, provider);
        match start_turn(&harness, thread_id, provider, opts, &text, sink.clone()).await {
            Ok(()) => Ok(()),
            Err(e) => {
                sink(HarnessEvent::error(e.clone()));
                sink(HarnessEvent::TurnFinished {
                    status: "failed".into(),
                });
                Err(e)
            }
        }
    }

    /// The lecture section of a thread's brief, read off its row on every
    /// send (chapters can land after the conversation started). Needed
    /// before *any* spawn, a rewind's included: the brief binds at start.
    async fn lecture_brief(pool: &SqlitePool, row: &store::ThreadRow) -> Option<LectureBrief> {
        let l = row.lecture.as_ref()?;
        Some(LectureBrief {
            id: l.id.clone(),
            title: l.title.clone(),
            date: l.date.clone(),
            has_transcript: l.has_transcript,
            chapters: crate::store::chapters(pool, &l.id)
                .await
                .unwrap_or_default(),
        })
    }

    async fn start_turn(
        harness: &Arc<Harness>,
        thread_id: i64,
        provider: Provider,
        opts: SendOptions,
        text: &str,
        sink: Sink,
    ) -> Result<(), String> {
        let pool = crate::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        if row.provider != provider {
            return Err(format!(
                "thread {thread_id} is a {} thread",
                row.provider.label()
            ));
        }
        if opts.model.is_some() && opts.model != row.model {
            store::set_model(&pool, thread_id, opts.model.as_deref()).await?;
            // Claude and agy fix `--model` at spawn. The thread is between
            // turns here, so nothing is killed mid-answer.
            if matches!(provider, Provider::Claude | Provider::Antigravity) {
                harness.close(thread_id);
            }
        }
        // The row, not the payload, decides model, subject and lecture.
        let lecture = lecture_brief(&pool, &row).await;
        // Only a spawn uses these, but whether this send spawns is `ensure`'s call.
        let antigravity_rules = match provider {
            Provider::Antigravity => Some(antigravity_rules::stored(&pool).await?),
            _ => None,
        };
        let opts = SendOptions {
            model: opts.model.or(row.model),
            scope: row.subject_code,
            lecture,
            antigravity_rules,
            ..opts
        };
        let resume = row.provider_session_id;
        // The row is what the student typed; the moment's text rides only
        // the prompt, or the timeline would show a transcript excerpt.
        sink(HarnessEvent::UserMessage {
            text: text.to_string(),
            at: opts.at,
        });
        let text = match &opts.context {
            Some(c) => format!("{text}\n\n---\n\n## The moment this was sent at\n\n{c}"),
            None => text.to_string(),
        };

        let (h, sink) = (harness.clone(), sink.clone());
        blocking(move || h.send(thread_id, provider, resume.as_deref(), &opts, &text, sink)).await
    }

    /// Where each CLI is, and whether it is there at all. Only `recheck`
    /// (Settings' *Recheck*) drops `discover`'s caches — a full recheck can
    /// cost a login shell per provider, and every model picker reads this.
    #[tauri::command]
    pub async fn harness_health(recheck: bool) -> Vec<discover::BridgeHealth> {
        tokio::task::spawn_blocking(move || {
            if recheck {
                discover::forget();
            }
            discover::PROVIDERS
                .iter()
                .map(|p| discover::health(*p))
                .collect()
        })
        .await
        .unwrap_or_default()
    }

    /// The ways this machine could install one of the CLIs, and which it can
    /// run. Detection is blocking and cached (`discover::tool`).
    #[tauri::command]
    pub async fn harness_install_offer(provider: Provider) -> install::InstallOffer {
        tokio::task::spawn_blocking(move || install::offer(provider, install::detect()))
            .await
            // The only safe reading of a failed join: commands to copy, no buttons.
            .unwrap_or_else(|_| install::offer(provider, install::Managers::default()))
    }

    /// Run one route. The webview names a provider and a manager, never the
    /// command. Output streams on `install::INSTALL_EVENT`; Settings rechecks
    /// on the `done` event.
    #[tauri::command]
    pub async fn harness_install_run(
        app: AppHandle,
        provider: Provider,
        manager: install::Manager,
    ) -> Result<(), String> {
        blocking(move || {
            let emitter = app.clone();
            install::start(provider, manager, move |line| {
                emitter.emit(install::INSTALL_EVENT, line).ok();
            })
        })
        .await
    }

    /// Whether a provider has credentials, asked of the CLI every time (see
    /// `signin.rs` for why this is never cached).
    #[tauri::command]
    pub async fn harness_sign_in_status(provider: Provider) -> signin::SignInStatus {
        tokio::task::spawn_blocking(move || signin::status(provider))
            .await
            .unwrap_or_else(|e| signin::SignInStatus {
                provider,
                signed_in: None,
                account: None,
                error: Some(e.to_string()),
            })
    }

    /// Run the provider's own login flow. Output streams on
    /// `signin::SIGNIN_EVENT`; the first URL opens in the *system* browser,
    /// where the student is already signed in, and rides the event too so
    /// the dialog can offer it to copy.
    #[tauri::command]
    pub async fn harness_sign_in_start(app: AppHandle, provider: Provider) -> Result<(), String> {
        blocking(move || {
            let emitter = app.clone();
            signin::start(provider, move |line| {
                if let Some(url) = line.url.as_deref() {
                    tauri_plugin_opener::open_url(url, None::<&str>).ok();
                }
                emitter.emit(signin::SIGNIN_EVENT, line).ok();
            })
        })
        .await
    }

    /// The code Claude's flow ends on, pasted back from the browser.
    #[tauri::command]
    pub async fn harness_sign_in_code(
        state: State<'_, HarnessState>,
        provider: Provider,
        code: String,
    ) -> Result<(), String> {
        let h = state.harness.clone();
        blocking(move || {
            let signed_in = signin::submit_code(provider, &code);
            if provider == Provider::Claude {
                h.forget_claude_models();
            }
            signed_in
        })
        .await
    }

    /// The student closed the dialog — the only thing that ends a login.
    #[tauri::command]
    pub async fn harness_sign_in_cancel(provider: Provider) -> Result<(), String> {
        blocking(move || signin::cancel(provider)).await
    }

    /// Only Codex can be asked for its plan windows; the other providers'
    /// arrive with a turn.
    #[tauri::command]
    pub async fn harness_refresh_rate_limits(
        state: State<'_, HarnessState>,
        provider: Provider,
    ) -> Result<(), String> {
        if provider != Provider::Codex {
            return Ok(());
        }
        let h = state.harness.clone();
        blocking(move || {
            h.refresh_codex_rate_limits();
            Ok(())
        })
        .await
    }

    #[tauri::command]
    pub async fn harness_codex_models(
        state: State<'_, HarnessState>,
    ) -> Result<Vec<ModelInfo>, String> {
        let h = state.harness.clone();
        blocking(move || h.codex_models()).await
    }

    #[tauri::command]
    pub async fn harness_antigravity_models(
        state: State<'_, HarnessState>,
    ) -> Result<Vec<antigravity::ModelInfo>, String> {
        let h = state.harness.clone();
        blocking(move || h.antigravity_models()).await
    }

    /// Allow what an Antigravity thread was just stopped at. `rule` is in
    /// `agy`'s syntax. It is stored and the thread's process dropped (a live
    /// `agy` never re-reads its rules); the webview sends the follow-up.
    /// Refused mid-turn, and for a rule one of Oculus's own denies covers.
    #[tauri::command]
    pub async fn harness_antigravity_allow(
        state: State<'_, HarnessState>,
        thread_id: i64,
        rule: String,
    ) -> Result<Vec<String>, String> {
        let rule = rule.trim().to_string();
        if !antigravity_rules::is_valid_rule(&rule) {
            return Err(format!(
                "not a rule Oculus can allow: {rule:?} — expected command(…), read_file(…), \
                 write_file(…) or read_url(…)"
            ));
        }
        if let Some(d) = antigravity_rules::denied_by(&crate::paths::data_dir(), &rule) {
            return Err(format!("{rule} would change nothing: Oculus keeps {d} closed to every agent"));
        }
        let pool = crate::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        if row.provider != Provider::Antigravity {
            return Err(format!("thread {thread_id} is a {} thread", row.provider.label()));
        }
        if state.queue.lock().unwrap().is_busy(thread_id) {
            return Err("stop the current turn before allowing something new".into());
        }
        let mut rules = antigravity_rules::stored(&pool).await?;
        if !rules.contains(&rule) {
            rules.push(rule);
            antigravity_rules::save(&pool, &rules).await?;
        }
        state.harness.close(thread_id);
        Ok(rules)
    }

    /// The student's Antigravity approvals, for Settings to list.
    #[tauri::command]
    pub async fn harness_antigravity_rules() -> Result<Vec<String>, String> {
        let pool = crate::store::open_pool().await?;
        antigravity_rules::stored(&pool).await
    }

    /// Take an approval back. The settings file is rewritten now (the
    /// student's own `agy` reads it too), and every idle Antigravity thread's
    /// process is dropped so none keeps running under the rule.
    #[tauri::command]
    pub async fn harness_antigravity_revoke(
        state: State<'_, HarnessState>,
        rule: String,
    ) -> Result<Vec<String>, String> {
        let pool = crate::store::open_pool().await?;
        let mut rules = antigravity_rules::stored(&pool).await?;
        rules.retain(|r| r != rule.trim());
        antigravity_rules::save(&pool, &rules).await?;
        let written = rules.clone();
        blocking(move || antigravity_rules::install(&crate::paths::data_dir(), Some(written)))
            .await?;
        for id in state.harness.live_threads(Provider::Antigravity) {
            if !state.queue.lock().unwrap().is_busy(id) {
                state.harness.close(id);
            }
        }
        Ok(rules)
    }

    /// Claude Code's catalogue, off the CLI's `initialize` answer — no turn,
    /// nothing billed.
    #[tauri::command]
    pub async fn harness_claude_models(
        state: State<'_, HarnessState>,
    ) -> Result<Vec<claude::ModelInfo>, String> {
        let h = state.harness.clone();
        blocking(move || h.claude_models()).await
    }

    /// opencode's catalogue. Starts the server if it is not up.
    #[tauri::command]
    pub async fn harness_opencode_models(
        state: State<'_, HarnessState>,
    ) -> Result<Vec<opencode::ModelInfo>, String> {
        let h = state.harness.clone();
        blocking(move || h.opencode_models()).await
    }

    // ── opencode credentials ─────────────────────────────────────────────
    //
    // Each starts the server if it is down, so none is called on a page
    // opening. A credential never comes back out: there is no read side.

    #[tauri::command]
    pub async fn harness_opencode_providers(
        state: State<'_, HarnessState>,
        refresh: bool,
    ) -> Result<opencode::ProviderList, String> {
        let h = state.harness.clone();
        blocking(move || h.opencode_providers(refresh)).await
    }

    #[tauri::command]
    pub async fn harness_opencode_set_key(
        state: State<'_, HarnessState>,
        provider: String,
        method: usize,
        key: String,
        answers: Option<BTreeMap<String, String>>,
    ) -> Result<opencode::ProviderList, String> {
        let h = state.harness.clone();
        let answers = answers.unwrap_or_default();
        blocking(move || h.opencode_set_api_key(&provider, method, &key, &answers)).await
    }

    #[tauri::command]
    pub async fn harness_opencode_disconnect(
        state: State<'_, HarnessState>,
        provider: String,
    ) -> Result<opencode::ProviderList, String> {
        let h = state.harness.clone();
        blocking(move || h.opencode_disconnect(&provider)).await
    }

    /// Start a browser flow: the URL to open (in the system browser), whether
    /// the server finishes it by itself (`auto`), and what to tell the student.
    #[tauri::command]
    pub async fn harness_opencode_oauth_start(
        state: State<'_, HarnessState>,
        provider: String,
        method: usize,
        answers: Option<BTreeMap<String, String>>,
    ) -> Result<opencode::Authorization, String> {
        let h = state.harness.clone();
        let answers = answers.unwrap_or_default();
        blocking(move || h.opencode_oauth_authorize(&provider, method, &answers)).await
    }

    /// Finish a `code` flow. An `auto` one never calls this; the dialog polls
    /// `harness_opencode_providers` with `refresh` instead.
    #[tauri::command]
    pub async fn harness_opencode_oauth_finish(
        state: State<'_, HarnessState>,
        provider: String,
        method: usize,
        code: Option<String>,
    ) -> Result<opencode::ProviderList, String> {
        let h = state.harness.clone();
        blocking(move || h.opencode_oauth_callback(&provider, method, code.as_deref())).await
    }

    /// Send a message; creates the thread when `thread_id` is null. Returns
    /// the thread id. A message sent mid-turn waits in the [`Queue`] and the
    /// webview gets only a `queued` event until it goes out.
    #[tauri::command]
    pub async fn harness_send(
        state: State<'_, HarnessState>,
        thread_id: Option<i64>,
        provider: String,
        text: String,
        options: Option<SendOptions>,
    ) -> Result<i64, String> {
        let provider =
            Provider::parse(&provider).ok_or_else(|| format!("unknown provider {provider}"))?;
        let mut opts = options.unwrap_or_default();
        opts.reasoning_effort = validate_effort(opts.reasoning_effort)?;
        let pool = crate::store::open_pool().await?;

        let id = match thread_id {
            Some(id) => {
                let row = store::thread(&pool, id).await?;
                if row.provider != provider {
                    return Err(format!("thread {id} is a {} thread", row.provider.label()));
                }
                id
            }
            None => {
                store::create_thread(
                    &pool,
                    provider,
                    opts.model.as_deref(),
                    opts.subject_id,
                    opts.lecture_id.as_deref(),
                    &text,
                )
                .await?
            }
        };

        if !state.queue.lock().unwrap().try_claim(id) {
            let msg = state.queue.lock().unwrap().push(id, &text, &opts);
            state.sink(id, provider)(HarnessEvent::Queued {
                id: msg.id,
                text: msg.text,
            });
            return Ok(id);
        }
        dispatch(
            state.harness.clone(),
            state.bus.clone(),
            id,
            provider,
            opts,
            text,
        )
        .await?;
        Ok(id)
    }

    /// Ask a question again, differently: rewind the thread to that question
    /// (rows and, where it can, the agent's context) and send the new text.
    /// A provider that cannot rewind is not fatal — the rows still go, and
    /// the `Rewound` event says the agent kept the original.
    #[tauri::command]
    pub async fn harness_edit_resend(
        state: State<'_, HarnessState>,
        thread_id: i64,
        item_id: i64,
        text: String,
        options: Option<SendOptions>,
    ) -> Result<(), String> {
        let mut opts = options.unwrap_or_default();
        opts.reasoning_effort = validate_effort(opts.reasoning_effort)?;
        let pool = crate::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        // Checked against the row: everything from it on is about to go.
        let question = store::user_item(&pool, thread_id, item_id).await?;
        if !state.queue.lock().unwrap().try_claim(thread_id) {
            return Err("stop the current turn before editing a question".into());
        }
        let context = rewind_provider(&state, &pool, thread_id, &row, &question, &opts).await;
        if let Err(e) = store::truncate_from(&pool, thread_id, item_id).await {
            // Nothing was sent, so nothing will close the turn this claimed.
            state.queue.lock().unwrap().next(thread_id);
            return Err(e);
        }
        let sink = state.sink(thread_id, row.provider);
        sink(HarnessEvent::Rewound {
            from_item_id: item_id,
            context,
        });
        dispatch(
            state.harness.clone(),
            state.bus.clone(),
            thread_id,
            row.provider,
            opts,
            text,
        )
        .await
    }

    /// Ask the provider to forget this question and everything after it, and
    /// answer whether it did. No anchor, a refusal or an unresumable session
    /// all answer `false`, which the timeline shows as a note.
    async fn rewind_provider(
        state: &State<'_, HarnessState>,
        pool: &SqlitePool,
        thread_id: i64,
        row: &store::ThreadRow,
        question: &store::Question,
        opts: &SendOptions,
    ) -> bool {
        let Some(anchor) = question.anchor.clone() else {
            return false;
        };
        let Some(resume) = row.provider_session_id.clone() else {
            return false;
        };
        let (h, provider) = (state.harness.clone(), row.provider);
        let sink = state.sink(thread_id, provider);
        // A rewind may respawn the session, which binds the brief.
        let opts = SendOptions {
            model: opts.model.clone().or_else(|| row.model.clone()),
            scope: row.subject_code.clone(),
            lecture: lecture_brief(pool, row).await,
            ..opts.clone()
        };
        blocking(move || h.rewind(thread_id, provider, Some(&resume), &opts, &anchor, sink))
            .await
            .is_ok()
    }

    /// Take the thread back to just before a question and hand the question
    /// back for the composer. Unlike [`harness_edit_resend`], nothing is sent.
    #[tauri::command]
    pub async fn harness_rewind(
        state: State<'_, HarnessState>,
        thread_id: i64,
        item_id: i64,
    ) -> Result<String, String> {
        let pool = crate::store::open_pool().await?;
        let row = store::thread(&pool, thread_id).await?;
        let question = store::user_item(&pool, thread_id, item_id).await?;
        if state.queue.lock().unwrap().is_busy(thread_id) {
            return Err("stop the current turn before rewinding".into());
        }
        let opts = SendOptions {
            model: row.model.clone(),
            ..Default::default()
        };
        let context = rewind_provider(&state, &pool, thread_id, &row, &question, &opts).await;
        store::truncate_from(&pool, thread_id, item_id).await?;
        state.sink(thread_id, row.provider)(HarnessEvent::Rewound {
            from_item_id: item_id,
            context,
        });
        Ok(question.text)
    }

    /// What is still waiting behind this thread's turn (the queue is only in
    /// memory), for a reloaded page.
    #[tauri::command]
    pub async fn harness_queued(
        state: State<'_, HarnessState>,
        thread_id: i64,
    ) -> Result<Vec<QueuedMessage>, String> {
        Ok(state.queue.lock().unwrap().list(thread_id))
    }

    /// Drop one message that has not gone out yet.
    #[tauri::command]
    pub async fn harness_unqueue(
        state: State<'_, HarnessState>,
        thread_id: i64,
        queue_id: String,
    ) -> Result<(), String> {
        if state.queue.lock().unwrap().remove(thread_id, &queue_id) {
            let pool = crate::store::open_pool().await?;
            let row = store::thread(&pool, thread_id).await?;
            state.sink(thread_id, row.provider)(HarnessEvent::Unqueued { id: queue_id });
        }
        Ok(())
    }

    /// Rewrite one that has not gone out yet; it comes back as a `queued`
    /// event under the same id, so the webview replaces it.
    #[tauri::command]
    pub async fn harness_edit_queued(
        state: State<'_, HarnessState>,
        thread_id: i64,
        queue_id: String,
        text: String,
    ) -> Result<(), String> {
        let edited = state
            .queue
            .lock()
            .unwrap()
            .edit(thread_id, &queue_id, &text);
        if let Some(msg) = edited {
            let pool = crate::store::open_pool().await?;
            let row = store::thread(&pool, thread_id).await?;
            state.sink(thread_id, row.provider)(HarnessEvent::Queued {
                id: msg.id,
                text: msg.text,
            });
        }
        Ok(())
    }

    /// Stop the running turn and drop whatever was waiting behind it,
    /// handing the dropped texts back to the composer.
    #[tauri::command]
    pub async fn harness_interrupt(
        state: State<'_, HarnessState>,
        thread_id: i64,
    ) -> Result<Vec<String>, String> {
        let cleared = state.queue.lock().unwrap().clear(thread_id);
        if !cleared.is_empty() {
            let pool = crate::store::open_pool().await?;
            let row = store::thread(&pool, thread_id).await?;
            let sink = state.sink(thread_id, row.provider);
            for m in &cleared {
                sink(HarnessEvent::Unqueued { id: m.id.clone() });
            }
        }
        let h = state.harness.clone();
        blocking(move || h.interrupt(thread_id)).await?;
        Ok(cleared.into_iter().map(|m| m.text).collect())
    }

    /// One inline completion for the document editor: the text to insert at
    /// the caret between `before` and `after` in the note at `path`
    /// (library-relative, for the prompt's title and subject). Empty for none,
    /// and for a call a newer `request_id` or a cancel superseded.
    #[tauri::command]
    pub async fn document_suggest(
        state: State<'_, HarnessState>,
        request_id: u64,
        path: String,
        before: String,
        after: String,
    ) -> Result<String, String> {
        let pool = crate::store::open_pool().await?;
        let sel = jobs::selection(&pool, jobs::Job::DocumentSuggestions).await;
        let h = state.harness.clone();
        blocking(move || h.suggest(request_id, &sel, &path, &before, &after)).await
    }

    /// Stop the suggestion in flight, if any; its call answers empty.
    #[tauri::command]
    pub async fn document_suggest_cancel(state: State<'_, HarnessState>) -> Result<(), String> {
        let h = state.harness.clone();
        blocking(move || {
            h.cancel_suggestion();
            Ok(())
        })
        .await
    }

    #[tauri::command]
    pub async fn harness_delete_thread(
        state: State<'_, HarnessState>,
        thread_id: i64,
    ) -> Result<(), String> {
        state.harness.close(thread_id);
        state.queue.lock().unwrap().forget(thread_id);
        let pool = crate::store::open_pool().await?;
        store::delete_thread(&pool, thread_id).await
    }

    /// Startup: kill opencode servers a previous run was terminated out of
    /// (no `shutdown` or `Drop` ran). See [`opencode::sweep`].
    pub fn sweep_strays() {
        let killed = opencode::sweep();
        if !killed.is_empty() {
            eprintln!(
                "[oculus] harness: killed {} stray opencode server(s): {killed:?}",
                killed.len()
            );
        }
    }

    /// Startup: nothing survives a restart as `running`.
    pub fn reconcile(app: &AppHandle) {
        let _ = app;
        tauri::async_runtime::spawn(async {
            if let Ok(pool) = crate::store::open_pool().await {
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

    #[test]
    fn a_name_is_taken_out_of_whatever_the_model_wrapped_it_in() {
        assert_eq!(
            clean_title("Dijkstra worked example").as_deref(),
            Some("Dijkstra worked example")
        );
        assert_eq!(
            clean_title("\"Week 6 tutorial questions\"\n").as_deref(),
            Some("Week 6 tutorial questions")
        );
        assert_eq!(
            clean_title("Title: **Semaphores and deadlock**").as_deref(),
            Some("Semaphores and deadlock")
        );
        assert_eq!(
            clean_title("Assignment 2 marking scheme.").as_deref(),
            Some("Assignment 2 marking scheme")
        );
        assert_eq!(clean_title(""), None);
        assert_eq!(clean_title("   \n\n "), None);
        assert_eq!(
            clean_title("The student asks about the difficulty of the week 6 lecture and the assistant replies"),
            None,
            "prose is refused rather than becoming the name"
        );
    }

    #[test]
    fn a_thread_runs_one_turn_and_the_rest_wait_in_order() {
        let mut q = Queue::default();
        let opts = SendOptions::default();
        assert!(q.try_claim(1), "an idle thread is taken by the first send");
        assert!(!q.try_claim(1), "and not by the second");

        let a = q.push(1, "first", &opts);
        let b = q.push(1, "second", &opts);
        assert_eq!(q.list(1).len(), 2);
        // Another thread is not held up by this one.
        assert!(q.try_claim(2));

        assert_eq!(
            q.next(1).map(|(m, _)| m),
            Some(a),
            "in the order they were typed"
        );
        assert!(
            !q.try_claim(1),
            "the thread stays claimed while one is going out"
        );
        assert_eq!(q.next(1).map(|(m, _)| m.text), Some("second".into()));
        assert!(q.next(1).is_none(), "nothing left");
        assert!(q.try_claim(1), "and the thread is free again");
        let _ = b;
    }

    #[test]
    fn stopping_clears_the_queue_and_returns_what_it_held() {
        let mut q = Queue::default();
        let opts = SendOptions::default();
        q.try_claim(7);
        q.push(7, "one", &opts);
        let two = q.push(7, "two", &opts);
        q.push(7, "three", &opts);
        assert!(
            q.remove(7, &two.id),
            "a pending message can be dropped on its own"
        );
        assert_eq!(
            q.clear(7).into_iter().map(|m| m.text).collect::<Vec<_>>(),
            vec!["one".to_string(), "three".to_string()]
        );
        assert!(q.list(7).is_empty());
        // Only the running turn's `TurnFinished` releases the thread.
        assert!(!q.try_claim(7));
        assert!(q.next(7).is_none());
        assert!(q.try_claim(7));
    }

    /// The scope is appended to the library brief, not substituted.
    #[test]
    fn a_scoped_thread_keeps_the_library_brief_and_names_its_folder() {
        let root = crate::test_support::Scratch::new("harness-scope");
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        let general = instructions(&root, None, None);
        assert!(
            general.contains("`COMP30026_2026_SM2`"),
            "the course list is filled in"
        );
        assert!(
            !general.contains("This conversation"),
            "no scope section on a general thread"
        );
        // The memory contract rides every brief, both halves: read the index
        // first, write a fact the moment it is true.
        assert!(
            general.contains("oculus memory list"),
            "the index is read with the command"
        );
        assert!(
            general.contains("oculus memory write"),
            "and written with it"
        );
        assert!(
            general.contains("`./TASTE.md`"),
            "the standing preferences are named too"
        );
        assert!(
            general.contains("## Memory"),
            "and the contract is its own section"
        );

        let scoped = instructions(&root, Some("COMP30026_2026_SM2"), None);
        assert!(
            scoped.starts_with(&general),
            "the scope is appended to the same brief"
        );
        assert!(scoped.contains("`../courses/COMP30026_2026_SM2/`"));
        // The subject bucket is named by flag, never by the course folder's
        // own `agents/memories/` path, which every sandbox refuses.
        assert!(scoped.contains("oculus memory list -s COMP30026_2026_SM2"));
        assert!(
            !scoped.contains("../courses/COMP30026_2026_SM2/agents/memories"),
            "the unwritable path is not offered as a place to write"
        );
    }

    /// The lecture is a third layer on the same brief, with paths as typed
    /// from `agents/` and the chapters inline.
    #[test]
    fn a_lecture_thread_keeps_both_briefs_and_names_the_recording() {
        let root = crate::test_support::Scratch::new("harness-lecture");
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        let scoped = instructions(&root, Some("COMP30026_2026_SM2"), None);
        let lecture = LectureBrief {
            id: "abc-123".into(),
            title: "Lecture 14".into(),
            date: "2026-09-08".into(),
            has_transcript: true,
            chapters: vec![crate::chapters::Chapter {
                start_seconds: 1382,
                title: "Resolution".into(),
                summary: "Unification, worked".into(),
            }],
        };
        let full = instructions(&root, Some("COMP30026_2026_SM2"), Some(&lecture));

        assert!(
            full.starts_with(&scoped),
            "the lecture is appended to the subject's brief"
        );
        assert!(
            full.contains("`../lectures/abc-123/`"),
            "the folder as the agent would type it"
        );
        assert!(full.contains("`../lectures/abc-123/transcript.vtt`"));
        assert!(
            full.contains("00:23:02 — Resolution"),
            "chapters are inline"
        );
        assert!(full.contains("the moment it was sent at"));
        assert!(full.contains("2026-09-08"), "the recording's date is named");
        assert!(
            full.contains("oculus files COMP30026_2026_SM2 --type pdf"),
            "the deck hunt is written out with its real flags"
        );

        let no_transcript = instructions(
            &root,
            Some("COMP30026_2026_SM2"),
            Some(&LectureBrief {
                has_transcript: false,
                chapters: vec![],
                ..lecture
            }),
        );
        assert!(
            !no_transcript.contains("`../lectures/abc-123/transcript.vtt`"),
            "a transcript that is not on disk is not named"
        );
    }
}
