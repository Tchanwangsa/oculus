//! The window: the last stretch of the transcript, as numbered lines.

use super::TAIL_SECS;
use crate::lectures::chapters::TranscriptCue;

/// One transcript cue in the window, in whole seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Floor of the cue's start: the first column, and what a reply cites.
    pub start: u32,
    /// Floor of the cue's end.
    pub end: u32,
    pub speaker: Option<String>,
    pub text: String,
}

/// The recording's length: the row's duration, or the last cue's end when
/// that is later (or the row has none).
pub fn recording_length(duration: u32, cues: &[(TranscriptCue, Option<String>)]) -> u32 {
    let last = cues
        .iter()
        .map(|(cue, _)| cue.end.max(0.0).floor() as u32)
        .max()
        .unwrap_or(0);
    duration.max(last)
}

/// The cues that start in the last [`TAIL_SECS`] of a `length`-second recording.
pub fn window(cues: &[(TranscriptCue, Option<String>)], length: u32) -> Vec<Line> {
    let from = length.saturating_sub(TAIL_SECS) as f32;
    cues.iter()
        .filter(|(cue, _)| cue.start >= from)
        .map(|(cue, speaker)| Line {
            start: cue.start.max(0.0).floor() as u32,
            end: cue.end.max(0.0).floor() as u32,
            speaker: speaker.clone(),
            text: cue.text.clone(),
        })
        .collect()
}

/// `mm:ss`, or `h:mm:ss` past the hour.
pub fn clock(secs: u32) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// `second  clock  speaker: text` per line. Both columns, because a model
/// converting a clock rounds and then fails validation; the speaker is named
/// only where it changes, since one label per cue doubled the prompt.
pub fn transcript_lines(lines: &[Line]) -> String {
    let one_voice = lines
        .windows(2)
        .all(|pair| pair[0].speaker == pair[1].speaker);
    let mut last: Option<&String> = None;
    lines
        .iter()
        .map(|line| {
            let changed = line.speaker.is_some() && line.speaker.as_ref() != last;
            last = line.speaker.as_ref();
            match &line.speaker {
                Some(who) if changed && !one_voice => {
                    format!(
                        "{}  {}  {who}: {}",
                        line.start,
                        clock(line.start),
                        line.text
                    )
                }
                _ => format!("{}  {}  {}", line.start, clock(line.start), line.text),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
