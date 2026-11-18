//! Offline provider-frame replay through production encoders, translators,
//! manager, ordered consumer, migrated SQLite and the frontend projection.

mod agy_recovery;
mod agy_requests;
mod assertions;
mod claude_requests;
mod codex_requests;
mod commands;
mod continuation;
mod driver;
mod fixture;
mod force;
mod guard;
mod handoff_recordings;
mod identity;
mod io;
mod locks;
mod matrix;
pub(in crate::harness) mod outbound;
mod pacing;
mod projection;
mod questions;
mod queue;
mod recovery;
mod reload;
pub(in crate::harness) mod replay;
mod requests;
mod rig;
mod runs;
mod scenario;
mod snapshot;
mod steering;
mod steps;
mod stop;
mod switching;
mod upgrade;
