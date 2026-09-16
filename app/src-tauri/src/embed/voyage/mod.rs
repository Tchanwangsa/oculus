//! Voyage cloud embedding, split the way `parse/mineru/` is.
//!
//! `client` speaks the HTTP protocol and *is* the `Embedder`; `ledger` holds
//! the allowance, the per-minute throttle and the tier it discovered; `batch`
//! decides which pages travel in one request and how many requests run at once.
//! There is no `render` here — the page images come from `embed/raster.rs`,
//! which is shared with anything else that needs to look at a page.

pub mod batch;
pub mod client;
pub mod ledger;
