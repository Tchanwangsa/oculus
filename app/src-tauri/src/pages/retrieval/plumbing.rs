use base64::Engine;

use crate::embed;

/// The space this app searches in, off the seam's constants: `stats` must
/// answer with no API key stored. `embed::Health::check` refuses any backend
/// that disagrees, so the two agree by construction.
pub(super) fn current_space() -> (&'static str, i64) {
    (embed::EMBED_MODEL, embed::EMBED_DIM as i64)
}

/// The stored blob *is* the wire string, base64-decoded. Not through floats,
/// which would re-normalise and could move the last bit.
pub(super) fn blob_from_wire(page_no: u32, encoded: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("page {page_no}: bad base64: {e}"))
}
