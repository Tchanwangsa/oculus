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

pub mod antigravity;
pub mod attach;
pub mod claude;
pub mod codex;
pub mod discover;
pub mod event;
pub mod install;
pub mod jobs;
pub mod opencode;
pub mod signin;
pub mod store;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use serde::{Deserialize, Serialize};

use antigravity::{AntigravitySession, AntigravitySpawn};
use claude::{ClaudeSession, ClaudeSpawn};
use codex::{CodexServer, CodexSpawn, CodexThreadOpts, ModelInfo};
use opencode::{OpencodeServer, OpencodeSessionOpts, OpencodeSpawn, ProviderList};
pub use event::{HarnessEvent, Provider, ToolKind};

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
///
/// `lecture` is the recording a dock conversation is about, and it is
/// appended after the subject section rather than instead of it: a lecture
/// thread is still scoped to that lecture's course, and the agent still reads
/// the whole library.
pub fn instructions(data_dir: &Path, scope: Option<&str>, lecture: Option<&LectureBrief>) -> String {
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
    format!("{base}{}", thread_sections(scope, lecture))
}

/// The part of the brief that is about *this thread* — the subject it was
/// scoped to, the lecture it was opened over — and not about the library.
///
/// Split out of [`instructions`] because opencode cannot take the two
/// together. Claude appends a whole system prompt per process and Codex
/// takes `developerInstructions` per thread, so for both this is simply the
/// tail of the one string. opencode has no append at all: an agent's
/// `prompt` *replaces* its system prompt and lives in one config document
/// shared by every thread, so its bridge puts the library-wide half in that
/// document and sends this half ahead of the session's first message.
pub fn thread_sections(scope: Option<&str>, lecture: Option<&LectureBrief>) -> String {
    let mut out = String::new();
    if let Some(code) = scope {
        out.push_str(&format!(
            "\n\n## This conversation\n\n\
             It is scoped to **{code}** — the folder `../courses/{code}/`. Unless the \
             student names another subject, answer from that folder, and pass `{code}` \
             as the subject to the CLI. Read its `AGENTS.md` for the layout, and \
             `./memories/{code}/` for what you have already learned about it — that is \
             where a fact worth keeping from this conversation belongs, rather than in \
             `./memories/` itself. (The course folder's own `agents/memories/` is a \
             link to it; write the path above, since the folder it links to is the one \
             you can write.)\n"
        ));
    }
    if let Some(lec) = lecture {
        out.push_str(&lecture_section(lec, scope));
    }
    out
}

/// What the agent is told about the recording the student is watching.
///
/// Paths are given the way every other path in this brief is — relative to
/// `agents/`, which every thread runs from — so the agent can paste one
/// straight into a read rather than reconstructing the library root.
///
/// The chapter list is **inlined** while the transcript is only named. The
/// chapters are a dozen short lines and asking for them would cost a tool
/// call the student waits through; the transcript is twenty thousand words
/// and the agent should open the part it needs, which is the same call
/// `chapters::prompt` makes about the same two files.
///
/// The rest of it is there because of what a real thread spent its turn
/// doing (`agents/threads/26.ndjson`): four reads scrolling around a VTT
/// whose every other line is a `NOTE CONF` the agent had no way to know was
/// noise, two `oculus files` calls to discover which PDF the slide deck was,
/// and two refusals from guessing that `grep` and `read` take a subject
/// positionally the way `files` does. None of that is the model being slow —
/// it is this brief naming a course folder and leaving the rest to be
/// rediscovered every time. So: the recording's date, since the title is the
/// timetable's and says nothing about which week; the shape of the VTT; and
/// the two commands written out with their real flags.
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
    /// The recording a dock conversation is about. Read only when the send
    /// creates the thread, like `subject_id` — and unlike it, it also
    /// *decides* the subject, which Rust reads off the lecture's own row
    /// (`store::create_thread`).
    pub lecture_id: Option<String>,
    /// That lecture, resolved from the thread row for the same reason
    /// `scope` is: the instructions name a folder and a chapter list, and
    /// both come from the database rather than from the webview.
    #[serde(skip)]
    pub lecture: Option<LectureBrief>,
    /// The moment, built by the player at send time: the timestamp, the last
    /// minute of transcript, the chapter, the frame path. Appended to the
    /// prompt the CLI receives, **after** the student's text — it is context
    /// for the question, not the question. It never becomes the row's
    /// content: the timeline shows what was typed.
    pub context: Option<String>,
    /// The playhead's second when the message was sent. Goes on the user
    /// row's `meta` so the bubble can say "at 3:40"; see
    /// [`HarnessEvent::UserMessage`].
    pub at: Option<i64>,
}

