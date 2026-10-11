//! `MinerUCloud` as a `BatchRun` and as the `Parser` the app parses through.

use super::document::{CloudDocument, DocumentOutput};
use super::MinerUCloud;
use super::BACKEND;
use crate::parse::mineru::batch::{BatchRun, Batcher};
use crate::parse::mineru::ledger::MAX_FILE_BYTES;
use crate::parse::{check_size, Health, ParseError, ParseOutput, Parser, Progress, PARSER_VERSION};
use std::path::Path;
use std::sync::Arc;

impl BatchRun for MinerUCloud {
    fn run(&self, documents: &[Arc<CloudDocument>]) -> Vec<Result<DocumentOutput, ParseError>> {
        self.extract_documents(documents)
    }
}

impl Parser for MinerUCloud {
    fn parse(
        &self,
        pdf: &Path,
        images_dir: &Path,
        images_rel: &str,
        on_progress: &dyn Fn(Progress),
    ) -> Result<ParseOutput, ParseError> {
        // Before the batch, so an oversized file never takes a seat.
        check_size(pdf, MAX_FILE_BYTES)?;

        let document = CloudDocument::new(pdf, images_dir, images_rel);
        document.count_pages()?;
        let runner: Arc<dyn BatchRun> = Arc::new(self.clone());
        let output = Batcher::shared().submit(self.batch_key(), document, runner, on_progress)?;
        Ok(ParseOutput::new(
            pdf,
            output.total_pages,
            output.pages,
            Some(BACKEND.to_string()),
            output.image_count,
        ))
    }

    /// Ready whenever it has a token, which `new` guarantees. Quota is not
    /// readiness: it surfaces as `QuotaExhausted` on the call that hits it.
    fn health(&self) -> Health {
        Health {
            backend: BACKEND.to_string(),
            parser_version: PARSER_VERSION,
            ready: true,
        }
    }
}
