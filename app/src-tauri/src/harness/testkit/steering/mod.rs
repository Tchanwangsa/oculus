//! Steering the running turn (docs/harness.md, "Steering the running
//! turn"): against a fake bridge on the scripted bus (`fake.rs`,
//! `restart.rs`), and through the recorded Codex and Claude probes
//! (`codex.rs`, `claude.rs`) and ACP cancellations (`antigravity.rs`).

pub(super) mod antigravity;
mod bench;
mod claude;
mod codex;
mod fake;
pub(super) mod recorded;
mod restart;
