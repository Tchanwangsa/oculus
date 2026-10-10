//! Managing the provider CLIs themselves: finding them, installing a
//! missing one, updating an installed one and signing in to it.
//!
//! `harness` re-exports each module, so `harness::discover::binary` and
//! friends keep resolving.

pub mod discover;
pub mod install;
pub mod signin;
pub mod update;
