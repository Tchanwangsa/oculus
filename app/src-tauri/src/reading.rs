//! The reading copy: the lecture rewritten as text, one sentence per line,
//! each pinned to the second it was said — spoken maths set as maths, ASR
//! fixed from the slide.
//!
//! Shares `chapters`' detector, frame grabs and reply parsing, with a denser
//! boundary set whose slide changes become paragraph breaks. The recording is
//! split into ~ten-minute windows, each parsed, validated and written on its
//! own, so a failure leaves the completed windows visible. See
//! `docs/chapters.md`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Shortest segment (paragraph) the slide changes are thinned to.
const MIN_SEGMENT_SECS: u32 = 25;

/// Past this length a segment is split at its longest transcript pause.
const MAX_SEGMENT_SECS: u32 = 3 * 60;

/// One agent turn should carry about this much lecture.
const WINDOW_TARGET_SECS: u32 = 10 * 60;

/// How far from the target a chapter boundary may be and still become the
/// window edge.
const WINDOW_SNAP_SECS: u32 = 3 * 60;

/// The coverage ceiling that separates a rewrite from a summary: a line
/// covering more cues than this rejects the window so the model splits it.
pub const MAX_CUES_PER_LINE: usize = 8;
/// The summed cue duration (speech only, not pauses) one line may cover.
pub const MAX_SPEECH_PER_LINE_SECS: f32 = 45.0;

pub use crate::chapters::{parse_transcript, TranscriptCue};

/// One stored line of the reading copy; it ends where the next begins.
/// `start_seconds` is `floor(cue.start)` of the first cue it covers; `para` is
/// derived by [`mark_paragraphs`], never asked of the model.
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

/// One agent turn's span of the lecture; not persisted. `segment_starts` are
/// the slide changes inside it (paragraph breaks), not where lines start —
/// lines start on transcript cues.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub start_seconds: u32,
    pub end_seconds: u32,
    pub segment_starts: Vec<u32>,
    pub chapter_title: Option<String>,
}

// ── Transcript and segmentation ─────────────────────────────────────────────

/// Dense, time-ordered slide-change seconds, always beginning at 0: chapters'
/// detector thinned at [`MIN_SEGMENT_SECS`], short edge fragments merged away,
/// then any span over [`MAX_SEGMENT_SECS`] split recursively at its longest
/// transcript pause (or its midpoint, with no cue to split on).
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

/// Chunk segments into ~ten-minute windows. A chapter boundary within
/// [`WINDOW_SNAP_SECS`] of the target wins, mapped to the nearest segment
/// start so every window opens on a slide change.
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

/// The integer second a line cites for a cue, as printed in the transcript.
fn cue_second(cue: &TranscriptCue) -> u32 {
    cue.start.max(0.0).floor() as u32
}

/// The cues that belong to a window, by their start — exactly one window per
/// cue, so the prompt and the validator see the same set.
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

pub struct Prompt<'a> {
    pub title: &'a str,
    pub lecture_dir: &'a str,
    pub course_dir: Option<&'a str>,
    pub window: &'a Window,
    pub cues: &'a [TranscriptCue],
}

/// One self-contained reading-copy turn: transcript inline, frames by path.
/// The bare second is printed first because a line's `start` must be exactly
/// it (see `chapters::outline`).
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

/// The line array out of the reply, every `para` false until
/// [`mark_paragraphs`].
pub fn parse_lines(reply: &str) -> Result<Vec<ReadingLine>, String> {
    crate::chapters::parse_reply(reply, "reading line list", decode_lines)
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

/// Validate one window before any of its rows are written: non-empty text,
/// strictly increasing starts, each start a cue second in this window, the
/// first on the window's first cue, and the coverage ceiling. Every error
/// names the line and its clock.
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

/// Set `para` on a validated window: the first line, and the first line at or
/// after each slide change.
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

pub use crate::lecture_jobs::Run;

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
    pub source: crate::echo360::SourceNum,
    pub segments: usize,
    pub windows: usize,
    pub lines: Vec<ReadingLine>,
}

/// Segment, grab, ask and write a lecture's reading copy. Windows run in
/// sequence; a rejected one is retried once with the error appended, and each
/// valid one is committed before the next, so a failure leaves a partial set.
pub fn run(
    rt: &tokio::runtime::Handle,
    pool: &sqlx::SqlitePool,
    job: &Run,
    on_step: impl Fn(Step),
    on_event: impl Fn(&crate::harness::HarnessEvent) + Send + Sync + 'static,
) -> Result<Outcome, String> {
    use crate::harness::HarnessEvent;

    let id = job.lecture_id;
    let source = rt.block_on(crate::lecture_jobs::source(pool, id))?;
    let title = &source.title;
    let duration = source.duration;
    let transcript = &source.transcript;
    let code = &source.code;

    let existing = rt.block_on(crate::store::reading(pool, id))?;
    if !existing.is_empty() && !job.force {
        return Err(format!(
            "{title} already has a reading copy of {} line(s) — re-running replaces it",
            existing.len()
        ));
    }
    let video = source.video()?;
    let transcript = transcript.as_deref().ok_or_else(|| {
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
        let gaps = crate::chapters::cue_gaps(&vtt);
        let dir = crate::echo360::lecture_dir(job.data_dir, id);
        let mut last = std::time::Instant::now();
        // Raw diffs rather than candidates: this job thins at its own radius.
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
        // Its own subfolder: `extract_frames` sweeps grabs it will not
        // rewrite, and this job can run alongside a chaptering one.
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
            // No speech, nothing for a line to start on.
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
                let report = on_event.clone();
                let result = crate::lecture_jobs::reply(
                    job.data_dir, job.selection, &text, move |event| report(event),
                )
                    .and_then(|reply| parse_lines(&reply))
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
    use crate::lecture_jobs::{check_start, reconcile_status, spawn_job, Progress, WindowProgress};
    use tauri::{AppHandle, Emitter};

    pub const LECTURE_READING_EVENT: &str = "lecture-reading";
    pub const LECTURE_READING_PROGRESS_EVENT: &str = "lecture-reading-progress";

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
        check_start(
            &lecture_id,
            source,
            "reading_status",
            "that lecture's reading copy is already being written",
        )
        .await?;

        let data_dir = crate::paths::data_dir();
        let force = force.unwrap_or(false);
        spawn_job("reading", crate::harness::jobs::Job::LectureReading, move |rt, pool, selection| {
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
                pool,
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

    /// Startup: clear `running` left by a killed run.
    pub fn reconcile(_app: &AppHandle) {
        reconcile_status("reading", |pool| async move {
            crate::store::reconcile_reading_status(&pool).await
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
