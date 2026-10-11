//! The on-disk record beside a PDF and the encoding of its vectors.
//!
//! One file beside the PDF, named off its stem — the same rule the parse
//! artifacts follow, so a library folder can be copied whole. Voyage has no f16
//! `output_dtype` (its base64 `output_encoding` is f32), so every backend hands
//! over `&[f32]` and the narrowing to f16 happens here, once.

use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use half::f16;

use super::{EmbedError, EmbedOutput, EMBED_DIM, EMBED_MODEL};

pub fn emb_path(pdf: &Path) -> PathBuf {
    pdf.with_extension("emb.json")
}

/// The record beside this PDF. Records from another model still deserialise
/// (so a library scan never blows up on one) but `is_embedded` rejects them.
pub fn read_record(pdf: &Path) -> Option<EmbedOutput> {
    serde_json::from_str(&fs::read_to_string(emb_path(pdf)).ok()?).ok()
}

/// True when this PDF's vectors are in this app's space and cover every page.
///
/// Model, dim and instruction must all match — a vector from another model is
/// a different geometry, not older output. Coverage matters too: a cloud run
/// can lose a page to a rate limit or a refusal, and `EmbedOutput::new` drops
/// it rather than inventing a vector, so without the count check that page
/// would never be searchable or retried. The expected count comes from the
/// parse record; with none, identity alone has to do.
pub fn is_embedded(pdf: &Path) -> bool {
    let Some(record) = read_record(pdf) else {
        return false;
    };
    record.is_current(crate::parse::read_record(pdf).map(|parsed| parsed.page_count))
}
///
/// A Matryoshka prefix is a valid embedding, so a longer vector is truncated;
/// a shorter one is a different space and is refused, not padded. Truncation
/// breaks unit length and Voyage's own norm is only ≈1, so this re-normalises:
/// `pages/retrieval/` ranks by a raw dot product, and an unnormalised vector would
/// silently let long pages win.
pub fn encode_vector(vector: &[f32]) -> Result<String, EmbedError> {
    let bytes = pack_vector(vector)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// The raw bytes that go into `pages.embedding` — exactly what
/// `encode_vector` base64s, so the blob and the record never disagree.
pub fn pack_vector(vector: &[f32]) -> Result<Vec<u8>, EmbedError> {
    if vector.len() < EMBED_DIM {
        return Err(EmbedError::ModelMismatch {
            app_model: EMBED_MODEL.to_string(),
            app_dim: EMBED_DIM,
            backend_model: EMBED_MODEL.to_string(),
            backend_dim: vector.len(),
        });
    }
    let head = &vector[..EMBED_DIM];
    let norm = head
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    // A zero vector cannot be normalised and would rank 0 against every query.
    if !norm.is_finite() || norm <= 0.0 {
        return Err(EmbedError::Document {
            code: "zero_vector".into(),
        });
    }
    let mut bytes = Vec::with_capacity(EMBED_DIM * 2);
    for value in head {
        bytes.extend_from_slice(&f16::from_f32((*value as f64 / norm) as f32).to_le_bytes());
    }
    Ok(bytes)
}

/// base64 f16 -> f32, the read side of `encode_vector`; tests use it to check
/// the round trip.
#[cfg(test)]
pub fn decode_vector(encoded: &str) -> Result<Vec<f32>, EmbedError> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| EmbedError::Document {
            code: format!("bad base64: {e}"),
        })?;
    Ok(unpack_vector(&raw))
}

/// float16 little-endian -> f32. Vectors are stored normalised, so a dot
/// product is cosine similarity.
pub fn unpack_vector(raw: &[u8]) -> Vec<f32> {
    raw.chunks_exact(2)
        .map(|c| f16::from_le_bytes([c[0], c[1]]).to_f32())
        .collect()
}
