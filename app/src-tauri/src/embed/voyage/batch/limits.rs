//! How many pages, tokens and requests one document run may have out at once.

use super::{MAX_INPUTS_PER_REQUEST, MAX_TOKENS_PER_REQUEST};

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_inputs: usize,
    /// The API's hard maximum; packing uses its `min` with
    /// `RequestRun::max_tokens`.
    pub max_tokens: u64,
    pub in_flight: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_inputs: MAX_INPUTS_PER_REQUEST,
            max_tokens: MAX_TOKENS_PER_REQUEST,
            // The gate already paces; each request holds its PNGs in memory.
            in_flight: 4,
        }
    }
}
