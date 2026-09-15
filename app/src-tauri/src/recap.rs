//! Lecture recap: one short note for each meaningful visual moment.
//!
//! Recap shares chapters' cheap visual detector and splash-resistant frame
//! grabs, but deliberately keeps a denser boundary set. The recording is
//! then split into roughly ten-minute windows so a long lecture never becomes
//! one enormous agent turn. Each window is parsed, validated and written on
//! its own; a failure therefore leaves the completed windows visible.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Recap follows slide changes closely enough that looking away for half a
/// minute normally moves to at most one new note.
const MIN_SEGMENT_SECS: u32 = 25;

/// A lecturer can speak over one unchanged slide for a long time. Past this
/// length the longest transcript pause becomes an extra segment boundary.
const MAX_SEGMENT_SECS: u32 = 3 * 60;

/// One agent turn should carry about this much lecture.
const WINDOW_TARGET_SECS: u32 = 10 * 60;

/// A nearby chapter boundary is a better window edge than an arbitrary slide
/// change, but a far-away one should not make a tiny or enormous turn.
const WINDOW_SNAP_SECS: u32 = 3 * 60;

/// One WebVTT cue, with tags removed and whitespace folded for prompt use.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptCue {
    pub start: f32,
    pub end: f32,
    pub text: String,
}

/// One stored recap row. Its end is the next row's start (or the lecture's
/// duration), so storing an end would duplicate a fact just as it would for a
/// chapter.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct RecapNote {
    pub start_seconds: u32,
    pub label: String,
    pub body: String,
}

#[derive(serde::Deserialize)]
struct ReplyNote {
    #[serde(alias = "start_seconds", alias = "seconds", alias = "at")]
    start: f64,
    #[serde(default)]
    label: String,
    body: String,
}

#[derive(serde::Deserialize)]
struct ReplyEnvelope {
    #[serde(alias = "recap")]
    notes: Vec<ReplyNote>,
}

/// A job-time window. Windows are intentionally not persisted: they are only
/// a way to keep each agent turn bounded.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub start_seconds: u32,
    pub end_seconds: u32,
    pub segment_starts: Vec<u32>,
    pub chapter_title: Option<String>,
}

// ── Transcript and segmentation ─────────────────────────────────────────────

/// Parse WebVTT timing and text into plain cues suitable for an inline prompt.
///
/// The timing shapes agree with the player's parser and `chapters::cue_gaps`:
/// both `HH:MM:SS.mmm` and `MM:SS.mmm` are accepted. Cue identifiers and VTT
/// settings are ignored, simple WebVTT tags are stripped, and malformed blocks
/// are skipped rather than poisoning the rest of the transcript.
pub fn parse_transcript(vtt: &str) -> Vec<TranscriptCue> {
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
            let text = plain_text(&lines[timing_at + 1..].join(" "));
            if text.is_empty() {
                return None;
            }
            Some(TranscriptCue { start, end, text })
        })
        .collect()
}

