//! The trait every parser backend implements.

use super::{Health, ParseError, ParseOutput, Progress};
use std::path::Path;

pub trait Parser: Send + Sync {
    /// Parse one PDF. `images_dir` and `images_rel` come from `ImageStaging`,
    /// whose scratch directory's name differs from the prefix on purpose.
    /// `on_progress` may be called from any thread.
    fn parse(
        &self,
        pdf: &Path,
        images_dir: &Path,
        images_rel: &str,
        on_progress: &dyn Fn(Progress),
    ) -> Result<ParseOutput, ParseError>;

    fn health(&self) -> Health;
}
