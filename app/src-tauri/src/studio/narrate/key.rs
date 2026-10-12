//! A clip's content address.

use sha2::{Digest, Sha256};

use crate::studio::timing::MODEL_ID;
use crate::studio::voice::ServerVoice;

/// Bumped when what goes into a clip changes in a way the inputs below do
/// not show, so every clip is made again.
const SCHEME: &str = "oculus-narration-2";

/// sha256 hex of the section text, the parts the speaker makes it from
/// ([`super::Speaker::key`]) and the aligner model, each length-prefixed so
/// no two different inputs join to the same bytes. The text is always in
/// it: a cached clip's manifest entry carries the text it was timed
/// against. Editing one paragraph changes one scene's key.
pub fn cache_key(text: &str, speaker: &[String]) -> String {
    let mut hash = Sha256::new();
    let parts = [SCHEME, text]
        .into_iter()
        .chain(speaker.iter().map(String::as_str))
        .chain([MODEL_ID]);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

/// A server clip's parts: the engine, the normalised server URL, model,
/// voice and speed.
pub fn server_key(voice: &ServerVoice) -> Vec<String> {
    vec![
        "server".to_owned(),
        voice.base_url.clone(),
        voice.model.clone(),
        voice.voice.clone(),
        format!("{:?}", voice.speed),
    ]
}