fn vtt_secs(stamp: &str) -> Option<f32> {
    let parts: Vec<&str> = stamp.trim().split(':').collect();
    let number = |s: &str| s.trim().parse::<f32>().ok();
    match parts.len() {
        3 => Some(number(parts[0])? * 3600.0 + number(parts[1])? * 60.0 + number(parts[2])?),
        2 => Some(number(parts[0])? * 60.0 + number(parts[1])?),
        _ => None,
    }
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

/// Dense, time-ordered recap segment starts, always beginning at second 0.
///
/// Visual candidates use chapters' measured detector with a 25-second
/// thinning radius. Short leading/trailing fragments are merged away, then a
/// span over three minutes is recursively split at its longest usable
/// transcript pause. When a malformed transcript offers no cue inside a long
/// span, the midpoint is the only honest fallback that still enforces the
/// ceiling.
pub fn segment_starts(
    diffs: &[(u32, f32)],
    gaps: &[(u32, f32)],
    cues: &[TranscriptCue],
    duration_secs: u32,
) -> Vec<u32> {
    if duration_secs == 0 {
        return vec![0];
    }
    let found = crate::chapters::candidates_with_spacing(
        diffs,
        gaps,
        duration_secs,
        MIN_SEGMENT_SECS,
    );
    let mut starts = vec![0];
    starts.extend(found.into_iter().map(|candidate| candidate.seconds));
    starts.sort_unstable();
    starts.dedup();
    merge_short_edges(&mut starts, duration_secs);

    let original = starts.clone();
    for (idx, &start) in original.iter().enumerate() {
        let end = original.get(idx + 1).copied().unwrap_or(duration_secs);
        starts.extend(split_points(start, end, cues));
    }
    starts.sort_unstable();
    starts.dedup();
    starts
}

fn merge_short_edges(starts: &mut Vec<u32>, duration: u32) {
    while starts.len() > 1 && starts[1] - starts[0] < MIN_SEGMENT_SECS {
        starts.remove(1);
    }
    while starts.len() > 1 && duration.saturating_sub(*starts.last().unwrap()) < MIN_SEGMENT_SECS {
        starts.pop();
    }
}

fn split_points(start: u32, end: u32, cues: &[TranscriptCue]) -> Vec<u32> {
    if end.saturating_sub(start) <= MAX_SEGMENT_SECS {
        return Vec::new();
    }
    let low = start + MIN_SEGMENT_SECS;
    let high = end.saturating_sub(MIN_SEGMENT_SECS);
    let midpoint = start + (end - start) / 2;
    let split = cues
        .iter()
        .filter_map(|cue| {
            let at = cue.start.max(0.0).round() as u32;
            if at < low || at > high {
                return None;
            }
            let gap = cue.start - previous_cue_end(cues, cue.start);
            Some((at, gap.max(0.0), at.abs_diff(midpoint)))
        })
        .max_by(|a, b| {
            a.1.partial_cmp(&b.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.2.cmp(&a.2))
        })
        .map(|(at, _, _)| at)
        .unwrap_or(midpoint.clamp(low, high));

    let mut out = split_points(start, split, cues);
    out.push(split);
    out.extend(split_points(split, end, cues));
    out
}

fn previous_cue_end(cues: &[TranscriptCue], start: f32) -> f32 {
    cues.iter()
        .filter(|cue| cue.end <= start)
        .map(|cue| cue.end)
        .fold(0.0, f32::max)
}

// ── Windows and prompt ──────────────────────────────────────────────────────
/// Chunk segments into approximately ten-minute windows. A chapter boundary
/// within three minutes of the target wins; it is mapped to the nearest
/// segment start so the first note in every window remains a valid segment.
pub fn windows(
    starts: &[u32],
    duration_secs: u32,
    chapters: &[crate::chapters::Chapter],
) -> Vec<Window> {
    if starts.is_empty() || duration_secs == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut first = 0usize;
    while first < starts.len() {
        let start = starts[first];
        let target = start.saturating_add(WINDOW_TARGET_SECS);
        let next = if target >= duration_secs {
            starts.len()
        } else {
            let ordinary = nearest_later_start(starts, first, target);
            let snapped = chapters
                .iter()
                .filter(|chapter| {
                    chapter.start_seconds > start && chapter.start_seconds < duration_secs
                })
                .min_by_key(|chapter| chapter.start_seconds.abs_diff(target))
                .filter(|chapter| chapter.start_seconds.abs_diff(target) <= WINDOW_SNAP_SECS)
                .map(|chapter| nearest_later_start(starts, first, chapter.start_seconds));
            snapped.unwrap_or(ordinary)
        };
        let next = next.max(first + 1).min(starts.len());
        let end = starts.get(next).copied().unwrap_or(duration_secs);
        let chapter_title = chapters
            .iter()
            .filter(|chapter| chapter.start_seconds <= start)
            .max_by_key(|chapter| chapter.start_seconds)
            .map(|chapter| chapter.title.clone());
        out.push(Window {
            start_seconds: start,
            end_seconds: end,
            segment_starts: starts[first..next].to_vec(),
            chapter_title,
        });
        first = next;
    }
    out
}

fn nearest_later_start(starts: &[u32], first: usize, target: u32) -> usize {
    let after = starts.partition_point(|second| *second < target).max(first + 1);
    if after >= starts.len() {
        let last = starts.len() - 1;
        return if last > first { last } else { starts.len() };
    }
    let before = after.saturating_sub(1);
    if before > first && starts[before].abs_diff(target) <= starts[after].abs_diff(target) {
        before
    } else {
        after
    }
}

/// Everything one window's prompt needs.
pub struct Prompt<'a> {
    pub title: &'a str,
    pub lecture_dir: &'a str,
    pub course_dir: Option<&'a str>,
    pub window: &'a Window,
    pub cues: &'a [TranscriptCue],
}

