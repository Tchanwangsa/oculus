//! The reading copy: the lecture as it would read on the page, one sentence
//! per line, each pinned to the second it was said.
//!
//! The transcript is speech — filler, restarts, and maths said out loud. The
//! reading copy is the same content rewritten as text: spoken maths set as
//! maths, ASR fixed from the slide, one sentence per thought. It shares
//! chapters' cheap visual detector and splash-resistant frame grabs, and keeps
//! a denser boundary set whose slide changes become paragraph breaks. The
//! recording is split into roughly ten-minute windows so a long lecture never
//! becomes one enormous agent turn. Each window is parsed, validated and
//! written on its own; a failure therefore leaves the completed windows
//! visible.
//!
//! This job replaced the lecture recap (`recap.rs`, third-person notes per
//! slide); the windows, frames and commit shape are its, with the unit and
//! the validator changed.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Paragraphs follow slide changes closely enough that looking away for
/// half a minute normally moves to at most one new paragraph.
const MIN_SEGMENT_SECS: u32 = 25;

/// A lecturer can speak over one unchanged slide for a long time. Past this
/// length the longest transcript pause becomes an extra segment boundary.
const MAX_SEGMENT_SECS: u32 = 3 * 60;

/// One agent turn should carry about this much lecture.
const WINDOW_TARGET_SECS: u32 = 10 * 60;

/// A nearby chapter boundary is a better window edge than an arbitrary slide
/// change, but a far-away one should not make a tiny or enormous turn.
const WINDOW_SNAP_SECS: u32 = 3 * 60;

/// The coverage floor: the whole difference between a reading copy and a
/// recap that drifted into summary. A line that covers more transcript than
/// this is a summary, and the window is rejected so the model splits it.
///
/// Both constants are guesses from cue statistics (Echo360 cues run ~3 s;
/// the prompt asks for two to six per line) and exist to be tuned after one
/// real run.
pub const MAX_CUES_PER_LINE: usize = 8;
/// The summed cue duration one line may cover — speech only, so a pause
/// between cues does not count against the line. See [`MAX_CUES_PER_LINE`].
pub const MAX_SPEECH_PER_LINE_SECS: f32 = 45.0;

/// One WebVTT cue, and the parser for them, both `chapters`'.
///
/// They were here first, when the reading copy (then the recap) was the only
/// job that needed the words as well as the timings. Chaptering needs them
/// now too — the outline it hands its agent is the transcript with the slide
/// changes merged in — so the parser sits beside `chapters::cue_gaps`, which
/// reads the same file for the same two timestamp shapes. One parser, so a
/// cue start in a reading window and a cue start in an outline are the same
/// second.
pub use crate::chapters::{parse_transcript, TranscriptCue};

/// One stored line of the reading copy. Its end is the next line's start (or
/// the lecture's duration), so storing an end would duplicate a fact just as
/// it would for a chapter.
///
/// `start_seconds` is `floor(cue.start)` of the first transcript cue the line
/// covers, so a line can be found from the playhead and the playhead from a
/// line. `para` is derived here from the slide changes, never asked of the
/// model — see [`mark_paragraphs`].
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct ReadingLine {
    pub start_seconds: u32,
    pub para: bool,
    pub text: String,
}

#[derive(serde::Deserialize)]
struct ReplyLine {
    #[serde(alias = "start_seconds", alias = "seconds", alias = "at")]
    start: f64,
    #[serde(alias = "body", alias = "line")]
    text: String,
}

#[derive(serde::Deserialize)]
struct ReplyEnvelope {
    #[serde(alias = "reading", alias = "notes")]
    lines: Vec<ReplyLine>,
}

/// A job-time window. Windows are intentionally not persisted: they are only
/// a way to keep each agent turn bounded.
///
/// `segment_starts` are the slide changes inside the window — what the prompt
/// lists and what `para` is derived from. They are not where a line has to
/// start: a line starts on a transcript cue.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub start_seconds: u32,
    pub end_seconds: u32,
    pub segment_starts: Vec<u32>,
    pub chapter_title: Option<String>,
}

// ── Transcript and segmentation ─────────────────────────────────────────────

/// Dense, time-ordered slide-change seconds, always beginning at second 0.
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
/// segment start so every window opens on a slide change.
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

/// The integer second a line cites for a cue: the number printed in the
/// transcript's first column, and the only form a `start` may take.
fn cue_second(cue: &TranscriptCue) -> u32 {
    cue.start.max(0.0).floor() as u32
}

