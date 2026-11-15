//! Context handoff: the earlier conversation a run's first prompt carries
//! to a session that has not seen it, because the student changed provider
//! mid-chat or the session had to start over. `budget` says how much fits,
//! `history` chooses and renders it, and `store` plans, saves and settles
//! one per run. docs/harness.md, "Changing provider mid-chat".

pub(in crate::harness) mod budget;
pub(in crate::harness) mod history;