/// What a lecture thread's appended instructions say about the recording.
/// Assembled from the thread's row and `store::chapters` before the send
/// reaches a bridge.
#[derive(Clone)]
pub struct LectureBrief {
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD`, from the row. See [`store::LectureRef::date`]: the title
    /// is the timetable's, so the date is what tells the agent which week —
    /// and therefore which slide deck — it is being asked about.
    pub date: String,
    pub has_transcript: bool,
    /// Inlined rather than left for the agent to fetch: a chapter list is a
    /// dozen short lines, and a turn spent reading it back is a turn the
    /// student waits through.
    pub chapters: Vec<crate::chapters::Chapter>,
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
/// Both CLIs accept a second message mid-turn and neither does anything good
/// with it. Measured against both, and the recordings are the fixtures the
/// bridge tests replay: `claude` queues it itself and starts a second turn
/// the instant the first closes — so the composer flickers idle between them
/// and stop has nothing to stop — while `codex` answers a second `turn/start`
/// with the *running* turn's id and folds the message into it, so the
/// question lands in the timeline above the answer to the previous one, and
/// sometimes is never answered visibly at all.
///
/// So the waiting happens here: one turn per thread, everything else pending,
/// and a pending message is not part of the conversation — no row is written
/// for it until it goes out, which is why it can still be edited or dropped.
#[derive(Default)]
pub struct Queue {
    threads: HashMap<i64, ThreadQueue>,
}

#[derive(Default)]
struct ThreadQueue {
    /// A turn of ours is open on this thread. Released by the
    /// `TurnFinished` that closes it — both bridges promise exactly one per
    /// message they accept, which is what `expecting` (Claude) and the turn
    /// id taken from `turn/start` (Codex) are for.
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

    /// Everything still waiting, dropped — what stop does. They are handed
    /// back rather than discarded so the composer can return them to the
    /// student, who typed them and never saw them sent.
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

// Naming is a one-line job on a clipped exchange, so it runs on something
// cheap rather than on whatever the thread itself is using — but *which*
// cheap thing is a configured job now (`jobs::Job::ThreadNaming`), not a
// constant here. See `jobs.rs`.

/// Long enough for a cold `claude` start on a slow disk, short enough that a
/// wedged CLI does not leave a thread thinking it is being named.
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

/// What survives from a naming reply, if anything.
///
/// A model asked for a name alone still sometimes wraps it in quotes, labels
/// it, or writes a sentence. The first non-empty line is taken, the wrapping
/// is stripped, and anything that reads like prose rather than a name — too
/// long — is refused so the first-line title stays instead.
fn clean_title(raw: &str) -> Option<String> {
    let line = raw.lines().find(|l| !l.trim().is_empty())?.trim();
    let line = line.strip_prefix("Title:").or_else(|| line.strip_prefix("Name:")).unwrap_or(line);
    let line = line.trim().trim_matches(|c| matches!(c, '"' | '\'' | '`' | '*' | '#')).trim();
    let line = line.trim_end_matches(['.', '!']).trim();
    if line.is_empty() || line.chars().count() > 60 {
        return None;
    }
    Some(line.to_string())
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
    Opencode {
        server: Arc<OpencodeServer>,
        session: String,
        /// What variant this session was created with. opencode binds it at
        /// session creation, like the other two bind their level, so asking
        /// for a different one means a new session.
        variant: Option<String>,
    },
    Antigravity {
        session: Arc<AntigravitySession>,
        /// `--effort` is a process flag here as it is for Claude, so a level
        /// changed mid-thread costs a respawn.
        effort: Option<String>,
    },
}

impl Live {
    fn is_alive(&self) -> bool {
        match self {
            Live::Claude { session, .. } => session.is_alive(),
            Live::Codex { server, thread_id, .. } => server.is_alive() && server.has_thread(thread_id),
            Live::Opencode { server, session, .. } => server.is_alive() && server.has_session(session),
            Live::Antigravity { session, .. } => session.is_alive(),
        }
    }

    /// The reasoning level this session is already running under.
    fn effort(&self) -> Option<&str> {
        match self {
            Live::Claude { effort, .. } => effort.as_deref(),
            Live::Codex { opts, .. } => opts.reasoning_effort.as_deref(),
            Live::Opencode { variant, .. } => variant.as_deref(),
            Live::Antigravity { effort, .. } => effort.as_deref(),
        }
    }
}

/// A session lifted out of the map so it can be talked to without holding it.
enum Rewindable {
    Claude(Arc<ClaudeSession>),
    Codex(Arc<CodexServer>, String),
    Opencode(Arc<OpencodeServer>, String),
    /// Refuses. Antigravity's protocol has no way back to an earlier message,
    /// and the session says so rather than answering `Ok(())` to a caller that
    /// is about to delete rows on the strength of it.
    Antigravity(Arc<AntigravitySession>),
}