/// The cues that belong to a window, by their start. A cue belongs to exactly
/// one window, so the transcript the prompt prints and the starts the
/// validator allows are the same set, and neighbouring windows never both
/// own the cue that straddles their edge.
fn window_cues<'a>(
    cues: &'a [TranscriptCue],
    window: &Window,
) -> impl Iterator<Item = &'a TranscriptCue> + 'a {
    let start = window.start_seconds as f32;
    let end = window.end_seconds as f32;
    cues.iter()
        .filter(move |cue| cue.start >= start && cue.start < end)
}

/// The second the window's first line has to start at, or `None` when the
/// window has no speech in it at all.
pub fn first_cue_start(cues: &[TranscriptCue], window: &Window) -> Option<u32> {
    window_cues(cues, window).next().map(cue_second)
}

/// Everything one window's prompt needs.
pub struct Prompt<'a> {
    pub title: &'a str,
    pub lecture_dir: &'a str,
    pub course_dir: Option<&'a str>,
    pub window: &'a Window,
    pub cues: &'a [TranscriptCue],
}

/// Build one self-contained reading-copy turn: transcript inline, frames by
/// path.
///
/// Every transcript line is printed as `second  timestamp  text`, the bare
/// second first, because that number is what a line's `start` has to be
/// *exactly*. A model asked to convert a clock to seconds will sometimes
/// round, and a rounded second is not a cue start.
pub fn prompt(job: &Prompt) -> String {
    let segments = job
        .window
        .segment_starts
        .iter()
        .map(|second| format!("  {second:>6}  {}", crate::chapters::hms(*second)))
        .collect::<Vec<_>>()
        .join("\n");
    let transcript = transcript_span(job.cues, job.window);
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
    let first = first_cue_start(job.cues, job.window).unwrap_or(job.window.start_seconds);
    format!(
        r#"Write one window of a university lecture as a reading copy: the lecture as it
would read on the page, one sentence per line, each pinned to the second it
was said.

Lecture: {title}
Window: {from}–{to}
{chapter}Recording folder: {lecture_dir}
Frames: {lecture_dir}/frames/reading/<second>.jpg — one grab per slide change
in this window, named by its second. Open them as images: they show what the
maths and the diagrams actually look like, which is how you set the notation.
{course}
Slide changes in this window (second, timestamp):
{segments}

Transcript for this window — every line is `second  timestamp  text`:
{transcript}

What a reading copy is
- The transcript is speech: filler, false starts, repeats, and maths said out
  loud ("a naught ket zero plus a one ket one"). The reading copy is the same
  content as text a student can read, skim and search — $a_0|0\rangle +
  a_1|1\rangle$. Setting spoken maths as maths is the main job; the slide
  frame tells you the notation.
- It is a rewrite, not a summary. Keep every claim, definition, formula,
  worked step, example, question and answer, in the order they were said. Drop
  only filler, restarts and repetition. If one of your lines covers more than
  about twenty seconds of speech you are summarising: split it.
- Write it as the lecture reads, in the lecturer's voice and tense — never
  "he explains that…" or "the lecturer says…". Fix speech-recognition errors
  from context and from the slide.
- One sentence per line, occasionally two short ones. Use the lecturer's own
  vocabulary and notation. Maths in $…$ or $$…$$, never Unicode look-alikes;
  code in backticks.
- Housekeeping, admin and asides are kept, but kept short.
- Some frames are the lecture theatre's AV splash screen (a panel saying
  "connect your laptop"), not a slide. Ignore it and use the transcript and
  the neighbouring frames.

Lines
- A line's `start` is the number in the first column of the first transcript
  line it covers — exactly that number, never rounded or in between.
- Lines cover the whole window in order with no gap: every transcript line
  belongs to exactly one of yours. A line covers two to six transcript lines,
  never more than eight.
- The first line starts at {first}. Start a new line at every slide change.

Reply with JSON and nothing else:

[{{"start": {first}, "text": "…"}}, {{"start": …, "text": "…"}}]"#,
        title = job.title,
        from = crate::chapters::hms(job.window.start_seconds),
        to = crate::chapters::hms(job.window.end_seconds),
        chapter = chapter,
        lecture_dir = job.lecture_dir,
        course = course,
        segments = segments,
        transcript = transcript,
        first = first,
    )
}

