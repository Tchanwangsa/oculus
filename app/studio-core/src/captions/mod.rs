//! Captions from the narration's word timings, as subtitle files: a scene's
//! words become phrases (`phrase`), each on screen for a stretch of the
//! scene (`timing`), and the phrases in video time become WebVTT or SubRip
//! cues (`subtitles`). Phrases never cross a scene.

mod phrase;
mod subtitles;
mod timing;

pub use phrase::{Phrase, phrases};
pub use subtitles::{Cue, srt, vtt};

use crate::error::Result;
use crate::project::Project;

/// Characters a caption line holds, the usual subtitle limit.
pub const LINE_CHARS: usize = 42;
/// Lines a caption holds.
pub const MAX_LINES: usize = 2;

impl Project {
    /// Every narrated scene's phrases as subtitle cues in video seconds.
    /// Fails on stale narration.
    pub fn caption_cues(&self) -> Result<Vec<Cue>> {
        let timeline = self.timeline()?;
        let mut cues = Vec::new();
        for i in 0..self.video.scenes.len() {
            let Some(words) = self.cued_words(i)? else {
                continue;
            };
            let pauses = self.cued_pauses(i)?;
            let (start, end) = timeline.window(i)?;
            for p in phrases(&words, &pauses, LINE_CHARS, MAX_LINES) {
                let lines = p
                    .lines
                    .iter()
                    .map(|l| {
                        let words: Vec<&str> =
                            words[l.clone()].iter().map(|w| w.text.as_str()).collect();
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
