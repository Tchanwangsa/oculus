//! A narrated scene's words as `word()` and `wordEnd()` read them, and its
//! pauses as `pause()` does.

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

/// One `[pause]`'s silence in scene seconds, the narration lead included.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CuedPause {
    /// The index of the word it follows in the scene's words; `None`
    /// before the first.
    pub after: Option<usize>,
    pub start: f64,
    pub end: f64,
}

impl CuedPause {
    /// `clip`'s pauses with the theme's narration `lead` added.
    pub(crate) fn all(clip: &ManifestScene, lead: f64) -> Vec<CuedPause> {
        clip.pauses
            .iter()
            .map(|p| CuedPause {
                after: p.after,
                start: p.start + lead,
                end: p.end + lead,
            })
            .collect()
    }
}

/// One scene's word timings, scene-relative with the narration lead
/// included, ready for a component call. Only a project makes them.
#[derive(Clone, Debug, PartialEq)]
pub struct WordCues {
    /// `{ scene, words: [[text, start, end], ...], pauses: [[start, end],
    /// ...] }`, or `{ unavailable: why }`, parsed by the prelude's
    /// `words.js` on first use.
    json: String,
}

impl WordCues {
    pub(crate) fn new(scene: &str, words: &[CuedWord], pauses: &[CuedPause]) -> WordCues {
        let words: Vec<_> = words
            .iter()
            .map(|w| json!([w.text, w.start, w.end]))
            .collect();
        let pauses: Vec<_> = pauses.iter().map(|p| json!([p.start, p.end])).collect();
        WordCues {
            json: json!({ "scene": scene, "words": words, "pauses": pauses }).to_string(),
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