/// Build one self-contained recap turn: transcript inline, frames by path.
pub fn prompt(job: &Prompt) -> String {
    let segments = job
        .window
        .segment_starts
        .iter()
        .map(|second| format!("  {second:>6}  {}", crate::chapters::hms(*second)))
        .collect::<Vec<_>>()
        .join("\n");
    let transcript = transcript_span(job.cues, job.window.start_seconds, job.window.end_seconds);
    let chapter = job
        .window
        .chapter_title
        .as_deref()
        .map(|title| format!("Chapter at this window: {title}\n"))
        .unwrap_or_default();
    let course = job
        .course_dir
        .map(|dir| format!("Course folder: {dir}/\n"))
        .unwrap_or_default();
    format!(
        "Write the recap notes for one window of a university lecture.\n\n\
Lecture: {title}\n\
Window: {from}–{to}\n\
{chapter}\
Recording folder: {lecture_dir}\n\
Frames: {lecture_dir}/frames/<second>.jpg\n\
{course}\n\
Segment starts (second, timestamp):\n{segments}\n\n\
Transcript for this window:\n\n{transcript}\n\n\
Rules\n\
- Write one note per segment unless two adjacent segments are genuinely one thought. You may merge adjacent segments by omitting the later start; never invent or split a segment.\n\
- Describe this moment, not the whole chapter: what the slide shows, what the lecturer is arguing, any equation or definition written, and any question asked. Use two to four sentences in present tense.\n\
- Use the lecturer's own vocabulary and notation. Put maths in $…$ or $$…$$, and code in fenced code blocks.\n\
- Say when the moment is a worked example, student question, aside, or housekeeping.\n\
- Give a short label of two to six words. It may be empty when the body is the whole point.\n\
- Some frames are the lecture theatre's AV splash screen (for example, a panel saying \"connect your laptop\"), not a slide. Ignore it and use the transcript and neighbouring frames.\n\
- The first segment in this window must have a note. Starts must be in strictly increasing play order.\n\n\
Reply with JSON and nothing else:\n\n\
[{{\"start\": {first}, \"label\": \"…\", \"body\": \"…\"}}]",
        title = job.title,
        from = crate::chapters::hms(job.window.start_seconds),
        to = crate::chapters::hms(job.window.end_seconds),
        chapter = chapter,
        lecture_dir = job.lecture_dir,
        course = course,
        segments = segments,
        transcript = transcript,
        first = job.window.segment_starts[0],
    )
}

