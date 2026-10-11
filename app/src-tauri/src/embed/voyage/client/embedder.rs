//! `VoyageCloud` as a `RequestRun` (one request) and as the `Embedder` the app indexes through.

use super::send::{Cost, SendFailure};
use super::wire::image_inputs;
use super::{VoyageCloud, BACKEND};
use crate::embed::raster;
use crate::embed::raster::RenderedPage;
use crate::embed::voyage::batch;
use crate::embed::voyage::batch::RequestRun;
use crate::embed::{
    pack_vector, unpack_vector, EmbedError, EmbedOutput, Embedder, Health, Progress, Wait,
    EMBED_DIM, EMBED_MODEL,
};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;

/// The wire side of `embed::QUERY_INSTRUCTION`.
const INPUT_TYPE_DOCUMENT: &str = "document";
const INPUT_TYPE_QUERY: &str = "query";

impl RequestRun for VoyageCloud {
    /// The largest request this account can get accepted: the API maximum or
    /// the tier's TPM, whichever is smaller. A request over TPM draws a 429 no
    /// pacing clears.
    fn max_tokens(&self) -> u64 {
        let tier = self.gate.tier().tpm.max(1.0) as u64;
        batch::MAX_TOKENS_PER_REQUEST.min(tier)
    }

    /// Embed a group of pages in as many requests as the current ceiling
    /// allows. Usually one; the loop covers a first request packed before a 429
    /// taught the tier.
    fn run(
        &self,
        pages: &[RenderedPage],
        on_wait: &dyn Fn(Option<Wait>),
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let costs: Vec<u64> = pages
            .iter()
            .map(|page| batch::tokens_for(page.width, page.height))
            .collect();
        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(pages.len());
        let mut offset = 0;

        while offset < pages.len() {
            // Re-read: the previous chunk's 429 may have shrunk it.
            let ceiling = self.max_tokens();
            let mut end = offset;
            let mut spent = 0u64;
            while end < pages.len() {
                let over = end > offset
                    && (end - offset >= batch::MAX_INPUTS_PER_REQUEST
                        || spent + costs[end] > ceiling);
                if over {
                    break;
                }
                spent += costs[end];
                end += 1;
            }

            let chunk = &pages[offset..end];
            let cost = Cost {
                tokens: spent,
                pixels: chunk.iter().map(batch::billed_pixels).sum(),
            };
            match self.send(
                image_inputs(chunk),
                INPUT_TYPE_DOCUMENT,
                chunk.len(),
                cost,
                on_wait,
            ) {
                Ok(part) => {
                    vectors.extend(part);
                    offset = end;
                }
                Err(SendFailure::Resize) => continue,
                Err(SendFailure::Embed(error)) => return Err(error),
            }
        }
        Ok(vectors)
    }
}

impl Embedder for VoyageCloud {
    fn embed(
        &self,
        pdf: &Path,
        page_count: u32,
        on_progress: &dyn Fn(Progress),
    ) -> Result<EmbedOutput, EmbedError> {
        // The renderer counts pages as the parse did, so they disagree only
        // when the file changed after its parse. `page_no` is the join key, so
        // refuse before a pixel is billed rather than file vectors under wrong
        // pages.
        let theirs = raster::page_count(pdf)?;
        if page_count > 0 && theirs != page_count {
            return Err(EmbedError::Document {
                code: "page-count-mismatch".into(),
            });
        }
        let expected = if page_count > 0 { page_count } else { theirs };

        // Before the work, so a run that cannot fit does not half-embed.
        self.ledger.ensure_available(0)?;

        let runner: Arc<dyn RequestRun> = Arc::new(self.clone());
        let pages = batch::run_document(pdf, expected, runner, self.limits, on_progress)?;

        // Never a partial record: `EmbedOutput::new` drops out-of-range pages,
        // so drifted page numbers would otherwise leave one short.
        let output = EmbedOutput::new(pdf, expected, pages);
        if output.page_count as u32 != expected {
            return Err(EmbedError::Document {
                code: "incomplete".into(),
            });
        }
        Ok(output)
    }

    fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(EmbedError::Document {
                code: "empty-query".into(),
            });
        }
        // ~4 characters a token, only to pace and reserve; the response's
        // `usage.total_tokens` settles it.
        let cost = Cost {
            tokens: (text.len() as u64 / 4) + 1,
            pixels: 0,
        };
        let inputs = json!([{ "content": [{ "type": "text", "text": text }] }]);
        let vectors = self
            .send(inputs, INPUT_TYPE_QUERY, 1, cost, &|_| {})
            .map_err(|failure| {
                match failure {
                    SendFailure::Embed(error) => error,
                    // Unreachable: `send` only resizes multi-input requests.
                    SendFailure::Resize => EmbedError::RateLimited {
                        retry_after_secs: None,
                    },
                }
            })?;
        let raw = vectors.into_iter().next().ok_or(EmbedError::Document {
            code: "embedding-missing".into(),
        })?;
        // Normalised by the same gate as stored pages.
        Ok(unpack_vector(&pack_vector(&raw)?))
    }

    /// Ready means a key, which `new` guarantees; the renderer is compiled
    /// in. Quota is not readiness; it surfaces as `QuotaExhausted`.
    fn health(&self) -> Health {
        Health {
            backend: BACKEND.to_string(),
            model: EMBED_MODEL.to_string(),
            dim: EMBED_DIM,
            ready: true,
        }
    }
}
