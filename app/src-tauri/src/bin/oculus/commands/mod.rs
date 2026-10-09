//! The `impl Ctx` command bodies, one module per command family. `main.rs`
//! dispatches to them; the clap tree they take their arguments from is in
//! `args/`.

mod auth;
mod docs;
mod lecture;
mod memory;
mod planning;
pub(crate) mod query;
mod run;
mod transcribe;