fn transcript_span(cues: &[TranscriptCue], window: &Window) -> String {
    let lines = window_cues(cues, window)
        .map(|cue| {
            let second = cue_second(cue);
            format!("{second:>6}  {}  {}", crate::chapters::hms(second), cue.text)
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        "(No transcript lines in this window.)".to_string()
    } else {
        lines.join("\n")
    }
}

// ── Reply parsing and validation ─────────────────────────────────────────────

/// Pull a line array out of a bare, fenced, prose-wrapped or enveloped reply.
///
/// Every line comes back with `para` false; [`mark_paragraphs`] sets it after
/// the window has been validated.
pub fn parse_lines(reply: &str) -> Result<Vec<ReadingLine>, String> {
    for candidate in json_candidates(reply) {
        if let Some(lines) = decode_lines(&candidate) {
            return Ok(lines);
        }
    }
    Err(format!(
        "no reading line list in the reply ({} chars): {}",
        reply.chars().count(),
        clip(reply.trim(), 200)
    ))
}

fn decode_lines(text: &str) -> Option<Vec<ReadingLine>> {
    let items: Vec<ReplyLine> = serde_json::from_str(text)
        .or_else(|_| serde_json::from_str::<ReplyEnvelope>(text).map(|envelope| envelope.lines))
        .ok()?;
    if items.is_empty() {
        return None;
    }
    items
        .into_iter()
        .map(|line| {
            if !line.start.is_finite()
                || line.start < 0.0
                || line.start > f64::from(u32::MAX)
                || line.start.fract() != 0.0
            {
                return None;
            }
            Some(ReadingLine {
                start_seconds: line.start as u32,
                para: false,
                text: line.text.trim().to_string(),
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
///
/// Five rules, every failure naming the line and its clock: a non-empty
/// reply with non-empty text; starts strictly increasing; every start the
/// integer second of a cue in this window; the first line on the window's
/// first cue; and the coverage floor — the cues from a line's start up to
/// the next line's start number at most [`MAX_CUES_PER_LINE`] and speak for
/// at most [`MAX_SPEECH_PER_LINE_SECS`] between them. The last is what
/// separates a reading copy from a summary: a model that folds a minute of
/// speech into one sentence has stopped rewriting.
pub fn validate(
    lines: &[ReadingLine],
    window: &Window,
    cues: &[TranscriptCue],
) -> Result<(), String> {
    if lines.is_empty() {
        return Err("no reading lines in the reply".to_string());
    }
    let cues: Vec<&TranscriptCue> = window_cues(cues, window).collect();
    let Some(first) = cues.first().map(|cue| cue_second(cue)) else {
        return Err("no transcript lines in this window".to_string());
    };
    if lines[0].start_seconds != first {
        return Err(format!(
            "line 1 ({}): the first line must start on the window's first transcript line, {first}",
            crate::chapters::hms(lines[0].start_seconds)
        ));
    }
    let mut previous = None;
    for (idx, line) in lines.iter().enumerate() {
        let where_ = format!("line {} ({}): ", idx + 1, crate::chapters::hms(line.start_seconds));
        if line.text.trim().is_empty() {
            return Err(format!("{where_}a line needs text"));
        }
        if !cues.iter().any(|cue| cue_second(cue) == line.start_seconds) {
            return Err(format!(
                "{where_}start is not the second of a transcript line in this window"
            ));
        }
        if let Some(before) = previous {
            if line.start_seconds <= before {
                return Err(format!("{where_}start is not after the line before it ({before})"));
            }
        }
        previous = Some(line.start_seconds);

        let until = lines.get(idx + 1).map(|next| next.start_seconds);
        let covered = cues.iter().copied().filter(|cue| {
            let second = cue_second(cue);
            second >= line.start_seconds && until.is_none_or(|until| second < until)
        });
        let (count, speech) = covered.fold((0usize, 0.0f32), |(count, speech), cue| {
            (count + 1, speech + (cue.end - cue.start).max(0.0))
        });
        if count > MAX_CUES_PER_LINE {
            return Err(format!(
                "{where_}covers {count} transcript lines, more than {MAX_CUES_PER_LINE} — split it"
            ));
        }
        if speech > MAX_SPEECH_PER_LINE_SECS {
            return Err(format!(
                "{where_}covers {speech:.0} s of speech, more than {MAX_SPEECH_PER_LINE_SECS:.0} — split it"
            ));
        }
    }
    Ok(())
}

/// Set `para` on a validated, ordered window of lines: the first line, and
/// the first line at or after each slide change. Derived here rather than
/// asked of the model because the slide changes are already known to the
/// second, and a model asked to mark them would mark some other set.
pub fn mark_paragraphs(lines: &mut [ReadingLine], slide_changes: &[u32]) {
    for line in lines.iter_mut() {
        line.para = false;
    }
    if let Some(first) = lines.first_mut() {
        first.para = true;
    }
    for &change in slide_changes {
        if let Some(line) = lines.iter_mut().find(|line| line.start_seconds >= change) {
            line.para = true;
        }
    }
}

// ── Running the whole job ────────────────────────────────────────────────────

pub struct Run<'a> {
    pub data_dir: &'a Path,
    pub lecture_id: &'a str,
    pub selection: &'a crate::harness::jobs::JobSelection,
    pub force: bool,
    /// Read this stream instead of letting `chapters::detect` choose. The
    /// reading copy carries it for the same reason chaptering does and with
    /// the same meaning: the two jobs decode the same file and had the same
    /// blind spot, so fixing one and leaving the other would only hide it.
    pub source: Option<crate::echo360::SourceNum>,
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
    /// Which stream the segments were detected on.
    pub source: crate::echo360::SourceNum,
    pub segments: usize,
    pub windows: usize,
    pub lines: Vec<ReadingLine>,
}

/// Segment, grab, ask and write a lecture's reading copy.
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

    let existing = rt.block_on(crate::store::reading(pool, id))?;
    if !existing.is_empty() && !job.force {
        return Err(format!(
            "{title} already has a reading copy of {} line(s) — re-running replaces it",
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

    if !rt.block_on(crate::store::claim_reading(pool, id))? {
        return Err(format!("a reading copy is already being written for {title}"));
    }

    let on_event: Arc<dyn Fn(&HarnessEvent) + Send + Sync> = Arc::new(on_event);
    let outcome = (|| -> Result<Outcome, String> {
        // `claim_reading` made this a new, empty set. From here on, each
        // accepted window becomes visible immediately; if a later one fails,
        // those rows deliberately remain as the partial result of this run.
        let gaps = crate::chapters::cue_gaps(&vtt);
        let dir = crate::echo360::lecture_dir(job.data_dir, id);
        let mut last = std::time::Instant::now();
        // Which of the capture's streams actually holds the slides; see
        // `chapters::detect`. The reading copy reads the raw diffs rather
        // than the candidates because it thins them at its own radius.
        let detected = crate::chapters::detect(
            &ffmpeg,
            &dir,
            &video,
            &gaps,
            duration,
            job.source,
            |second| {
                if last.elapsed() >= std::time::Duration::from_millis(250) {
                    last = std::time::Instant::now();
                    on_step(Step::Decoding { second, duration });
                }
            },
        )?;
        let starts = segment_starts(&detected.diffs, &gaps, &cues, duration);
        if starts.is_empty() {
            return Err(format!("no reading segments in {title}"));
        }
        on_step(Step::Segmented {
            title: &title,
            duration,
            segments: starts.len(),
        });

        let total = starts.len();
        // A subfolder of chapters', the way the chat dock's `live/` is.
        // `extract_frames` now deletes the grabs a run will not rewrite, and
        // the two jobs are independently claimed — a reading copy can start
        // while a chaptering turn is still open on the same lecture — so
        // sharing one folder would let either job pull the other's frames
        // out from under it. This job's boundaries are thinned at 25 s
        // against chapters' 90 s, so they would mostly not survive each
        // other's sweep.
        crate::chapters::extract_frames(
            &ffmpeg,
            &detected.video,
            &starts,
            &dir.join("frames").join("reading"),
            |done| on_step(Step::Grabbing { done, total }),
        )?;

        let chapters = rt.block_on(crate::store::chapters(pool, id))?;
        let windows = windows(&starts, duration, &chapters);
        if windows.is_empty() {
            return Err(format!("no reading windows in {title}"));
        }
        let course_dir = code
            .as_deref()
            .map(|value| format!("../courses/{}", crate::paths::safe_dir(value)));
        let lecture_dir = format!("../lectures/{id}");
        let window_total = windows.len();
        let mut all_lines = Vec::new();

        for (index, window) in windows.iter().enumerate() {
            // A window with no speech in it has no reading copy: there is
            // nothing a line could start on, so asking would only spend two
            // turns to be told so.
            if first_cue_start(&cues, window).is_none() {
                continue;
            }
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
                    .and_then(|()| parse_lines(&reply))
                    .and_then(|lines| validate(&lines, window, &cues).map(|()| lines));
                match result {
                    Ok(lines) => {
                        accepted = Some(lines);
                        break;
                    }
                    Err(error) => failure = Some(error),
                }
            }
            let mut lines = accepted.ok_or_else(|| {
                format!(
                    "window {} of {} ({}–{}) failed twice: {}",
                    index + 1,
                    window_total,
                    crate::chapters::hms(window.start_seconds),
                    crate::chapters::hms(window.end_seconds),
                    failure.unwrap_or_else(|| "unknown error".to_string())
                )
            })?;
            mark_paragraphs(&mut lines, &window.segment_starts);
            on_step(Step::Writing {
                done: index + 1,
                total: window_total,
            });
            rt.block_on(crate::store::save_reading_window(pool, id, &lines))?;
            all_lines.extend(lines);
        }

        rt.block_on(crate::store::set_reading_status(pool, id, Some("ready"), None))?;
        Ok(Outcome {
            title: title.clone(),
            duration_seconds: duration,
            source: detected.source,
            segments: starts.len(),
            windows: window_total,
            lines: all_lines,
        })
    })();

    if let Err(error) = &outcome {
        rt.block_on(crate::store::set_reading_status(pool, id, Some("error"), Some(error)))?;
    }
    outcome
}

// ── Tauri ────────────────────────────────────────────────────────────────────

pub mod app {
    use super::*;
    use tauri::{AppHandle, Emitter};

    pub const LECTURE_READING_EVENT: &str = "lecture-reading";
    pub const LECTURE_READING_PROGRESS_EVENT: &str = "lecture-reading-progress";

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
        lines: usize,
        error: Option<String>,
    }

    #[tauri::command]
    pub async fn lecture_write_reading(
        app: AppHandle,
        lecture_id: String,
        force: Option<bool>,
        source: Option<u8>,
    ) -> Result<(), String> {
        if let Some(n) = source {
            if n != 1 && n != 2 {
                return Err(format!("{n} is not a source — a capture has 1 and sometimes 2"));
            }
        }
        let pool = crate::store::open_pool().await?;
        let running: Option<String> =
            sqlx::query_scalar("SELECT reading_status FROM lectures WHERE id = ?1")
                .bind(&lecture_id)
                .fetch_optional(&pool)
                .await
                .map_err(|error| error.to_string())?
                .flatten();
        if running.as_deref() == Some("running") {
            return Err("that lecture's reading copy is already being written".into());
        }
        drop(pool);

        let data_dir = crate::paths::data_dir();
        let force = force.unwrap_or(false);
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(runtime) => runtime,
                Err(error) => return eprintln!("[oculus] reading: {error}"),
            };
            let pool = match rt.block_on(crate::store::open_pool()) {
                Ok(pool) => pool,
                Err(error) => return eprintln!("[oculus] reading: {error}"),
            };
            let selection = rt.block_on(crate::harness::jobs::selection(
                &pool,
                crate::harness::jobs::Job::LectureReading,
            ));
            let window = Arc::new(Mutex::new(None::<WindowProgress>));
            let emit = {
                let app = app.clone();
                move |progress: Progress| {
                    app.emit(LECTURE_READING_PROGRESS_EVENT, progress).ok();
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
                    source,
                },
                step,
                event,
            );
            let finished = match outcome {
                Ok(outcome) => Finished {
                    lecture_id: lecture_id.clone(),
                    status: "ready",
                    lines: outcome.lines.len(),
                    error: None,
                },
                Err(error) => {
                    eprintln!("[oculus] reading: {error}");
                    Finished {
                        lecture_id: lecture_id.clone(),
                        status: "error",
                        lines: 0,
                        error: Some(error),
                    }
                }
            };
            app.emit(LECTURE_READING_EVENT, finished).ok();
        });
        Ok(())
    }

    /// Clear a stale `running` status left by a killed app or agent turn.
    pub fn reconcile(app: &AppHandle) {
        let _ = app;
        tauri::async_runtime::spawn(async {
            if let Ok(pool) = crate::store::open_pool().await {
                if let Ok(count) = crate::store::reconcile_reading_status(&pool).await {
                    if count > 0 {
                        eprintln!("[oculus] reading: cleared {count} interrupted run(s)");
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

    fn line(start: u32, text: &str) -> ReadingLine {
        ReadingLine { start_seconds: start, para: false, text: text.to_string() }
    }

    /// `count` cues, `step` seconds apart from `from`, each `length` long.
    fn cues_every(from: u32, count: usize, step: u32, length: f32) -> Vec<TranscriptCue> {
        (0..count)
            .map(|n| {
                let start = (from + n as u32 * step) as f32;
                cue(start, start + length, &format!("cue at {start}"))
            })
            .collect()
    }

    /// Second 0 to 100 with slide changes at 0, 40 and 70.
    fn window() -> Window {
        Window {
            start_seconds: 0,
            end_seconds: 100,
            segment_starts: vec![0, 40, 70],
            chapter_title: None,
        }
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
      {"start": 0, "text": "We define $x$ and set out what the proof needs."},
      {"start": 40, "text": "The first case is the one where $x = 0$."}
    ]"#;

    #[test]
    fn tolerant_json_parsing_accepts_wrappers_fences_and_aliases() {
        assert_eq!(parse_lines(REPLY).unwrap().len(), 2);
        assert_eq!(parse_lines(&format!("```json\n{REPLY}\n```")).unwrap().len(), 2);
        assert_eq!(parse_lines(&format!("Here: {{\"lines\":{REPLY}}}")).unwrap().len(), 2);
        let parsed = parse_lines(r#"[{"start_seconds":0.0,"body":"Opening."}]"#).unwrap();
        assert_eq!(parsed[0], line(0, "Opening."));
        let parsed = parse_lines(r#"[{"at":7,"line":" Trimmed. "}]"#).unwrap();
        assert_eq!(parsed[0], line(7, "Trimmed."));
        let parsed = parse_lines(r#"[{"seconds":9,"text":"Nine."}]"#).unwrap();
        assert_eq!(parsed[0].start_seconds, 9);
    }

    #[test]
    fn parsing_rejects_negative_and_fractional_starts() {
        for start in ["-1", "0.4"] {
            let reply = format!(r#"[{{"start":{start},"text":"Opening."}}]"#);
            assert!(parse_lines(&reply).is_err(), "{start} must not be coerced");
        }
        assert_eq!(
            parse_lines(r#"[{"start":742.0,"text":"A valid whole second."}]"#)
                .unwrap()[0]
                .start_seconds,
            742
        );
    }

    #[test]
    fn validation_accepts_lines_on_cue_starts_covering_the_window() {
        // Twenty 3-second cues, one every 5 s; five lines of four cues each.
        let cues = cues_every(0, 20, 5, 3.0);
        let lines = [line(0, "a"), line(20, "b"), line(40, "c"), line(60, "d"), line(80, "e")];
        assert!(validate(&lines, &window(), &cues).is_ok());
    }

    #[test]
    fn validation_rejects_an_empty_reply_and_empty_text() {
        let cues = cues_every(0, 20, 5, 3.0);
        assert!(validate(&[], &window(), &cues).is_err());
        let error = validate(&[line(0, "a"), line(20, "  ")], &window(), &cues).unwrap_err();
        assert!(error.starts_with("line 2 (00:00:20):"), "{error}");
    }

    #[test]
    fn validation_rejects_a_start_that_is_not_a_cue_start() {
        let cues = cues_every(0, 20, 5, 3.0);
        let error = validate(&[line(0, "a"), line(12, "between cues")], &window(), &cues)
            .unwrap_err();
        assert!(error.starts_with("line 2 (00:00:12):"), "{error}");
        // A cue's start is its floor: 22.6 s is cited as 22, and 23 is nobody's.
        let cues = vec![cue(0.0, 3.0, "a"), cue(22.6, 25.0, "b")];
        assert!(validate(&[line(0, "a"), line(22, "b")], &window(), &cues).is_ok());
        assert!(validate(&[line(0, "a"), line(23, "b")], &window(), &cues).is_err());
    }

    #[test]
    fn validation_rejects_a_first_line_off_the_windows_first_cue() {
        let cues = cues_every(0, 20, 5, 3.0);
        let error = validate(&[line(5, "late")], &window(), &cues).unwrap_err();
        assert!(error.starts_with("line 1 (00:00:05):"), "{error}");
        // The first cue *in the window*, not the transcript's first cue.
        let later = Window { start_seconds: 50, end_seconds: 100, ..window() };
        assert!(validate(&[line(50, "a"), line(75, "b")], &later, &cues).is_ok());
        assert!(validate(&[line(0, "a")], &later, &cues).is_err());
    }

    #[test]
    fn validation_rejects_starts_out_of_order() {
        let cues = cues_every(0, 20, 5, 3.0);
        let error = validate(&[line(0, "a"), line(20, "b"), line(20, "c")], &window(), &cues)
            .unwrap_err();
        assert!(error.starts_with("line 3 (00:00:20):"), "{error}");
    }

    #[test]
    fn coverage_rejects_a_line_over_nine_cues() {
        // Line 1 would cover the cues at 0, 5, …, 40: nine of them.
        let cues = cues_every(0, 20, 5, 3.0);
        let error = validate(&[line(0, "too much"), line(45, "rest")], &window(), &cues)
            .unwrap_err();
        assert!(error.starts_with("line 1 (00:00:00):"), "{error}");
        assert!(error.contains("9 transcript lines"), "{error}");
        // Eight is the ceiling, and passes.
        assert!(validate(&[line(0, "a"), line(40, "b"), line(80, "c")], &window(), &cues).is_ok());
    }

    #[test]
    fn coverage_rejects_a_line_over_forty_five_seconds_of_speech() {
        // Six 10-second cues back to back: 0–10, 10–20, … 50–60.
        let cues = cues_every(0, 6, 10, 10.0);
        let window = Window { end_seconds: 60, ..window() };
        // Five cues is under the count ceiling but 50 s of speech.
        let error = validate(&[line(0, "long"), line(50, "rest")], &window, &cues).unwrap_err();
        assert!(error.starts_with("line 1 (00:00:00):"), "{error}");
        assert!(error.contains("50 s of speech"), "{error}");
        // Silence between cues does not count: the same five starts with
        // 2-second cues is 10 s of speech.
        let sparse = cues_every(0, 6, 10, 2.0);
        assert!(validate(&[line(0, "short"), line(50, "rest")], &window, &sparse).is_ok());
    }

    #[test]
    fn paragraphs_open_at_the_first_line_and_at_each_slide_change() {
        let mut lines = [line(0, "a"), line(20, "b"), line(40, "c"), line(60, "d"), line(80, "e")];
        mark_paragraphs(&mut lines, &[0, 40, 70]);
        let para: Vec<bool> = lines.iter().map(|line| line.para).collect();
        // 0 is the first line; 40 sits on a slide change; 80 is the first
        // line at or after the change at 70; 20 and 60 are mid-paragraph.
        assert_eq!(para, vec![true, false, true, false, true]);
        // No slide changes at all still opens the window with a paragraph.
        mark_paragraphs(&mut lines, &[]);
        let para: Vec<bool> = lines.iter().map(|line| line.para).collect();
        assert_eq!(para, vec![true, false, false, false, false]);
    }

    #[test]
    fn prompt_inlines_only_the_window_transcript_with_integer_starts() {
        let window = Window {
            start_seconds: 60,
            end_seconds: 120,
            segment_starts: vec![60, 90],
            chapter_title: Some("Resolution".into()),
        };
        let cues = vec![
            cue(10.0, 20.0, "outside"),
            cue(70.4, 80.0, "inside"),
            cue(120.0, 130.0, "next window"),
        ];
        let text = prompt(&Prompt {
            title: "Lecture 4",
            lecture_dir: "../lectures/abc",
            course_dir: Some("../courses/logic"),
            window: &window,
            cues: &cues,
        });
        assert!(text.contains("    70  00:01:10  inside"), "{text}");
        assert!(!text.contains("outside"));
        assert!(!text.contains("next window"), "a cue on the end edge is the next window's");
        assert!(text.contains("../lectures/abc/frames/reading/<second>.jpg"));
        assert!(text.contains("Slide changes in this window (second, timestamp):\n      60  00:01:00\n      90  00:01:30\n"));
        assert!(text.contains("Chapter at this window: Resolution"));
        assert!(text.contains("Course folder: ../courses/logic/"));
        assert!(text.contains("The first line starts at 70."));
        assert!(text.contains(r#"[{"start": 70, "text": "…"}"#));
        // `\r` in the LaTeX would be a carriage return in an ordinary string
        // literal; the prompt is a raw string so the backslash survives.
        assert!(text.contains(r"$a_0|0\rangle +"), "LaTeX survives the literal");
        assert!(text.contains(r"a_1|1\rangle$"), "{text}");
    }
}
