//! Voyage cloud embedding. `client` speaks HTTP and is the `Embedder`;
//! `ledger` holds the allowance, the throttle and the learned tier; `batch`
//! packs pages into requests. Page images come from `embed/raster/`.

pub mod batch;
pub mod client;
pub mod ledger;