/// The set of live sessions plus the shared Codex server. One per app.
pub struct Harness {
    data_dir: PathBuf,
    live: Mutex<HashMap<i64, Live>>,
    codex: Mutex<Option<Arc<CodexServer>>>,
    /// The shared opencode server, the same shape as the Codex one: one
    /// process for the app, one session per thread inside it.
    opencode: Mutex<Option<Arc<OpencodeServer>>>,
    /// Codex events that belong to the account rather than to any one
    /// thread — the rate-limit windows, which the shared server reports with
    /// no `threadId` on them. Claude needs no equivalent: its processes are
    /// one per thread, so its windows already arrive on a thread's stream.
    /// Set by the app; `None` headless, where nothing is listening.
    codex_account_sink: Mutex<Option<Sink>>,
    /// Where opencode's session-less events go — thirty of its eighty-eight
    /// types name no session, and `session.error`'s own id is optional. Same
    /// role as the Codex account sink above, and the same thread id 0.
    opencode_default_sink: Mutex<Option<Sink>>,
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
        }
    }

    /// Where Codex's account-scoped events go, set once at startup. It is on
    /// the harness rather than on a session because the fact it carries — how
    /// much of the plan is spent — outlives every thread that reports it, and
    /// the server that reports it is shared by all of them.
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
        // Seed the meter off the pull, so the windows are current from the
        // moment the server is up rather than from the first turn. Off the
        // caller's thread: this runs inside the first send.
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

    /// Re-read the windows on a server that is already up — what the Chat
    /// page asks for when it opens. It never starts the server to answer:
    /// a page visit is not a reason to spawn a CLI, and a server that has
    /// just started has already seeded itself above.
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

    /// Antigravity's catalogue. No server and no session behind it — `agy
    /// models` is a listing subcommand, so this is one short-lived process
    /// and nothing is spent.
    pub fn antigravity_models(&self) -> Result<Vec<antigravity::ModelInfo>, String> {
        let bin = discover::binary(Provider::Antigravity)?;
        antigravity::list_models(&bin, &discover::child_env())
    }

    /// The shared opencode server, started on first use.
    ///
    /// The config it runs under is rendered first, every time. That document
    /// is both the containment ruleset and the only way to give opencode a
    /// system prompt (`opencode::write_config`), and it has the library's
    /// real paths and the course folders on disk in it — so a stale one is a
    /// stale brief *and* a stale deny list.
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
            NAMING_INSTRUCTIONS,
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
    // Claude Code and Codex carry the student's own subscription and are
    // signed in with their own CLIs; opencode carries whatever
    // `opencode auth` holds, and until this existed the only way to put
    // something there was a terminal. Every call below goes through the
    // server's own auth endpoints (`opencode.rs`), so the credential lands
    // in opencode's store and is the same one the student's terminal
    // opencode uses. Nothing is mirrored into `settings`, the keychain or a
    // thread log.
    //
    // This does **not** soften [`discover::child_env`]'s strip of
    // `ANTHROPIC_API_KEY` / `OPENAI_API_KEY`. That strip exists so the
    // catalogue is what `opencode auth` holds rather than whatever is in a
    // shell; writing through `PUT /auth` *is* that store, so the rule stands
    // exactly as it did.
    //
    // All of them start the server if it is down — they are the answer to a
    // button the student pressed, not to a page being opened, which is the
    // same line `harness_opencode_models` sits on and the opposite of the
    // rate-limit read.

    pub fn opencode_providers(&self, refresh: bool) -> Result<ProviderList, String> {
        let server = self.opencode_server()?;
        // A refusal is a running turn: the list is then what the instance
        // last read, which may predate a credential written since.
        let stale = refresh && !server.refresh();
        Ok(ProviderList {
            providers: server.list_providers()?,
            stale,
        })
    }

    /// The key travels as an argument and a request body and is gone when
    /// this returns.
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
        match live.get(&thread_id).ok_or("no session")? {
            Live::Claude { session, .. } => session.send(text),
            Live::Codex {
                server,
                thread_id: tid,
                opts,
            } => server.start_turn(tid, text, opts),
            Live::Opencode { server, session, .. } => server.prompt(session, text),
            Live::Antigravity { session, .. } => session.send(text),
        }
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
        self.opencode_server()?.oauth_callback(provider, method, code)?;
        self.opencode_providers(true)
    }

    /// Take a question and everything after it out of the provider's own
    /// session, so the agent's context matches the thread the student is
    /// reading. `anchor` is the provider's handle for that question, kept on
    /// its row when the turn went out.
    ///
    /// A thread whose process has gone is resumed for this, without a turn:
    /// both bridges take the instruction on their control channel, which is
    /// live as soon as the session is, so nothing is spent on the model.
    pub fn rewind(
        &self,
        thread_id: i64,
        provider: Provider,
        resume: Option<&str>,
        opts: &SendOptions,
        anchor: &str,
        sink: Sink,
    ) -> Result<(), String> {
        // The handle is taken out from under the lock and the rewind done
        // outside it: a rewind waits on the CLI's answer, and holding the map
        // for that would stall every *other* thread's next message behind it.
        let handle = {
            let mut live = self.live.lock().unwrap();
            // Any live session will do. `ensure` would respawn one running
            // under a different reasoning level, which matters for a turn and
            // not at all for an instruction on the control channel.
            if !live.get(&thread_id).is_some_and(|l| l.is_alive()) {
                self.ensure(&mut live, thread_id, provider, resume, opts, sink)?;
            }
            match live.get(&thread_id).ok_or("no session")? {
                Live::Claude { session, .. } => Rewindable::Claude(session.clone()),
                Live::Codex {
                    server,
                    thread_id: tid,
                    ..
                } => Rewindable::Codex(server.clone(), tid.clone()),
                Live::Opencode { server, session, .. } => {
                    Rewindable::Opencode(server.clone(), session.clone())
                }
                Live::Antigravity { session, .. } => Rewindable::Antigravity(session.clone()),
            }
        };
        match handle {
            Rewindable::Claude(s) => s.rewind(anchor),
            Rewindable::Codex(server, tid) => server.revert(&tid, anchor),
            Rewindable::Opencode(server, ses) => server.revert(&ses, anchor),
            Rewindable::Antigravity(s) => s.rewind(anchor),
        }
    }

    /// Make sure this thread has a session that can be talked to, spawning or
    /// resuming one when it has none.
    ///
    /// A live session is reused only if it is running under the level asked
    /// for; both CLIs bind the reasoning level at session start, so a
    /// different one means a new process (Claude) or a new thread (Codex).
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
            .is_some_and(|l| l.is_alive() && l.effort() == opts.reasoning_effort.as_deref())
        {
            return Ok(());
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
                        oculus: discover::oculus_cli(),
                        resume: resume.map(String::from),
                        model: opts.model.clone(),
                        effort: opts.reasoning_effort.clone(),
                        permission_mode: "acceptEdits".into(),
                        system_append: instructions(&self.data_dir, opts.scope.as_deref(), opts.lecture.as_ref()),
                        env: discover::child_env(),
                        raw_log,
                    },
                    sink,
                )?;
                Live::Claude {
                    session: s,
                    effort: opts.reasoning_effort.clone(),
                }
            }
            Provider::Codex => {
                let server = self.codex_server()?;
                let topts = CodexThreadOpts {
                    cwd,
                    // Existing files only: Codex stats every writable root
                    // and refuses to run a command whose root it cannot
                    // inspect ("failed to inspect Seatbelt writable root"),
                    // so a missing WAL sidecar would cost the whole turn
                    // rather than one write. It is missing only when nothing
                    // holds the database open — and then the CLI would need a
                    // grant on the folder to create it, which this
                    // deliberately does not give.
                    writable_files: crate::paths::db_write_paths(&self.data_dir)
                        .into_iter()
                        .filter(|p| p.exists())
                        .collect(),
                    model: opts.model.clone(),
                    reasoning_effort: opts.reasoning_effort.clone(),
                    instructions: instructions(&self.data_dir, opts.scope.as_deref(), opts.lecture.as_ref()),
                };
                let tid = match resume {
                    Some(id) => {
                        server.resume_thread(id, &topts, sink)?;
                        id.to_string()
                    }
                    None => server.start_thread(&topts, sink)?,
                };
                Live::Codex {
                    server,
                    thread_id: tid,
                    opts: topts,
                }
            }
            Provider::Opencode => {
                let server = self.opencode_server()?;
                let sopts = OpencodeSessionOpts {
                    model: opts.model.clone(),
                    // Only ever set when the picker offered a level, which
                    // means the model declared one. Nothing is invented: in
                    // 1.18.2 no model declares any, so this is None and the
                    // session takes the model's own default.
                    variant: opts.reasoning_effort.clone(),
                    // The library-wide brief is the agent's prompt in the
                    // config; this is the rest of it, and it rides the
                    // session's first message because there is nowhere else
                    // to put it.
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
                Live::Opencode {
                    server,
                    session: ses,
                    variant: opts.reasoning_effort.clone(),
                }
            }
            Provider::Antigravity => {
                let s = AntigravitySession::spawn(
                    AntigravitySpawn {
                        bin: discover::binary(provider)?,
                        cwd,
                        library: self.data_dir.clone(),
                        resume: resume.map(String::from),
                        model: opts.model.clone(),
                        effort: opts.reasoning_effort.clone(),
                        // Only the per-thread half. The library-wide brief is
                        // already `agents/AGENTS.md`, which `agy` reads for
                        // itself from the directory it is spawned in — the
                        // same free ride Codex gets, and the reason there is
                        // no `--append-system-prompt` to miss.
                        brief: thread_sections(opts.scope.as_deref(), opts.lecture.as_ref()),
                        env: discover::child_env(),
                        raw_log,
                    },
                    sink,
                )?;
                Live::Antigravity {
                    session: s,
                    effort: opts.reasoning_effort.clone(),
                }
            }
        };
        live.insert(thread_id, session);
        Ok(())
    }

    /// Ask the provider to name a thread from its first exchange.
    ///
    /// Neither CLI names a conversation on its own: Claude's stream-json has
    /// no title event, and `app-server` sends none either — so a name that
    /// the model wrote has to be asked for, and it costs one turn. It is
    /// asked once, after the first exchange (`store::claim_naming`), on the
    /// agent, model and level the `threadNaming` job is configured with
    /// (`jobs.rs`) — not on the thread's own. Naming is a job like the
    /// chaptering one, and a student who has picked a namer in Settings has
    /// said which CLI should pay for it.
    ///
    /// It runs outside the thread: its own short-lived Claude process, or a
    /// throwaway thread on the shared Codex server. Sending it down the
    /// thread's own session would put a question the student never asked into
    /// the timeline, and would spend the thread's context on it.
    pub fn name_thread(
        &self,
        sel: &jobs::JobSelection,
        first_message: &str,
        reply: &str,
    ) -> Result<String, String> {
        let provider = sel.provider;
        let prompt = naming_prompt(first_message, reply);
        let (tx, rx) = mpsc::channel::<HarnessEvent>();
        let sink: Sink = Arc::new(move |ev| {
            let _ = tx.send(ev);
        });
        let cwd = thread_cwd(&self.data_dir);

        // Held so the session outlives the collect loop, and dropped after it.
        let claude;
        let codex;
        let oc;
        let agy;
        match provider {
            Provider::Claude => {
                let s = ClaudeSession::spawn(
                    ClaudeSpawn {
                        bin: discover::binary(provider)?,
                        cwd,
                        library: self.data_dir.clone(),
                        oculus: discover::oculus_cli(),
                        resume: None,
                        model: Some(sel.model.clone()),
                        effort: sel.reasoning_effort.clone(),
                        // Nothing here needs a tool, and `default` auto-allows
                        // none — with prompts routed to `none` a stray call is
                        // refused rather than hanging the turn.
                        permission_mode: "default".into(),
                        system_append: String::new(),
                        env: discover::child_env(),
                        raw_log: None,
                    },
                    sink,
                )?;
                s.send(&prompt)?;
                claude = Some(s);
                codex = None;
                oc = None;
                agy = None;
            }
            Provider::Codex => {
                let server = self.codex_server()?;
                let opts = CodexThreadOpts {
                    cwd,
                    // The namer writes nothing: it is one question with no
                    // tool worth reaching for, so it gets no hole in the
                    // sandbox either.
                    writable_files: Vec::new(),
                    model: Some(sel.model.clone()),
                    reasoning_effort: sel.reasoning_effort.clone(),
                    instructions: NAMING_INSTRUCTIONS.into(),
                };
                let tid = server.start_thread(&opts, sink)?;
                server.start_turn(&tid, &prompt, &opts)?;
                claude = None;
                codex = Some((server, tid));
                oc = None;
                agy = None;
            }
            Provider::Opencode => {
                let server = self.opencode_server()?;
                // A throwaway session on the shared server, exactly as for
                // Codex — and on the hidden `oculus-namer` agent, because
                // opencode has no per-session instructions and the naming
                // brief cannot ride the `oculus` agent's own prompt.
                let sopts = OpencodeSessionOpts {
                    model: Some(sel.model.clone()),
                    variant: sel.reasoning_effort.clone(),
                    brief: String::new(),
                    agent: opencode::NAMING_AGENT,
                };
                let ses = server.start_session(&sopts, sink)?;
                server.prompt(&ses, &prompt)?;
                claude = None;
                codex = None;
                oc = Some((server, ses));
                agy = None;
            }
            Provider::Antigravity => {
                // A throwaway process, the way Claude's namer is one. It gets
                // no brief and asks for no tool: naming a thread is a single
                // question about text that is already in the prompt.
                let s = AntigravitySession::spawn(
                    AntigravitySpawn {
                        bin: discover::binary(provider)?,
                        cwd,
                        library: self.data_dir.clone(),
                        resume: None,
                        model: Some(sel.model.clone()),
                        effort: sel.reasoning_effort.clone(),
                        brief: String::new(),
                        env: discover::child_env(),
                        raw_log: None,
                    },
                    sink,
                )?;
                s.send(&prompt)?;
                claude = None;
                codex = None;
                oc = None;
                agy = Some(s);
            }
        }

        let mut text = String::new();
        let mut failed: Option<String> = None;
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(NAMING_TIMEOUT_SECS)) {
                Ok(HarnessEvent::AssistantMessage { text: t }) => text.push_str(&t),
                Ok(HarnessEvent::Error { message, .. }) => failed = Some(message),
                Ok(HarnessEvent::TurnFinished { .. }) => break,
                Ok(HarnessEvent::Exited { code }) => {
                    failed.get_or_insert(format!("provider exited (code {code:?}) before naming the thread"));
                    break;
                }
                Ok(_) => {}
                Err(_) => {
                    failed.get_or_insert_with(|| "timed out naming the thread".into());
                    break;
                }
            }
        }
        if let Some(s) = claude {
            s.kill();
        }
        if let Some(s) = agy {
            s.kill();
        }
        if let Some((server, tid)) = codex {
            server.detach(&tid);
        }
        // Deleted rather than detached: a naming session is a question the
        // student never asked, and leaving it on the server would put it in
        // their own `opencode` session list for ever.
        if let Some((server, ses)) = oc {
            server.delete_session(&ses);
        }
        match (clean_title(&text), failed) {
            (Some(t), _) => Ok(t),
            (None, Some(e)) => Err(e),
            (None, None) => Err(format!("no usable name in the reply: {text:?}")),
        }
    }

    pub fn interrupt(&self, thread_id: i64) -> Result<(), String> {
        let live = self.live.lock().unwrap();
        match live.get(&thread_id) {
            Some(Live::Claude { session, .. }) => session.interrupt(),
            Some(Live::Codex { server, thread_id, .. }) => server.interrupt(thread_id),
            Some(Live::Opencode { server, session, .. }) => server.interrupt(session),
            Some(Live::Antigravity { session, .. }) => session.interrupt(),
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
                Live::Opencode { server, session, .. } => server.detach(&session),
                Live::Antigravity { session, .. } => session.kill(),
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
        if let Some(s) = self.opencode.lock().unwrap().take() {
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
            HarnessEvent::Error { message, .. } => failed = Some(message.clone()),
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
    /// plus the row id when the event became a row. The provider is on it
    /// too, because an account-scoped event (rate limits) has no thread to
    /// read it off — it arrives with `threadId` 0.
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

    /// One consumer thread folds every event, from every thread, in order:
    /// a row is written before the webview hears about it, and a tool's
    /// finish can never overtake its start.
    pub fn init(app: &AppHandle) -> HarnessState {
        let (tx, rx) = mpsc::channel::<(i64, Provider, HarnessEvent)>();
        let handle = app.clone();
        let harness = Arc::new(Harness::new(crate::paths::data_dir()));
        let queue: Arc<Mutex<Queue>> = Arc::new(Mutex::new(Queue::default()));
        // Codex's rate-limit windows come off the shared server with no thread
        // attached; thread id 0 is the same id its raw log uses.
        {
            let bus = tx.clone();
            harness.set_codex_account_sink(Arc::new(move |ev| {
                let _ = bus.send((0, Provider::Codex, ev));
            }));
        }
        // opencode's session-less events, on the same thread id 0 its raw log
        // uses.
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
                    // A finished first exchange is when a thread can be named.
                    // The claim is atomic, so this asks at most once; the turn
                    // itself takes seconds and cannot run on this loop, which
                    // every other thread's events are waiting behind.
                    if thread_id > 0 && matches!(&ev, HarnessEvent::TurnFinished { status } if status == "completed") {
                        match rt.block_on(store::claim_naming(p, thread_id)) {
                            Ok(Some(seed)) => {
                                // Read here rather than in the naming thread:
                                // the pool is already open on this loop, and
                                // the read is one indexed row.
                                let sel = rt.block_on(jobs::selection(p, jobs::Job::ThreadNaming));
                                let (namer, bus) = (namer.clone(), bus.clone());
                                std::thread::spawn(move || {
                                    match namer.name_thread(&sel, &seed.first_message, &seed.reply) {
                                        Ok(title) => {
                                            let _ = bus.send((thread_id, provider, HarnessEvent::ThreadTitled { title }));
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
                // A closed turn is when the next message waiting on this
                // thread may go. It cannot go from here — every other
                // thread's events queue behind this loop, and a send starts
                // a process — so the dispatch is spawned and this moves on.
                if thread_id > 0 && matches!(&ev, HarnessEvent::TurnFinished { .. }) {
                    let next = queued.lock().unwrap().next(thread_id);
                    if let Some((msg, opts)) = next {
                        let (h, b) = (namer.clone(), bus.clone());
                        let _ = b.send((thread_id, provider, HarnessEvent::Unqueued { id: msg.id }));
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = dispatch(h, b, thread_id, provider, opts, msg.text).await {
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

    /// Everything a send does once the thread exists and the queue has said
    /// it may go: resolve what the *row* says rather than what the payload
    /// asked for, write the question through the event path like any other
    /// row, and hand the text to the bridge.
    ///
    /// A failure here closes the turn it never opened — the error becomes a
    /// row and a `TurnFinished` releases the thread — so a send that cannot
    /// start does not leave whatever was queued behind it stranded.
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

    /// The lecture section of a thread's instructions, assembled off its row.
    ///
    /// Read on every send rather than carried on the thread, for the same
    /// reason the subject's folder name is joined rather than stored: the
    /// chapters can land eight minutes after the conversation started, and a
    /// brief built once at creation would never grow them. A lecture with no
    /// chapters simply has none in the brief.
    ///
    /// It has to be resolved before *any* session is spawned, including the
    /// one a rewind brings back up — both CLIs bind the appended instructions
    /// at session start, so a session opened without it would keep answering
    /// without it for the rest of the thread.
    async fn lecture_brief(pool: &SqlitePool, row: &store::ThreadRow) -> Option<LectureBrief> {
        let l = row.lecture.as_ref()?;
        Some(LectureBrief {
            id: l.id.clone(),
            title: l.title.clone(),
            date: l.date.clone(),
            has_transcript: l.has_transcript,
            chapters: crate::store::chapters(pool, &l.id).await.unwrap_or_default(),
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
            return Err(format!("thread {thread_id} is a {} thread", row.provider.label()));
        }
        if opts.model.is_some() && opts.model != row.model {
            store::set_model(&pool, thread_id, opts.model.as_deref()).await?;
            // A different model means a different Claude process. Safe here
            // and not at the moment the student picked it: the thread is
            // between turns, so nothing is killed mid-answer.
            if provider == Provider::Claude {
                harness.close(thread_id);
            }
        }
        // The row, not the payload, decides all three: an open thread keeps
        // the model it was last set to, the subject it was created with, and
        // the lecture it was opened over.
        let lecture = lecture_brief(&pool, &row).await;
        let opts = SendOptions {
            model: opts.model.or(row.model),
            scope: row.subject_code,
            lecture,
            ..opts
        };
        let resume = row.provider_session_id;
        // The row is what the student typed. The moment they sent it at is a
        // fact about the message and rides on the event; the moment's own
        // text rides the prompt below and is never a row, or the timeline
        // would read back a transcript excerpt as the question.
        sink(HarnessEvent::UserMessage {
            text: text.to_string(),
            at: opts.at,
        });
        let text = match &opts.context {
            Some(c) => format!("{text}\n\n---\n\n## The moment this was sent at\n\n{c}"),
            None => text.to_string(),
        };

        let (h, sink) = (harness.clone(), sink.clone());
        tokio::task::spawn_blocking(move || {
            h.send(thread_id, provider, resume.as_deref(), &opts, &text, sink)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Where each CLI is, and whether it is there at all.
    ///
    /// `recheck` is the difference between Settings' *Recheck* button and
    /// everything else. Only that button drops the cached lookups
    /// (`discover::forget`), because a full recheck can end in a login shell's
    /// `command -v` per provider — a few hundred milliseconds each, and this
    /// is now read by every model picker in the app rather than by one
    /// settings page. Without `recheck` the answer comes off
    /// `discover::health`'s cache, so opening a composer costs nothing after
    /// the first read of the session.
    #[tauri::command]
    pub async fn harness_health(recheck: bool) -> Vec<discover::BridgeHealth> {
        tokio::task::spawn_blocking(move || {
            if recheck {
                discover::forget();
            }
            discover::PROVIDERS.iter().map(|p| discover::health(*p)).collect()
        })
        .await
        .unwrap_or_default()
    }

    /// The four ways this machine could install one of the CLIs, and which of
    /// them it can actually run. Detection asks a login shell, so it is
    /// blocking and cached (`discover::tool`); *Recheck* drops that cache
    /// alongside everything else.
    #[tauri::command]
    pub async fn harness_install_offer(provider: Provider) -> install::InstallOffer {
        tokio::task::spawn_blocking(move || install::offer(provider, install::detect()))
            .await
            // A join that failed is not evidence that the machine is bare, but
            // it is the only safe thing to draw: commands to copy, no buttons.
            .unwrap_or_else(|_| install::offer(provider, install::Managers::default()))
    }

    /// Run one of them. The webview names a provider and a manager and never
    /// the command — `install.rs` owns the string, and that is the whole
    /// reason this takes an enum rather than the text the dialog is showing.
    ///
    /// Output streams on `install::INSTALL_EVENT` the way every other job's
    /// progress reaches the webview, and the run's last event is the one with
    /// `done` on it: that is where Settings rechecks, because
    /// `discover::binary` caches its failures and a freshly installed CLI
    /// stays missing until both caches are dropped.
    #[tauri::command]
    pub async fn harness_install_run(
        app: AppHandle,
        provider: Provider,
        manager: install::Manager,
    ) -> Result<(), String> {
        tokio::task::spawn_blocking(move || {
            let emitter = app.clone();
            install::start(provider, manager, move |line| {
                emitter.emit(install::INSTALL_EVENT, line).ok();
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// Whether a provider has credentials, asked of the provider.
    ///
    /// Read in two places and cached in neither: the Settings → AI row, and
    /// the sign-in card the timeline draws over an auth error. Both are places
    /// a student is looking *because* something is wrong, so a cached "signed
    /// out" that outlived the sign-in that fixed it would be the one answer
    /// that must not be stale — which is why this differs from
    /// `harness_health` and spawns the CLI every time. `discover::forget` has
    /// nothing of this to drop.
    ///
    /// opencode answers `signedIn: null`: its credentials are per provider and
    /// live behind `harness_opencode_providers`, not behind a login command.
    #[tauri::command]
    pub async fn harness_sign_in_status(provider: Provider) -> signin::SignInStatus {
        tokio::task::spawn_blocking(move || signin::status(provider))
            .await
            // A join that failed is not evidence about the account either way,
            // and "cannot say" is exactly what `signed_in: None` means.
            .unwrap_or_else(|e| signin::SignInStatus {
                provider,
                signed_in: None,
                account: None,
                error: Some(e.to_string()),
            })
    }

    /// Run the provider's own login flow, and open the page it points at.
    ///
    /// Output streams on `signin::SIGNIN_EVENT` the way an install's streams
    /// on `install::INSTALL_EVENT`. The one addition is the URL: the first
    /// line that carries one opens it **in the system browser**
    /// (`tauri_plugin_opener`), because the student is almost certainly
    /// already signed in to claude.com or chatgpt.com there, while this app's
    /// own in-app browser (`browser.rs`) has a cookie jar seeded for Canvas
    /// and nothing else. The URL rides the event as well as being opened, so
    /// the dialog can show it with a Copy button — an `open` that silently did
    /// nothing must not be a dead end.
    #[tauri::command]
    pub async fn harness_sign_in_start(app: AppHandle, provider: Provider) -> Result<(), String> {
        tokio::task::spawn_blocking(move || {
            let emitter = app.clone();
            signin::start(provider, move |line| {
                if let Some(url) = line.url.as_deref() {
                    tauri_plugin_opener::open_url(url, None::<&str>).ok();
                }
                emitter.emit(signin::SIGNIN_EVENT, line).ok();
            })
        })
        .await
        .map_err(|e| e.to_string())?
    }

    /// The code Claude's flow ends on, pasted back from the browser. Codex
    /// finishes on its own loopback callback and never reaches this.
    #[tauri::command]
    pub async fn harness_sign_in_code(provider: Provider, code: String) -> Result<(), String> {
        tokio::task::spawn_blocking(move || signin::submit_code(provider, &code))
            .await
            .map_err(|e| e.to_string())?
    }

    /// The student closed the dialog. Nothing else ends a login — an OAuth
    /// page can sit open for as long as a person takes, so there is no
    /// deadline above this to expire.
    #[tauri::command]
    pub async fn harness_sign_in_cancel(provider: Provider) -> Result<(), String> {
        tokio::task::spawn_blocking(move || signin::cancel(provider))
            .await
            .map_err(|e| e.to_string())?
    }

    /// The Chat page, on open and on a provider switch. Codex answers a read
    /// for its plan windows; Claude has no such request over `stream-json`,
    /// so its windows keep arriving with a turn and this is a no-op for it.
    #[tauri::command]
    pub async fn harness_refresh_rate_limits(
        state: State<'_, HarnessState>,
        provider: Provider,
    ) -> Result<(), String> {
        if provider != Provider::Codex {
            return Ok(());
        }
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.refresh_codex_rate_limits())
            .await
            .map_err(|e| e.to_string())
    }

    #[tauri::command]
    pub async fn harness_codex_models(state: State<'_, HarnessState>) -> Result<Vec<ModelInfo>, String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.codex_models())
            .await
            .map_err(|e| e.to_string())?
    }

    #[tauri::command]
    pub async fn harness_antigravity_models(
        state: State<'_, HarnessState>,
    ) -> Result<Vec<antigravity::ModelInfo>, String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.antigravity_models())
            .await
            .map_err(|e| e.to_string())?
    }

    /// opencode's catalogue, for the same picker. It starts the server if it
    /// is not up — unlike the rate-limit read, this one is the answer to a
    /// question the picker asked and there is no cached list behind it.
    #[tauri::command]
    pub async fn harness_opencode_models(
        state: State<'_, HarnessState>,
    ) -> Result<Vec<opencode::ModelInfo>, String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.opencode_models())
            .await
            .map_err(|e| e.to_string())?
    }

    // ── opencode credentials ─────────────────────────────────────────────
    //
    // Settings → AI's provider list and the dialog behind it. Each of these
    // starts the server if it is down, so none of them is called on a page
    // opening: the section draws a button first and the student's click is
    // what spawns opencode. `harness_health` already says whether the binary
    // exists at all, and it costs nothing, so the section can degrade to
    // "not installed" without any of this running.
    //
    // A credential never comes back out. `opencode_set_api_key` takes the
    // key and answers with the provider list; there is no read side, because
    // the app has no reason to know a key it has already handed over.

    #[tauri::command]
    pub async fn harness_opencode_providers(
        state: State<'_, HarnessState>,
        refresh: bool,
    ) -> Result<opencode::ProviderList, String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.opencode_providers(refresh))
            .await
            .map_err(|e| e.to_string())?
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
        tokio::task::spawn_blocking(move || h.opencode_set_api_key(&provider, method, &key, &answers))
            .await
            .map_err(|e| e.to_string())?
    }

    #[tauri::command]
    pub async fn harness_opencode_disconnect(
        state: State<'_, HarnessState>,
        provider: String,
    ) -> Result<opencode::ProviderList, String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.opencode_disconnect(&provider))
            .await
            .map_err(|e| e.to_string())?
    }

    /// Start a browser flow and hand back the URL to open, whether the
    /// server finishes it by itself (`auto`) and what to tell the student.
    /// The webview opens the URL in the *system* browser — they are far more
    /// likely to be signed in to GitHub or OpenAI there than in the app's
    /// in-app one.
    #[tauri::command]
    pub async fn harness_opencode_oauth_start(
        state: State<'_, HarnessState>,
        provider: String,
        method: usize,
        answers: Option<BTreeMap<String, String>>,
    ) -> Result<opencode::Authorization, String> {
        let h = state.harness.clone();
        let answers = answers.unwrap_or_default();
        tokio::task::spawn_blocking(move || h.opencode_oauth_authorize(&provider, method, &answers))
            .await
            .map_err(|e| e.to_string())?
    }

    /// Finish a `code` flow. An `auto` one never calls this — its credential
    /// is written by the server's own listener, and the dialog finds out by
    /// polling `harness_opencode_providers` with `refresh`.
    #[tauri::command]
    pub async fn harness_opencode_oauth_finish(
        state: State<'_, HarnessState>,
        provider: String,
        method: usize,
        code: Option<String>,
    ) -> Result<opencode::ProviderList, String> {
        let h = state.harness.clone();
        tokio::task::spawn_blocking(move || h.opencode_oauth_callback(&provider, method, code.as_deref()))
            .await
            .map_err(|e| e.to_string())?
    }

    /// Send a message; creates the thread when `thread_id` is null. Returns
    /// the thread id.
    ///
    /// A message sent while the thread is working does not reach the CLI: it
    /// waits in the [`Queue`], and the `queued` event is all the webview
    /// gets until the running turn ends and it goes out for real. The user's
    /// row is written by the event path like every other row, so the
    /// timeline sees the question in order with the answer it gets.
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
        dispatch(state.harness.clone(), state.bus.clone(), id, provider, opts, text).await?;
        Ok(id)
    }

    /// Ask a question again, differently: the thread is rewound to that
    /// question — it and everything after it stop being rows, and the agent
    /// is told to forget them too — and the new text is sent as the next
    /// turn.
    ///
    /// The agent's half is [`Harness::rewind`], and it is done first: if the
    /// provider will not rewind, its context and the timeline would disagree,
    /// and the timeline is the thing the student is about to reason from. It
    /// is not fatal, though — a question asked before the anchor was recorded
    /// has nothing to name, and a session the CLI has since dropped cannot be
    /// resumed — so the rewind falls back to our rows alone and says so on
    /// the event, which is what the timeline's note is drawn from.
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
        // The id comes from the webview and everything from it on is about to
        // be deleted, so it is checked against the row it names.
        let question = store::user_item(&pool, thread_id, item_id).await?;
        // Rewinding under a running turn would delete rows it is still
        // writing, and the CLI would answer the old question anyway.
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

    /// Ask the provider to forget this question and everything after it.
    /// Answers whether it did.
    ///
    /// A thread with no anchor on the row is one whose question predates the
    /// column (migration 28) or whose turn never started; there is nothing to
    /// name, and no amount of retrying will produce one. A provider that
    /// refuses, or a session that can no longer be resumed, lands the same
    /// way: the rows still go, and the `false` travels to the timeline so the
    /// student is told the agent kept the original rather than finding out
    /// from an answer that refers to it.
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
        // A rewind will resume a thread whose process has gone, and that
        // spawn binds the instructions for every turn after it — so the
        // lecture brief is resolved here too, not only on a send.
        let opts = SendOptions {
            model: opts.model.clone().or_else(|| row.model.clone()),
            scope: row.subject_code.clone(),
            lecture: lecture_brief(pool, row).await,
            ..opts.clone()
        };
        tokio::task::spawn_blocking(move || {
            h.rewind(thread_id, provider, Some(&resume), &opts, &anchor, sink)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r)
        .is_ok()
    }

    /// Take the thread back to just before a question: it and everything
    /// after it stop being rows, and the question comes back as text for the
    /// composer to hold. Claude Code's rewind, without the branching — there
    /// is one thread, and going back means the rest is gone.
    ///
    /// Nothing is sent. That is the whole difference from
    /// [`harness_edit_resend`]: rewinding is for picking the conversation up
    /// again yourself, which is why the words are handed back rather than
    /// put straight to the agent. The agent forgets either way.
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

    /// What is still waiting behind this thread's turn. The queue is in
    /// memory — a message that was never sent is not history — so this is
    /// how a reloaded page finds out it is there.
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

    /// Rewrite one that has not gone out yet. It arrives back as a `queued`
    /// event under the same id, which is how the webview knows to replace it
    /// rather than add another.
    #[tauri::command]
    pub async fn harness_edit_queued(
        state: State<'_, HarnessState>,
        thread_id: i64,
        queue_id: String,
        text: String,
    ) -> Result<(), String> {
        let edited = state.queue.lock().unwrap().edit(thread_id, &queue_id, &text);
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

    /// Stop the running turn, and drop whatever was waiting behind it —
    /// stop means nothing more goes out, not "one more first". The dropped
    /// messages are handed back so the composer can return them to the
    /// student, who typed them and never saw them sent.
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
        tokio::task::spawn_blocking(move || h.interrupt(thread_id))
            .await
            .map_err(|e| e.to_string())??;
        Ok(cleared.into_iter().map(|m| m.text).collect())
    }

    #[tauri::command]
    pub async fn harness_delete_thread(state: State<'_, HarnessState>, thread_id: i64) -> Result<(), String> {
        state.harness.close(thread_id);
        state.queue.lock().unwrap().forget(thread_id);
        let pool = crate::store::open_pool().await?;
        store::delete_thread(&pool, thread_id).await
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

    /// A model asked for a name alone mostly gives one, and sometimes dresses
    /// it up. What it dresses it in is stripped; a whole sentence is refused,
    /// because the first-line title it would replace is better than prose.
    #[test]
    fn a_name_is_taken_out_of_whatever_the_model_wrapped_it_in() {
        assert_eq!(clean_title("Dijkstra worked example").as_deref(), Some("Dijkstra worked example"));
        assert_eq!(clean_title("\"Week 6 tutorial questions\"\n").as_deref(), Some("Week 6 tutorial questions"));
        assert_eq!(clean_title("Title: **Semaphores and deadlock**").as_deref(), Some("Semaphores and deadlock"));
        assert_eq!(clean_title("Assignment 2 marking scheme.").as_deref(), Some("Assignment 2 marking scheme"));
        assert_eq!(clean_title(""), None);
        assert_eq!(clean_title("   \n\n "), None);
        assert_eq!(
            clean_title("The student asks about the difficulty of the week 6 lecture and the assistant replies"),
            None,
            "prose is refused rather than becoming the name"
        );
    }

    /// One turn per thread, and the rest in the order they were typed. The
    /// queue is the whole of what makes a message sent mid-turn behave: both
    /// CLIs would take it immediately, and both would ruin the thread doing
    /// it (`Queue`'s own docs).
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

        assert_eq!(q.next(1).map(|(m, _)| m), Some(a), "in the order they were typed");
        assert!(!q.try_claim(1), "the thread stays claimed while one is going out");
        assert_eq!(q.next(1).map(|(m, _)| m.text), Some("second".into()));
        assert!(q.next(1).is_none(), "nothing left");
        assert!(q.try_claim(1), "and the thread is free again");
        let _ = b;
    }

    /// Stop means nothing more goes out — and hands back what was waiting,
    /// because the student typed those words and never saw them sent.
    #[test]
    fn stopping_clears_the_queue_and_returns_what_it_held() {
        let mut q = Queue::default();
        let opts = SendOptions::default();
        q.try_claim(7);
        q.push(7, "one", &opts);
        let two = q.push(7, "two", &opts);
        q.push(7, "three", &opts);
        assert!(q.remove(7, &two.id), "a pending message can be dropped on its own");
        assert_eq!(
            q.clear(7).into_iter().map(|m| m.text).collect::<Vec<_>>(),
            vec!["one".to_string(), "three".to_string()]
        );
        assert!(q.list(7).is_empty());
        // The turn itself is still running: only its `TurnFinished` releases
        // the thread, and it finds nothing waiting.
        assert!(!q.try_claim(7));
        assert!(q.next(7).is_none());
        assert!(q.try_claim(7));
    }

    /// The scope is appended, not substituted: a scoped thread gets the whole
    /// library brief *and* the subject it is about, because it still reads
    /// across `courses/` and still writes only to `agents/`.
    #[test]
    fn a_scoped_thread_keeps_the_library_brief_and_names_its_folder() {
        let root = std::env::temp_dir().join("oculus-harness-scope");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("courses/COMP30026_2026_SM2")).unwrap();

        let general = instructions(&root, None, None);
        assert!(general.contains("`COMP30026_2026_SM2`"), "the course list is filled in");
        assert!(!general.contains("This conversation"), "no scope section on a general thread");

        let scoped = instructions(&root, Some("COMP30026_2026_SM2"), None);
        assert!(scoped.starts_with(&general), "the scope is appended to the same brief");
        assert!(scoped.contains("`../courses/COMP30026_2026_SM2/`"));
        // The memory bucket it names has to be one the thread can write:
        // `agents/memories/<CODE>/` under the cwd, never the course folder's
        // own, which every sandbox here refuses.
        assert!(scoped.contains("`./memories/COMP30026_2026_SM2/`"));
        assert!(
            !scoped.contains("../courses/COMP30026_2026_SM2/agents/memories"),
            "the unwritable path is not offered as a place to write"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The lecture is a third layer on the same brief, not a replacement for
    /// either of the two under it: a thread opened in the player's dock still
    /// reads the whole library and is still about that lecture's subject. It
    /// names the recording folder as the agent would have to type it — from
    /// `agents/` — and carries the chapters inline, so the first question
    /// does not cost a turn spent reading them back.
    #[test]
    fn a_lecture_thread_keeps_both_briefs_and_names_the_recording() {
        let root = std::env::temp_dir().join("oculus-harness-lecture");
        let _ = std::fs::remove_dir_all(&root);
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

        assert!(full.starts_with(&scoped), "the lecture is appended to the subject's brief");
        assert!(full.contains("`../lectures/abc-123/`"), "the folder as the agent would type it");
        assert!(full.contains("`../lectures/abc-123/transcript.vtt`"));
        assert!(full.contains("00:23:02 — Resolution"), "chapters are inline");
        assert!(full.contains("the moment it was sent at"));
        // The date is the only thing here that says which week's deck goes
        // with the recording — the title is the timetable's.
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
        // The library brief names `lectures/<id>/transcript.vtt` as a shape,
        // so what must be absent is this lecture's own path.
        assert!(
            !no_transcript.contains("`../lectures/abc-123/transcript.vtt`"),
            "a transcript that is not on disk is not named"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
