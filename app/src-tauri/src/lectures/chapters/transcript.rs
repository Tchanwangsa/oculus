//! The transcript half: pauses between cues, and cues with their voices.

/// The silence before each cue, paired with the second that cue starts at.
/// The timing half of the frontend's `parseVtt` (`app/src/lib/lectures/media/vtt.ts`).
pub fn cue_gaps(vtt: &str) -> Vec<(u32, f32)> {
    let normalised = vtt.replace("\r\n", "\n");
    let mut gaps: Vec<(u32, f32)> = Vec::new();
    let mut previous_end = 0.0f32;

    for block in normalised.split("\n\n") {
        let Some(line) = block.lines().find(|l| l.contains(" --> ")) else {
            continue;
        };
        let mut halves = line.split(" --> ");
        let start = halves
            .next()
            .map(str::trim)
            .and_then(vtt_secs)
            .unwrap_or(-1.0);
        let end = halves
            .next()
            .and_then(|h| h.split_whitespace().next())
            .and_then(vtt_secs)
            .unwrap_or(-1.0);
        if start < 0.0 {
            continue;
        }
        gaps.push((start as u32, (start - previous_end).max(0.0)));
        // A malformed end falls back to the cue's start, not 0.
        previous_end = if end >= start { end } else { start };
    }
    gaps
}

/// Seconds out of a WebVTT timestamp, `HH:MM:SS.mmm` or `MM:SS.mmm`. Shared by
/// [`cue_gaps`] and [`parse_transcript`] so both read the same seconds.
fn vtt_secs(stamp: &str) -> Option<f32> {
    let parts: Vec<&str> = stamp.trim().split(':').collect();
    let number = |s: &str| s.trim().parse::<f32>().ok();
    match parts.len() {
        3 => Some(number(parts[0])? * 3600.0 + number(parts[1])? * 60.0 + number(parts[2])?),
        2 => Some(number(parts[0])? * 60.0 + number(parts[1])?),
        _ => None,
    }
}

/// One WebVTT cue, with tags removed and whitespace folded.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptCue {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// Parse WebVTT timing and text into plain cues. Identifiers and settings are
/// ignored, tags stripped, and a malformed block skipped.
pub fn parse_transcript(vtt: &str) -> Vec<TranscriptCue> {
    parse_transcript_voiced(vtt)
        .into_iter()
        .map(|(cue, _)| cue)
        .collect()
}

/// [`parse_transcript`], each cue paired with its speaker — the annotation of
/// a leading `<v Speaker 0>` voice tag, which the plain text drops.
pub fn parse_transcript_voiced(vtt: &str) -> Vec<(TranscriptCue, Option<String>)> {
    let normalised = vtt.replace("\r\n", "\n");
    normalised
        .split("\n\n")
        .filter_map(|block| {
            let lines: Vec<&str> = block.lines().collect();
            let timing_at = lines.iter().position(|line| line.contains(" --> "))?;
            let mut halves = lines[timing_at].split(" --> ");
            let start = halves.next().map(str::trim).and_then(vtt_secs)?;
            let end = halves
                .next()
                .and_then(|half| half.split_whitespace().next())
                .and_then(vtt_secs)?;
            if start < 0.0 || end < start {
                return None;
            }
            let raw = lines[timing_at + 1..].join(" ");
            let text = plain_text(&raw);
            if text.is_empty() {
                return None;
            }
            Some((TranscriptCue { start, end, text }, voice(&raw)))
        })
        .collect()
}

/// The speaker of a cue's leading voice tag: `<v Speaker 0>` or
/// `<v.loud Speaker 0>` gives `Speaker 0`.
fn voice(raw: &str) -> Option<String> {
    let tag = raw.trim_start().strip_prefix("<v")?;
    if !tag.starts_with([' ', '.']) {
        return None;
    }
    let tag = &tag[..tag.find('>')?];
    let name = tag.split_once(' ').map(|(_, name)| name.trim())?;
    (!name.is_empty()).then(|| name.to_string())
}

fn plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
