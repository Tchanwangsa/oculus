//! A narrated scene's words as `word()` and `wordEnd()` read them.

use super::ManifestScene;
use serde_json::json;

/// One scene's word timings, scene-relative with the narration lead
/// included, ready for a component call. Only a project makes them.
#[derive(Clone, Debug, PartialEq)]
pub struct WordCues {
    /// `{ scene, words: [[text, start, end], ...] }`, parsed by the prelude's
    /// `words.js` on first use.
    json: String,
}

impl WordCues {
    pub(crate) fn new(scene: &str, clip: &ManifestScene, lead: f64) -> WordCues {
        let words: Vec<_> = clip
            .words
            .iter()
            .map(|w| json!([w.text, w.start + lead, w.end + lead]))
            .collect();
        WordCues {
            json: json!({ "scene": scene, "words": words }).to_string(),
        }
    }

    pub(crate) fn json(&self) -> &str {
        &self.json
    }
}
