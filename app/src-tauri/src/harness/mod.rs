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

pub mod app;
pub mod attach;
mod child;
pub mod cli;
pub mod event;
pub mod jobs;
mod manager;
mod protected;
mod providers;
pub mod store;
mod suggest;

pub use cli::{discover, install, signin, update};
pub use event::{HarnessEvent, Provider, ToolKind};
pub use manager::{
    instructions, run_once, thread_cwd, thread_sections, Harness, LectureBrief, Queue,
    QueuedMessage, RawLog, SendOptions,
};
pub use providers::antigravity::rules as antigravity_rules;
pub use providers::{antigravity, claude, codex, opencode};

use std::sync::Arc;

/// Where a bridge hands its events. Called from the bridge's reader thread,
/// in stream order; must not block on the bridge.
pub type Sink = Arc<dyn Fn(HarnessEvent) + Send + Sync>;
