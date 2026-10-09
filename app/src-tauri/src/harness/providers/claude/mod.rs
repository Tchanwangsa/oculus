//! The Claude Code bridge: one long-lived `claude -p` process per thread.
//!
//! Stream-json both ways: a user turn is one JSON line on stdin; stdout
//! carries stream events for live text, an `assistant` message per block, a
//! `user` message per tool result, and a `result` closing each turn. The
//! process stays up between turns; a thread whose process has gone resumes
//! with `--resume <session id>`.
//!
//! `--permission-prompts none` auto-denies, because under `-p` an unanswered
//! prompt hangs the turn — so containment is settled up front in
//! `settings::settings_json`. See docs/harness.md.

mod models;
mod session;
mod settings;
mod transcript;
mod translate;

use std::path::PathBuf;

use crate::harness::child::ThreadSpawn;

pub use models::{list_models, ModelInfo};
pub use session::ClaudeSession;

pub struct ClaudeSpawn {
    pub base: ThreadSpawn,
    /// The discovered `oculus` binary, allowed by absolute path as well as name.
    pub oculus: Option<PathBuf>,
    /// `default`, `acceptEdits`, `plan`, `bypassPermissions`.
    pub permission_mode: String,
    /// Appended to the CLI's own system prompt.
    pub system_append: String,
    /// A turn outside any thread (`Harness::one_off`): no tools, skills or MCP
    /// servers, and no session kept in the student's `claude --resume` list.
    pub one_off: bool,
}
