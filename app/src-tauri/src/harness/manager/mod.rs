//! The live sessions. [`Harness`] owns one session per thread plus the
//! shared Codex and opencode servers; one file per concern, each an
//! `impl Harness` block or the types it works on.

mod catalogue;
mod handle;
mod instructions;
mod naming;
mod one_off;
mod options;
pub(in crate::harness) mod policy;
mod queue;
mod raw_log;
mod release;
mod session;
mod sinks;
mod stoppable;
mod studio;

pub(in crate::harness) use handle::Handle;
pub use handle::RewindTo;
pub use instructions::{instructions, thread_cwd, thread_sections};
pub use one_off::run_once;
pub(in crate::harness) use one_off::OneOff;
pub(in crate::harness) use options::validate_effort;
pub use options::{LectureBrief, SendOptions, StudioSend};
pub use queue::OwnTurns;
pub use raw_log::RawLog;
pub use release::Release;
pub use sinks::{Generation, OneSink, Sinks};
pub use studio::StudioBrief;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::harness::codex::CodexServer;
use crate::harness::opencode::OpencodeServer;
use crate::harness::Sink;

use catalogue::ClaudeCatalogue;
use handle::Live;

/// The live sessions plus the shared Codex and opencode servers. One per app.
pub struct Harness {
    data_dir: PathBuf,
    #[cfg(test)]
    replay_only: bool,
    /// Recorded sessions a replay's next spawn takes, by thread.
    #[cfg(test)]
    replay_spawns: Mutex<HashMap<i64, ReplaySpawn>>,
    live: Mutex<HashMap<i64, Live>>,
    /// The last [`Generation`] minted.
    generations: AtomicU64,
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
    pub(in crate::harness) suggest: Mutex<crate::harness::suggest::Suggestions>,
    /// One-off turns stoppable by id (`stoppable.rs`).
    stoppable: Mutex<stoppable::Stoppable>,
}

impl Harness {
    pub fn new(data_dir: PathBuf) -> Self {
        Harness {
            data_dir,
            #[cfg(test)]
            replay_only: false,
            #[cfg(test)]
            replay_spawns: Mutex::new(HashMap::new()),
            live: Mutex::new(HashMap::new()),
            generations: AtomicU64::new(0),
            codex: Mutex::new(None),
            opencode: Mutex::new(None),
            codex_account_sink: Mutex::new(None),
            opencode_default_sink: Mutex::new(None),
            claude_models: Mutex::new(None),
            suggest: Mutex::new(crate::harness::suggest::Suggestions::default()),
            stoppable: Mutex::new(stoppable::Stoppable::default()),
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

    /// A number no session of this app has had.
    fn mint(&self) -> Generation {
        self.generations.fetch_add(1, Ordering::SeqCst) + 1
    }
}

/// Opens a recorded session on the sink of the generation minted for it.
#[cfg(test)]
type ReplaySpawn = Box<dyn FnOnce(Sink) -> Handle + Send>;

#[cfg(test)]
impl Harness {
    pub(in crate::harness) fn replay(data_dir: PathBuf) -> Self {
        let mut harness = Self::new(data_dir);
        harness.replay_only = true;
        harness
    }

    /// The session the thread's next spawn opens, in place of a process.
    pub(in crate::harness) fn replay_spawn(
        &self,
        thread: i64,
        spawn: impl FnOnce(Sink) -> Handle + Send + 'static,
    ) {
        self.replay_spawns
            .lock()
            .unwrap()
            .insert(thread, Box::new(spawn));
    }
}
