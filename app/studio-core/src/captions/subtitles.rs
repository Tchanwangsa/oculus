//! The same phrases as subtitle files: one cue per phrase, from its `show`
//! to its `hide` in video seconds, its lines as the cue's lines.

use serde::Serialize;

/// One subtitle cue, in seconds from the start of the video.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    /// The phrase's lines.
    pub lines: Vec<String>,
}

/// WebVTT.
pub fn vtt(cues: &[Cue]) -> String {
    let mut out = String::from("WEBVTT\n");
    for c in cues {
        out.push_str(&format!(
            "\n{} --> {}\n{}\n",
            stamp(c.start, '.'),
            stamp(c.end, '.'),
            c.lines.join("\n")
        ));
    }
    out
}

/// SubRip, numbered from 1.
pub fn srt(cues: &[Cue]) -> String {
    let mut out = String::new();
    for (i, c) in cues.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n",
            i + 1,
            stamp(c.start, ','),
            stamp(c.end, ','),
            c.lines.join("\n")
        ));
    }
    out
}

/// `hh:mm:ss.mmm`, the milliseconds after `sep`.
fn stamp(secs: f64, sep: char) -> String {
    let ms = (secs.max(0.0) * 1000.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{sep}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}
