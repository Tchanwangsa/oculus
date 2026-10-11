//! The Antigravity bridge: one long-lived `agy --print=` process per thread,
//! stream-json both ways. Claude's shape with a different vocabulary:
//! `init` / `step_update` / `result` events, `--conversation` for `--resume`.
//! No inline settings, no rewind and no protocol-level interrupt; containment
//! is rules in agy's global settings file ([`rules`]) plus
//! `--sandbox`, which bounds shell commands only. See docs/harness.md.
//!
//! Two quirks of agy 1.2.9 that its reference does not show: `-p` takes the
//! prompt as its value, so stream-json mode needs `--print=` with an empty
//! *attached* value; and tool parameters are PascalCase (`CommandLine`,
//! `AbsolutePath`). `fixtures/harness/antigravity-ls.ndjson` is a real session.

mod install;
mod json;
mod models;
pub mod rules;
mod session;
#[cfg(test)]
mod tests;
mod translate;

use crate::harness::child::ThreadSpawn;

pub use models::{list_models, run_models, ModelInfo, ModelsRun};
pub use session::AntigravitySession;

pub struct AntigravitySpawn {
    /// `base.cwd` is also where `agy` reads `AGENTS.md` from; `base.effort`
    /// is folded into `--model` by [`models::model_slug`].
    pub base: ThreadSpawn,
    /// The per-thread half of the brief, riding the first user message (no
    /// system-prompt flag; `agy` reads `AGENTS.md` itself).
    pub brief: String,
    /// Approved rules; `None` reuses the last written. See
    /// [`install::install`].
    pub approved: Option<Vec<String>>,
}
