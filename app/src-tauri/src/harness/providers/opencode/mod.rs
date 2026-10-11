//! The opencode bridge: HTTP and SSE against one `opencode serve` per app.
//!
//! One server (started on first use, killed on quit), one session per thread,
//! and one `GET /event` stream whose reader routes each event by session id.
//! Everything is opencode's v1 session API, scoped with `?directory=<agents>`
//! because an unscoped call binds to the server's own cwd. Not v2 (`/api/*`):
//! on 1.18.31 a v2 prompt on an `auth.json` provider fails inside the server
//! and emits nothing. The stream's rules are on [`events::translate`].
//!
//! opencode has no OS sandbox. Containment is its permission ruleset
//! (`templates/OPENCODE.template.json`, rule shapes in docs/harness.md), and
//! `bash` is a glob over the command string — a speed bump, not a boundary.
//! An agent's `prompt` replaces the whole system prompt, so the rendered
//! harness brief is the `oculus` agent's prompt and the per-thread part rides
//! the session's first message.

mod auth;
mod config;
mod events;
mod http;
mod models;
mod redact;
mod server;
mod session;
mod strays;
mod stream;
#[cfg(test)]
mod tests;
mod types;

pub use auth::visible_answers;
pub use config::{write_config, OneOffPrompts};
pub use models::split_model;
pub use server::OpencodeServer;
pub use strays::sweep;
pub use types::{
    AuthMethod, AuthOption, AuthPrompt, AuthWhen, Authorization, ModelCost, ModelFacts, ModelInfo,
    OpencodeSessionOpts, OpencodeSpawn, ProviderInfo, ProviderList,
};

/// The agent id the sessions run as, defined in the rendered config.
pub const AGENT: &str = "oculus";
/// The hidden agents the one-off turns run as (`Harness::one_off`): no tools,
/// and the turn's brief as the whole prompt.
pub const NAMING_AGENT: &str = "oculus-namer";
pub const WRITER_AGENT: &str = "oculus-writer";
pub const LECTURE_END_AGENT: &str = "oculus-lecture-end";

pub const CONFIG_NAME: &str = "opencode.json";
