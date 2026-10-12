//! Word-highlight captions from the narration's word timings. A scene's
//! words become phrases (`phrase`), each on screen for a stretch of the
//! scene (`timing`); at any moment the phrase on screen is drawn as an
//! element tree (`tree`) over the finished frame, the word being said
//! highlighted. Phrases never cross a scene, so a scene's captions depend on
//! its own words only. The same phrases make the WebVTT file
//! (`subtitles`). Styled by the theme's `captions` tokens (`style`).

mod phrase;
mod style;
mod subtitles;
mod timing;
mod tree;

pub use phrase::{Phrase, phrases};
pub use style::{CaptionStyle, Highlight, Placement};
pub use subtitles::{Cue, vtt};
pub use timing::{Moment, moment};

pub(crate) use style::{check, fill_defaults};

use crate::error::Result;
use crate::project::{CuedWord, Project};
use serde_json::Value;

/// One scene's captions: its words and their phrases.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCaptions {
    pub words: Vec<CuedWord>,
    pub phrases: Vec<Phrase>,
}

impl SceneCaptions {
    /// The caption tree at scene time `t` on a `size` frame, or `None` when
    /// no phrase is on screen.
    pub(crate) fn tree_at(&self, style: &CaptionStyle, t: f64, size: (u32, u32)) -> Option<Value> {
        let m = moment(&self.phrases, &self.words, t)?;
        Some(tree::tree(style, &self.phrases, &self.words, m, size))
    }
}

impl Project {
    /// Scene `index`'s captions, phrased for this video's frame and the
    /// theme's `captions` tokens: `None` for a silent scene. Fails like
    /// `cued_words` on stale narration.
    pub fn scene_captions(&self, index: usize) -> Result<Option<SceneCaptions>> {
        let Some(words) = self.cued_words(index)? else {
            return Ok(None);
        };
        let pauses = self.cued_pauses(index)?;
        let style = &self.theme.captions;
        let size = self.video.size;
        let line = style.line_chars(size.width, size.height);
        let phrases = phrases(&words, &pauses, line, style.max_lines);
        Ok(Some(SceneCaptions { words, phrases }))
    }

    /// Every scene's phrases as subtitle cues in video seconds, whether or
    /// not video.json burns captions in. Fails on stale narration.
    pub fn caption_cues(&self) -> Result<Vec<Cue>> {
        let timeline = self.timeline()?;
        let mut cues = Vec::new();
        for i in 0..self.video.scenes.len() {
            let Some(c) = self.scene_captions(i)? else {
                continue;
            };
            let (start, end) = timeline.window(i)?;
            for p in &c.phrases {
                let lines = p
                    .lines
                    .iter()
                    .map(|l| {
                        let words: Vec<&str> =
                            c.words[l.clone()].iter().map(|w| w.text.as_str()).collect();
                        words.join(" ")
                    })
                    .collect();
                cues.push(Cue {
                    start: start + p.show,
                    end: (start + p.hide).min(end),
                    lines,
                });
            }
        }
        Ok(cues)
    }
}

#[cfg(test)]
mod tests;
