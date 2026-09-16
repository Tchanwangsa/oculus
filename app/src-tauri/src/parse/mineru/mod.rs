//! MinerU, in its two shapes: the cloud service and a server on this machine.
//!
//! The cloud half is split the way the Python was: `client` speaks the HTTP
//! protocol, `ledger` holds the daily quota that must survive a restart,
//! `batch` decides which documents travel together. `local` is the whole of
//! the other half — a single blocking POST, because that is all MinerU's own
//! server offers — and it is tiny precisely because `render` is shared: the
//! content list is the same on both sides, so the module that decides what the
//! markdown says belongs to neither backend.

pub mod batch;
pub mod client;
pub mod ledger;
pub mod local;
pub mod render;
