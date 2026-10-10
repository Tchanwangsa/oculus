//! The embedded document: one vector per page, exactly the `.emb.json`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{
    emb_path, encode_vector, EmbedError, EMBED_DIM, EMBED_DTYPE, EMBED_MODEL, QUERY_INSTRUCTION,
};

/// One page's vector, keyed by its 1-based page number — the join key that
/// resolves a hit to markdown in `.pages.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedPage {
    pub page_no: u32,
    /// base64 of little-endian f16, `EMBED_DIM` wide, kept as the wire string
    /// so a record round-trips byte-for-byte.
    pub vector: String,
}

impl EmbedPage {
    /// Encode one page's vector, normalising it on the way in.
    pub fn new(page_no: u32, vector: &[f32]) -> Result<Self, EmbedError> {
        Ok(Self {
            page_no,
            vector: encode_vector(vector)?,
        })
    }
}

/// Exactly the `.emb.json` on disk; field names are the wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedOutput {
    /// The PDF's file name, not its path — the record travels with the folder.
    pub pdf: String,
    pub model: String,
    pub dim: usize,
    pub dtype: String,
    pub instruction: String,
    /// How many pages were embedded — **not** the document length, unlike
    /// `.pages.json`'s `page_count`, which `is_embedded` checks coverage against.
    pub page_count: usize,
    pub pages: Vec<EmbedPage>,
}

impl EmbedOutput {
    /// Identity and coverage checked against the parse record already read by
    /// the caller, so ingest does not decode either artifact twice.
    pub(crate) fn is_current(&self, expected_pages: Option<u32>) -> bool {
        self.model == EMBED_MODEL
            && self.dim == EMBED_DIM
            && self.instruction == QUERY_INSTRUCTION
            && expected_pages.is_none_or(|count| self.pages.len() >= count as usize)
    }

    /// Pages in ascending order, one per page number, none outside
    /// `1..=page_count`. Missing pages are dropped, not filled: unlike an empty
    /// markdown string there is no empty vector, and a zero one scores 0
    /// against every query while looking indexed.
    pub fn new(pdf: &Path, page_count: u32, pages: Vec<EmbedPage>) -> Self {
        let mut kept: Vec<EmbedPage> = Vec::with_capacity(pages.len());
        for page in pages {
            if page.page_no >= 1
                && page.page_no <= page_count
                && !kept.iter().any(|existing| existing.page_no == page.page_no)
            {
                kept.push(page);
            }
        }
        kept.sort_by_key(|page| page.page_no);
        Self {
            pdf: pdf
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            model: EMBED_MODEL.to_string(),
            dim: EMBED_DIM,
            dtype: EMBED_DTYPE.to_string(),
            instruction: QUERY_INSTRUCTION.to_string(),
            page_count: kept.len(),
            pages: kept,
        }
    }

    /// Put the record on disk atomically (temp file, fsync, rename): its
    /// existence is the only evidence the embedding finished, and a torn write
    /// could otherwise read as a complete record for a partly-indexed file.
    pub fn write(&self, pdf: &Path) -> Result<(), EmbedError> {
        let final_path = emb_path(pdf);
        let body = serde_json::to_vec(self)
            .map_err(|e| EmbedError::Io(format!("encode {}: {e}", final_path.display())))?;
        let tmp = final_path.with_extension(format!("json.tmp{}", std::process::id()));
        crate::runtime::atomic_write::write(&final_path, &tmp, &body).map_err(EmbedError::Io)
    }
}