fn transcript_span(cues: &[TranscriptCue], start: u32, end: u32) -> String {
    let lines = cues
        .iter()
        .filter(|cue| cue.start < end as f32 && cue.end >= start as f32)
        .map(|cue| {
            format!(
                "{}–{}  {}",
                crate::chapters::hms(cue.start.max(0.0) as u32),
                crate::chapters::hms(cue.end.max(0.0) as u32),
                cue.text
            )
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "(No transcript cues in this span.)".to_string()
    } else {
        lines.join("\n")
    }
}

// ── Reply parsing and validation ─────────────────────────────────────────────

/// Pull a recap array out of a bare, fenced, prose-wrapped or enveloped reply.
pub fn parse_notes(reply: &str) -> Result<Vec<RecapNote>, String> {
    for candidate in json_candidates(reply) {
        if let Some(notes) = decode_notes(&candidate) {
            return Ok(notes);
        }
    }
    Err(format!(
        "no recap note list in the reply ({} chars): {}",
        reply.chars().count(),
        clip(reply.trim(), 200)
    ))
}

fn decode_notes(text: &str) -> Option<Vec<RecapNote>> {
    let items: Vec<ReplyNote> = serde_json::from_str(text)
        .or_else(|_| serde_json::from_str::<ReplyEnvelope>(text).map(|envelope| envelope.notes))
        .ok()?;
    if items.is_empty() {
        return None;
    }
    items
        .into_iter()
        .map(|note| {
            if !note.start.is_finite()
                || note.start < 0.0
                || note.start > f64::from(u32::MAX)
                || note.start.fract() != 0.0
            {
                return None;
            }
            Some(RecapNote {
                start_seconds: note.start as u32,
                label: note.label.trim().to_string(),
                body: note.body.trim().to_string(),
            })
        })
        .collect()
}

fn json_candidates(reply: &str) -> Vec<String> {
    let mut out = vec![reply.trim().to_string()];
    let mut rest = reply;
    while let Some(open) = rest.find("```") {
        let after = &rest[open + 3..];
        let Some(newline) = after.find('\n') else { break };
        let body = &after[newline + 1..];
        match body.find("```") {
            Some(close) => {
                out.push(body[..close].trim().to_string());
                rest = &body[close + 3..];
            }
            None => {
                out.push(body.trim().to_string());
                break;
            }
        }
    }
    for open in ['[', '{'] {
        out.extend(balanced_runs(reply, open));
    }
    out
}

fn balanced_runs(text: &str, open: char) -> Vec<String> {
    const LIMIT: usize = 8;
    let close = if open == '[' { ']' } else { '}' };
    let mut out = Vec::new();
    let mut from = 0;
    while out.len() < LIMIT {
        let Some(offset) = text[from..].find(open) else { break };
        let start = from + offset;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        let mut end = None;
        for (at, ch) in text[start..].char_indices() {
            if in_string {
                match ch {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(start + at + ch.len_utf8());
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(end) => {
                out.push(text[start..end].to_string());
                from = end;
            }
            None => break,
        }
    }
    out
}

fn clip(text: &str, chars: usize) -> String {
    let head: String = text.chars().take(chars).collect();
    if text.chars().count() > chars {
        format!("{head}…")
    } else {
        head
    }
}

/// Validate one window before any of its rows are written.
pub fn validate(notes: &[RecapNote], window: &Window) -> Result<(), String> {
    if notes.is_empty() {
        return Err("no recap notes in the reply".to_string());
    }
    let first = window.segment_starts[0];
    if notes[0].start_seconds != first {
        return Err(format!(
            "note 1 ({}): the window's first segment must be present",
            crate::chapters::hms(notes[0].start_seconds)
        ));
    }
    let mut previous = None;
    for (idx, note) in notes.iter().enumerate() {
        let where_ = format!("note {} ({}): ", idx + 1, crate::chapters::hms(note.start_seconds));
        if note.body.is_empty() {
            return Err(format!("{where_}a note needs a body"));
        }
        if !window.segment_starts.contains(&note.start_seconds) {
            return Err(format!("{where_}start is not a segment in this window"));
        }
        if let Some(before) = previous {
            if note.start_seconds <= before {
                return Err(format!("{where_}start is not after the note before it ({before})"));
            }
        }
        previous = Some(note.start_seconds);
    }
    Ok(())
}

// ── Running the whole job ────────────────────────────────────────────────────

pub struct Run<'a> {
    pub data_dir: &'a Path,
    pub lecture_id: &'a str,
    pub selection: &'a crate::harness::jobs::JobSelection,
    pub force: bool,
}

pub enum Step<'a> {
    Decoding { second: u32, duration: u32 },
    Segmented { title: &'a str, duration: u32, segments: usize },
    Grabbing { done: usize, total: usize },
    Window { done: usize, total: usize, start: u32, end: u32 },
    Writing { done: usize, total: usize },
}

pub struct Outcome {
    pub title: String,
    pub duration_seconds: u32,
    pub segments: usize,
    pub windows: usize,
    pub notes: Vec<RecapNote>,
}

/// Segment, grab, ask and write a lecture recap.
///
/// Windows run strictly in sequence. A rejected or failed window is retried
/// once with the validation error appended to the original prompt. Each valid
/// window is committed before the next begins; after the second failure the
/// status records the error and the new partial set stays visible.
pub fn run(
    rt: &tokio::runtime::Handle,
    pool: &sqlx::SqlitePool,
    job: &Run,
    on_step: impl Fn(Step),
    on_event: impl Fn(&crate::harness::HarnessEvent) + Send + Sync + 'static,
) -> Result<Outcome, String> {
    use crate::harness::{self, HarnessEvent};
    use sqlx::Row;

    let id = job.lecture_id;
    let row = rt
        .block_on(
            sqlx::query(
                "SELECT l.title, l.duration_seconds, l.video_path, l.transcript_path, s.code
                   FROM lectures l LEFT JOIN subjects s ON s.id = l.subject_id
                  WHERE l.id = ?1",
            )
            .bind(id)
            .fetch_optional(pool),
        )
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no lecture {id}"))?;
    let title: String = row.get("title");
    let duration = row.get::<i64, _>("duration_seconds").max(0) as u32;
    let video: Option<String> = row.get("video_path");
    let transcript: Option<String> = row.get("transcript_path");
    let code: Option<String> = row.get("code");

    let existing = rt.block_on(crate::store::recap(pool, id))?;
    if !existing.is_empty() && !job.force {
        return Err(format!(
            "{title} already has {} recap note(s) — re-running replaces them",
            existing.len()
        ));
    }
    let video = video.ok_or_else(|| {
        format!("{title} is not downloaded — `oculus run -l --videos` fetches it")
    })?;
    let video = PathBuf::from(video);
    if !video.exists() {
        return Err(format!("{} is on record but missing from disk", video.display()));
    }
    let transcript = transcript.ok_or_else(|| {
        format!("{title} has no transcript — `oculus run -l --videos` downloads it")
    })?;
    let transcript_path = PathBuf::from(transcript);
    if !transcript_path.exists() {
        return Err(format!(
            "{} is on record but missing from disk — `oculus run -l --videos` downloads it",
            transcript_path.display()
        ));
    }
    let vtt = std::fs::read_to_string(&transcript_path)
        .map_err(|error| format!("{}: {error}", transcript_path.display()))?;
    let cues = parse_transcript(&vtt);
    if cues.is_empty() {
        return Err(format!("{} contains no transcript cues", transcript_path.display()));
    }
    let ffmpeg = crate::echo360::find_ffmpeg(None)
        .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

    if !rt.block_on(crate::store::claim_recap(pool, id))? {
        return Err(format!("a recap is already being written for {title}"));
    }

    let on_event: Arc<dyn Fn(&HarnessEvent) + Send + Sync> = Arc::new(on_event);
    let outcome = (|| -> Result<Outcome, String> {
        // `claim_recap` made this a new, empty set. From here on, each accepted
        // window becomes visible immediately; if a later one fails, those rows
        // deliberately remain as the partial result of this run.
        let mut last = std::time::Instant::now();
        let diffs = crate::chapters::sample_diffs(&ffmpeg, &video, |second| {
            if last.elapsed() >= std::time::Duration::from_millis(250) {
                last = std::time::Instant::now();
                on_step(Step::Decoding { second, duration });
            }
        })?;
        let gaps = crate::chapters::cue_gaps(&vtt);
        let starts = segment_starts(&diffs, &gaps, &cues, duration);
        if starts.is_empty() {
            return Err(format!("no recap segments in {title}"));
        }
        on_step(Step::Segmented {
            title: &title,
            duration,
            segments: starts.len(),
        });

        let dir = crate::echo360::lecture_dir(job.data_dir, id);
        let total = starts.len();
        crate::chapters::extract_frames(&ffmpeg, &video, &starts, &dir.join("frames"), |done| {
            on_step(Step::Grabbing { done, total });
        })?;

        let chapters = rt.block_on(crate::store::chapters(pool, id))?;
        let windows = windows(&starts, duration, &chapters);
        if windows.is_empty() {
            return Err(format!("no recap windows in {title}"));
        }
        let course_dir = code
            .as_deref()
            .map(|value| format!("../courses/{}", crate::paths::safe_dir(value)));
        let lecture_dir = format!("../lectures/{id}");
        let window_total = windows.len();
        let mut all_notes = Vec::new();

        for (index, window) in windows.iter().enumerate() {
            on_step(Step::Window {
                done: index + 1,
                total: window_total,
                start: window.start_seconds,
                end: window.end_seconds,
            });
            let base_prompt = prompt(&Prompt {
                title: &title,
                lecture_dir: &lecture_dir,
                course_dir: course_dir.as_deref(),
                window,
                cues: &cues,
            });
            let mut failure = None;
            let mut accepted = None;
            for attempt in 0..2 {
                let text = if attempt == 0 {
                    base_prompt.clone()
                } else {
                    format!(
                        "{base_prompt}\n\nYour previous reply was rejected: {}\n\
                         Return a corrected JSON array.",
                        failure.as_deref().unwrap_or("the agent turn failed")
                    )
                };
                let reply = Arc::new(Mutex::new(String::new()));
                let collect = reply.clone();
                let report = on_event.clone();
                let opts = harness::SendOptions {
                    model: Some(job.selection.model.clone()),
                    reasoning_effort: job.selection.reasoning_effort.clone(),
                    ..Default::default()
                };
                let turn = harness::run_once(
                    job.data_dir,
                    job.selection.provider,
                    &opts,
                    &text,
                    move |event| {
                        if let HarnessEvent::AssistantMessage { text } = event {
                            collect.lock().unwrap().push_str(text);
                        }
                        report(event);
                    },
                );
                let reply = reply.lock().unwrap().clone();
                let result = turn
                    .and_then(|()| parse_notes(&reply))
                    .and_then(|notes| validate(&notes, window).map(|()| notes));
                match result {
                    Ok(notes) => {
                        accepted = Some(notes);
                        break;
                    }
                    Err(error) => failure = Some(error),
                }
            }
            let notes = accepted.ok_or_else(|| {
                format!(
                    "window {} of {} ({}–{}) failed twice: {}",
                    index + 1,
                    window_total,
                    crate::chapters::hms(window.start_seconds),
                    crate::chapters::hms(window.end_seconds),
                    failure.unwrap_or_else(|| "unknown error".to_string())
                )
            })?;
            on_step(Step::Writing {
                done: index + 1,
                total: window_total,
            });
            rt.block_on(crate::store::save_recap_window(pool, id, &notes))?;
            all_notes.extend(notes);
        }

        rt.block_on(crate::store::set_recap_status(pool, id, Some("ready"), None))?;
        Ok(Outcome {
            title: title.clone(),
            duration_seconds: duration,
            segments: starts.len(),
            windows: window_total,
            notes: all_notes,
        })
    })();

    if let Err(error) = &outcome {
        rt.block_on(crate::store::set_recap_status(pool, id, Some("error"), Some(error)))?;
    }
    outcome
}

// ── Tauri ────────────────────────────────────────────────────────────────────

pub mod app {
    use super::*;
    use tauri::{AppHandle, Emitter};

    pub const LECTURE_RECAP_EVENT: &str = "lecture-recap";
    pub const LECTURE_RECAP_PROGRESS_EVENT: &str = "lecture-recap-progress";

    #[derive(serde::Serialize, Clone, Copy)]
    #[serde(rename_all = "camelCase")]
    struct WindowProgress {
        done: u32,
        total: u32,
    }

    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Progress {
        lecture_id: String,
        phase: &'static str,
        detail: Option<String>,
        kind: Option<crate::harness::ToolKind>,
        done: Option<u32>,
        total: Option<u32>,
        window: Option<WindowProgress>,
    }

    impl Progress {
        fn at(lecture_id: &str, phase: &'static str) -> Self {
            Self {
                lecture_id: lecture_id.to_string(),
                phase,
                detail: None,
                kind: None,
                done: None,
                total: None,
                window: None,
            }
        }
    }

    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Finished {
        lecture_id: String,
        status: &'static str,
        notes: usize,
        error: Option<String>,
    }

    #[tauri::command]
    pub async fn lecture_write_recap(
        app: AppHandle,
        lecture_id: String,
        force: Option<bool>,
    ) -> Result<(), String> {
        let pool = crate::llm::open_pool().await?;
        let running: Option<String> =
            sqlx::query_scalar("SELECT recap_status FROM lectures WHERE id = ?1")
                .bind(&lecture_id)
                .fetch_optional(&pool)
                .await
                .map_err(|error| error.to_string())?
                .flatten();
        if running.as_deref() == Some("running") {
            return Err("that lecture recap is already being written".into());
        }
        drop(pool);

        let data_dir = crate::paths::data_dir();
        let force = force.unwrap_or(false);
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => return eprintln!("[oculus] recap: {error}"),
            };
            let pool = match rt.block_on(crate::llm::open_pool()) {
                Ok(pool) => pool,
                Err(error) => return eprintln!("[oculus] recap: {error}"),
            };
            let selection = rt.block_on(crate::harness::jobs::selection(
                &pool,
                crate::harness::jobs::Job::LectureRecap,
            ));
            let window = Arc::new(Mutex::new(None::<WindowProgress>));
            let emit = {
                let app = app.clone();
                move |progress: Progress| {
                    app.emit(LECTURE_RECAP_PROGRESS_EVENT, progress).ok();
                }
            };
            let step = {
                let id = lecture_id.clone();
                let emit = emit.clone();
                let current_window = window.clone();
                move |step: Step| {
                    let progress = match step {
                        Step::Decoding { second, duration } => Progress {
                            done: Some(second),
                            total: (duration > 0).then_some(duration),
                            ..Progress::at(&id, "decoding")
                        },
                        Step::Segmented { segments, .. } => Progress {
                            done: Some(0),
                            total: Some(segments as u32),
                            ..Progress::at(&id, "frames")
                        },
                        Step::Grabbing { done, total } => Progress {
                            done: Some(done as u32),
                            total: Some(total as u32),
                            ..Progress::at(&id, "frames")
                        },
                        Step::Window { done, total, start, end } => {
                            let count = WindowProgress { done: done as u32, total: total as u32 };
                            *current_window.lock().unwrap() = Some(count);
                            Progress {
                                detail: Some(format!(
                                    "{}–{}",
                                    crate::chapters::hms(start),
                                    crate::chapters::hms(end)
                                )),
                                window: Some(count),
                                ..Progress::at(&id, "agent")
                            }
                        }
                        Step::Writing { done, total } => Progress {
                            window: Some(WindowProgress { done: done as u32, total: total as u32 }),
                            ..Progress::at(&id, "writing")
                        },
                    };
                    emit(progress);
                }
            };
            let event = {
                let id = lecture_id.clone();
                let current_window = window.clone();
                move |event: &crate::harness::HarnessEvent| {
                    let crate::harness::HarnessEvent::ToolStarted { kind, title, .. } = event else {
                        return;
                    };
                    emit(Progress {
                        detail: Some(title.clone()),
                        kind: Some(*kind),
                        window: *current_window.lock().unwrap(),
                        ..Progress::at(&id, "agent")
                    });
                }
            };
            let outcome = run(
                rt.handle(),
                &pool,
                &Run {
                    data_dir: &data_dir,
                    lecture_id: &lecture_id,
                    selection: &selection,
                    force,
                },
                step,
                event,
            );
            let finished = match outcome {
                Ok(outcome) => Finished {
                    lecture_id: lecture_id.clone(),
                    status: "ready",
                    notes: outcome.notes.len(),
                    error: None,
                },
                Err(error) => {
                    eprintln!("[oculus] recap: {error}");
                    Finished {
                        lecture_id: lecture_id.clone(),
                        status: "error",
                        notes: 0,
                        error: Some(error),
                    }
                }
            };
            app.emit(LECTURE_RECAP_EVENT, finished).ok();
        });
        Ok(())
    }

    /// Clear a stale `running` status left by a killed app or agent turn.
    pub fn reconcile(app: &AppHandle) {
        let _ = app;
        tauri::async_runtime::spawn(async {
            if let Ok(pool) = crate::llm::open_pool().await {
                if let Ok(count) = crate::store::reconcile_recap_status(&pool).await {
                    if count > 0 {
                        eprintln!("[oculus] recap: cleared {count} interrupted run(s)");
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f32, end: f32, text: &str) -> TranscriptCue {
        TranscriptCue { start, end, text: text.to_string() }
    }

    fn note(start: u32, body: &str) -> RecapNote {
        RecapNote { start_seconds: start, label: String::new(), body: body.to_string() }
    }

    #[test]
    fn transcript_parses_both_timestamp_shapes_and_plain_text() {
        let vtt = "WEBVTT\n\n1\n00:00:02.500 --> 00:00:05.000 position:10%\n\
                   <v A>First &amp; second</v>\n\n00:07.000 --> 00:09.000\n\
                   Next line\ncontinued\n";
        let cues = parse_transcript(vtt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0], cue(2.5, 5.0, "First & second"));
        assert_eq!(cues[1], cue(7.0, 9.0, "Next line continued"));
    }

    #[test]
    fn dense_candidates_keep_second_zero_and_merge_short_edges() {
        let diffs = vec![(10, 80.0), (40, 60.0), (70, 50.0), (282, 90.0)];
        let starts = segment_starts(&diffs, &[], &[], 300);
        assert_eq!(starts[0], 0);
        assert!(!starts.contains(&10), "a ten-second opening merges forward");
        assert!(!starts.contains(&282), "an eighteen-second ending merges backward");
        assert!(starts.contains(&40));
        assert!(starts.contains(&70));
    }

    #[test]
    fn a_long_segment_splits_at_the_longest_pause() {
        let cues = vec![
            cue(30.0, 80.0, "a"),
            cue(90.0, 120.0, "b"),
            cue(170.0, 200.0, "c"),
            cue(330.0, 350.0, "d"),
        ];
        let starts = segment_starts(&[], &[], &cues, 360);
        assert!(starts.contains(&330), "the 130-second pause is the longest usable one");
        assert!(starts.windows(2).all(|pair| pair[1] - pair[0] <= MAX_SEGMENT_SECS));
        assert!(360 - starts.last().unwrap() <= MAX_SEGMENT_SECS);
    }

    #[test]
    fn a_long_segment_without_a_pause_still_obeys_the_ceiling() {
        let starts = segment_starts(&[], &[], &[], 725);
        assert_eq!(starts[0], 0);
        assert!(starts.windows(2).all(|pair| pair[1] - pair[0] <= MAX_SEGMENT_SECS));
        assert!(725 - starts.last().unwrap() <= MAX_SEGMENT_SECS);
    }

    #[test]
    fn windows_cover_each_segment_once_and_snap_to_a_chapter() {
        let starts: Vec<u32> = (0..=1200).step_by(60).collect();
        let chapters = vec![crate::chapters::Chapter {
            start_seconds: 540,
            title: "The useful boundary".into(),
            summary: "A chapter".into(),
        }];
        let made = windows(&starts, 1260, &chapters);
        assert_eq!(made[0].end_seconds, 540);
        let flattened: Vec<u32> = made
            .iter()
            .flat_map(|window| window.segment_starts.clone())
            .collect();
        assert_eq!(flattened, starts);
        assert_eq!(made[1].chapter_title.as_deref(), Some("The useful boundary"));
    }

    const REPLY: &str = r#"[
      {"start": 0, "label": "Opening", "body": "Defines $x$ and motivates the proof."},
      {"start": 40, "label": "", "body": "Works through the first case."}
    ]"#;

    #[test]
    fn tolerant_json_parsing_accepts_wrappers_fences_and_aliases() {
        assert_eq!(parse_notes(REPLY).unwrap().len(), 2);
        assert_eq!(parse_notes(&format!("```json\n{REPLY}\n```" )).unwrap().len(), 2);
        assert_eq!(parse_notes(&format!("Here: {{\"notes\":{REPLY}}}" )).unwrap().len(), 2);
        let alias = r#"[{"start_seconds":0.0,"body":"Opening."}]"#;
        assert_eq!(parse_notes(alias).unwrap()[0].start_seconds, 0);
    }

    #[test]
    fn parsing_rejects_negative_and_fractional_segment_starts() {
        for start in ["-1", "0.4"] {
            let reply = format!(r#"[{{"start":{start},"body":"Opening."}}]"#);
            assert!(parse_notes(&reply).is_err(), "{start} must not be coerced");
        }
        assert_eq!(
            parse_notes(r#"[{"start":742.0,"body":"A valid whole second."}]"#)
                .unwrap()[0]
                .start_seconds,
            742
        );
    }

    #[test]
    fn validation_allows_merged_segments_but_not_missing_first_or_invented_starts() {
        let window = Window {
            start_seconds: 0,
            end_seconds: 100,
            segment_starts: vec![0, 40, 70],
            chapter_title: None,
        };
        assert!(validate(&[note(0, "Opening"), note(70, "Merged the middle")], &window).is_ok());
        let error = validate(&[note(40, "Too late")], &window).unwrap_err();
        assert!(error.starts_with("note 1 (00:00:40):"), "{error}");
        let error = validate(&[note(0, "Good"), note(55, "Invented")], &window).unwrap_err();
        assert!(error.starts_with("note 2 (00:00:55):"), "{error}");
    }

    #[test]
    fn prompt_inlines_only_the_window_transcript_and_names_frames() {
        let window = Window {
            start_seconds: 60,
            end_seconds: 120,
            segment_starts: vec![60, 90],
            chapter_title: Some("Resolution".into()),
        };
        let cues = vec![cue(10.0, 20.0, "outside"), cue(70.0, 80.0, "inside")];
        let text = prompt(&Prompt {
            title: "Lecture 4",
            lecture_dir: "../lectures/abc",
            course_dir: Some("../courses/logic"),
            window: &window,
            cues: &cues,
        });
        assert!(text.contains("inside"));
        assert!(!text.contains("outside"));
        assert!(text.contains("../lectures/abc/frames/<second>.jpg"));
        assert!(text.contains("Chapter at this window: Resolution"));
    }
}
