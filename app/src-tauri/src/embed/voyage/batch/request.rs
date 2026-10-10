//! What embeds one request.

use super::MAX_TOKENS_PER_REQUEST;
use crate::embed::raster::RenderedPage;
use crate::embed::{EmbedError, Wait};

/// What embeds a request; a trait so packing and concurrency test without a
/// server.
pub trait RequestRun: Send + Sync {
    /// One vector per page, in the order the pages were given. `on_wait`
    /// hears of a rate-limit wait as it starts, and `None` when it ends.
    fn run(
        &self,
        pages: &[RenderedPage],
        on_wait: &dyn Fn(Option<Wait>),
    ) -> Result<Vec<Vec<f32>>, EmbedError>;

    /// The largest request this backend can currently get accepted. A request
    /// over the account's TPM is refused whatever the pace, so this is re-read
    /// on every page and a run shrinks its requests once a 429 teaches the tier.
    fn max_tokens(&self) -> u64 {
        MAX_TOKENS_PER_REQUEST
    }
}
