//! A clip's content address.

use sha2::{Digest, Sha256};

use crate::studio::timing::MODEL_ID;
use crate::studio::voice::Voice;

/// Bumped when what goes into a clip changes in a way the inputs below do
/// not show, so every clip is made again.
const SCHEME: &str = "oculus-narration-1";

/// sha256 hex of the section text, the normalised server URL, model, voice,
/// speed and aligner model, each length-prefixed so no two different inputs
/// join to the same bytes. Editing one paragraph changes one scene's key.
pub fn cache_key(text: &str, voice: &Voice) -> String {
    let speed = format!("{:?}", voice.speed);
    let mut hash = Sha256::new();
    for part in [
        SCHEME,
        text,
        &voice.base_url,
        &voice.model,
        &voice.voice,
        &speed,
        MODEL_ID,
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}
