//! The Codex bridge: JSON-RPC over stdio to `codex app-server`.
//!
//! One app-server process serves every Codex thread; each notification
//! carries a `threadId`. Framing is one JSON object per line: responses echo
//! a numeric `id` with no `method`, notifications have a `method` and no
//! `id`, and a line with both is the server asking *us* something, which must
//! be answered or the turn hangs.
//!
//! Shapes are from `codex app-server` 0.153. `thread/start` takes `sandbox`
//! (a mode string) while `turn/start` takes `sandboxPolicy` (an object), and a
//! resumed thread replays its last turn's token usage first.

mod inbound;
mod server;
#[cfg(test)]
mod tests;
mod translate;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use crate::harness::event::ToolKind;
use crate::harness::{RawLog, Sink};

pub use server::CodexServer;

pub struct CodexSpawn {
    pub bin: PathBuf,
    pub env: Vec<(String, String)>,
    pub raw_log: Option<RawLog>,
    /// Takes the notifications that are about the account rather than a
    /// thread; see `translate_account`.
    pub account_sink: Option<Sink>,
}

/// How to open a thread. `cwd` is also the sandbox's writable root.
pub struct CodexThreadOpts {
    pub cwd: PathBuf,
    /// The database's three files (`paths::db_write_paths`) — files, not
    /// their folder, since the sandbox is Codex's only containment.
    pub writable_files: Vec<PathBuf>,
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Appended to Codex's own instructions (`developerInstructions`).
    pub instructions: String,
    /// Kept out of the student's `codex resume` list: a one-off turn's thread.
    pub ephemeral: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub reasoning_efforts: Vec<String>,
    pub default_reasoning_effort: Option<String>,
    pub is_default: bool,
}

struct ThreadRoute {
    sink: Sink,
    state: Mutex<ThreadState>,
}

#[derive(Default)]
struct ThreadState {
    active_turn: Option<String>,
    /// Set on resume: the server replays the previous turn's usage first.
    ignore_usage_until_turn: bool,
    /// Items already announced, so a completion arriving first can synthesise
    /// the open.
    open_items: HashMap<String, ToolKind>,
    streamed_messages: std::collections::HashSet<String>,
    /// The answer as it streams; an interrupted turn never sends the
    /// `item/completed` that would commit it.
    partial_message: String,
}
