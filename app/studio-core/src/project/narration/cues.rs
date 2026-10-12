//! A narrated scene's words as `word()` and `wordEnd()` read them.

use super::ManifestScene;
use serde::Serialize;
use serde_json::json;

/// One narrated word at the times `word()` and `wordEnd()` return for it:
/// seconds from the scene's start, the narration lead included.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CuedWord {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

impl CuedWord {
    /// `clip`'s words with the theme's narration `lead` added.
    pub(crate) fn all(clip: &ManifestScene, lead: f64) -> Vec<CuedWord> {
        clip.words
            .iter()
            .map(|w| CuedWord {
                text: w.text.clone(),
                start: w.start + lead,
                end: w.end + lead,
            })
            .collect()
    }
}

/// One scene's word timings, scene-relative with the narration lead
/// included, ready for a component call. Only a project makes them.
#[derive(Clone, Debug, PartialEq)]
pub struct WordCues {
    /// `{ scene, words: [[text, start, end], ...] }`, or `{ unavailable:
    /// why }`, parsed by the prelude's `words.js` on first use.
    json: String,
}

impl WordCues {
    pub(crate) fn new(scene: &str, words: &[CuedWord]) -> WordCues {
        let words: Vec<_> = words
            .iter()
            .map(|w| json!([w.text, w.start, w.end]))
            .collect();
        WordCues {
            json: json!({ "scene": scene, "words": words }).to_string(),
        }
    }

    /// No words, for a call that has no narration to read (a gallery):
    /// `word()` and `wordEnd()` throw `why` after the call.
    pub(crate) fn unavailable(why: &str) -> WordCues {
        WordCues {
            json: json!({ "unavailable": why }).to_string(),
        }
    }

    pub(crate) fn json(&self) -> &str {
        &self.json
    }
}
