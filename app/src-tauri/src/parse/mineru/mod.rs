//! MinerU cloud, ported from the Python sidecar.
//!
//! Split the way the Python was: `client` speaks the HTTP protocol, `ledger`
//! holds the daily quota that must survive a restart, `batch` decides which
//! documents travel together, and `render` turns MinerU's content list into
//! the page records the seam defines.

pub mod batch;
pub mod client;
pub mod ledger;
pub mod render;
