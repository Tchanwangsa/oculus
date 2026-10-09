//! The trait every embedding backend implements.

use std::path::Path;

use super::{EmbedError, EmbedOutput, Health, Progress};

pub trait Embedder: Send + Sync {
    /// Embed every page of one PDF.
    ///
    /// `page_count` comes from the parse record and bounds `EmbedOutput::new`;
    /// a backend may return fewer pages, never more. `on_progress` may be
    /// called from any thread, and in jumps when pages are batched.
    fn embed(
        &self,
        pdf: &Path,
        page_count: u32,
        on_progress: &dyn Fn(Progress),
    ) -> Result<EmbedOutput, EmbedError>;

    /// Embed a search query — the other side of `QUERY_INSTRUCTION`. Returns
    /// floats, since a query vector is never stored, but still normalised so
    /// the dot product is a cosine.
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError>;

    fn health(&self) -> Health;
}
