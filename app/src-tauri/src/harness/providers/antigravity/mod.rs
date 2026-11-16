//! The Antigravity bridge: one `agy acp` process per thread, speaking the
//! Agent Client Protocol ([`acp`]). Containment is Rust's answer to each
//! permission request; nothing is written to the student's own agy
//! settings. See docs/harness.md, "Antigravity runs over ACP".

pub mod acp;
mod models;
mod skill;
#[cfg(test)]
mod tests;

pub use acp::{AcpSession, AcpSpawn};
pub use models::{list_models, run_models, ModelInfo, ModelsRun};
pub use skill::invoke_skill;
