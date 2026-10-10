//! The live sessions. [`Harness`] owns one session per thread plus the
//! shared Codex and opencode servers; one file per concern, each an
//! `impl Harness` block or the types it works on.

mod catalogue;
mod handle;
mod instructions;
mod naming;
mod one_off;
mod options;
mod queue;
mod raw_log;
mod session;

pub(in crate::harness) use handle::Handle;
pub use instructions::{instructions, thread_cwd, thread_sections};
pub use one_off::run_once;
pub(in crate::harness) use one_off::OneOff;
pub(in crate::harness) use options::validate_effort;
pub use options::{LectureBrief, SendOptions};
pub use queue::{Queue, QueuedMessage};
pub use raw_log::RawLog;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::harness::codex::CodexServer;
use crate::harness::opencode::OpencodeServer;
use crate::harness::Sink;

use catalogue::ClaudeCatalogue;
use handle::Live;

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
    pub(in crate::harness) suggest: Mutex<crate::harness::suggest::Suggestions>,
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
            suggest: Mutex::new(crate::harness::suggest::Suggestions::default()),
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
}
