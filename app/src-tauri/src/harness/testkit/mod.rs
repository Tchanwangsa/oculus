//! Offline provider-frame replay through production encoders, translators,
//! manager, ordered consumer, migrated SQLite and the frontend projection.

mod assertions;
mod fixture;
mod guard;
mod matrix;
pub(in crate::harness) mod outbound;
mod projection;
pub(in crate::harness) mod replay;
mod runs;
mod scenario;
mod snapshot;
mod upgrade;
