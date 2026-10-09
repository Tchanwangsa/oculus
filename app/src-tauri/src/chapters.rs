//! Lecture chapters: where a recording changes topic, and the agent job that
//! names them.
//!
//! Detection is visual: a slide capture is dead still between slides and a
//! cliff at a change, so one fixed threshold separates them (a room camera has
//! no such gap, which is why [`detect`] picks the stream rather than tuning).
//! Transcript pauses only nudge a candidate's score. Nothing is cached —
//! re-detecting is one fast decode. Measurements behind every constant here
//! are in `docs/chapters.md`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 160×90 greyscale: the geometry ffmpeg is asked for, so one frame's size on
/// the pipe.
const FRAME_W: usize = 160;
const FRAME_H: usize = 90;
const FRAME_BYTES: usize = FRAME_W * FRAME_H;

/// Mean absolute difference above which a frame pair counts as a change. Sits
/// in the empty middle of a bimodal distribution, so it is not a setting.
const DIFF_THRESHOLD: f32 = 6.0;

/// Loud frames this close together (a dissolve, a build) are one event,
/// reported at its first frame.
const COLLAPSE_SECS: u32 = 3;

/// A silence at least this long counts as a pause between topics.
const PAUSE_SECS: f32 = 2.0;

/// How far from a change-point a pause may sit and still be about it.
const PAUSE_WINDOW: u32 = 8;

/// Added to a candidate's score when a pause supports it. Small against the
/// magnitude scale: it reorders near-equals during thinning, never more.
const PAUSE_BONUS: f32 = 3.0;

/// No two boundaries closer than this: a shorter chapter is a slide, not a topic.
const MIN_SPACING: u32 = 90;

/// At most this many candidates in a whole recording means the stream is dead,
/// not that the lecture was quiet. A failed capture gives ~1, a healthy one
/// well over ten, so this is a which-file decision rather than a knob.
const DEAD_SOURCE: usize = 2;

/// Offsets past a boundary to probe when grabbing its frame, in preference
/// order: clear the cut, step over a dropout, and the boundary itself last.
const GRAB_OFFSETS: [u32; 4] = [2, 6, 12, 0];

/// Width of a chaptering run's frames: small, since a run writes dozens, but
/// slide titles and formulas stay readable.
const GRAB_WIDTH: u32 = 768;

/// Width cap for the dock's live grab: one or two frames per message, and the
/// question may be about a whiteboard, so in practice the stream's own width.
const LIVE_GRAB_WIDTH: u32 = 1536;

/// Width of the Up Next card's thumbnail (`app::lecture_thumbnail`), drawn a
/// little over 128 px wide.
const THUMB_WIDTH: u32 = 320;

/// How close to the most detailed probe a frame must be to be taken instead of
/// it. Relative, because "detailed" depends on the deck.
const GRAB_TOLERANCE: f32 = 0.95;

/// One place the lecture plausibly changes topic.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Candidate {
    /// Offset into the recording, in whole seconds.
    pub seconds: u32,
    /// Visual magnitude plus the pause bonus; comparable within one lecture only.
    pub score: f32,
    /// The raw mean-absolute-difference that triggered it, before any bonus.
    pub diff: f32,
    /// Whether a transcript silence backed this boundary up.
    pub pause: bool,
}

// ── The decode pass ──────────────────────────────────────────────────────────

/// Mean absolute difference between each sampled frame (`fps=1`, so frame *n*
/// is second *n*; the first entry is second 1) and the one before it. Frames
/// are diffed as they arrive rather than held.
///
/// `on_frame` fires per frame; the caller throttles.
pub fn sample_diffs(
    ffmpeg: &Path,
    video: &Path,
    mut on_frame: impl FnMut(u32),
) -> Result<Vec<(u32, f32)>, String> {
    let mut previous = vec![0u8; FRAME_BYTES];
    let mut diffs: Vec<(u32, f32)> = Vec::new();
    each_frame(ffmpeg, video, "error", &[], |index, frame| {
        if index > 0 {
            diffs.push((index, mean_abs_diff(&previous, frame)));
            on_frame(index);
        }
        previous.copy_from_slice(frame);
    })?;
    if diffs.is_empty() {
        return Err("no video frames decoded".to_string());
    }
    Ok(diffs)
}

/// The mean brightness (0–255) of each second of the last `secs` seconds of
/// `video`, from one decode of the tail alone (`-sseof`), and the file's own
/// length from ffmpeg's log. ffmpeg ignores a seek before the start, so a
/// shorter file is read whole.
pub fn tail_luma(
    ffmpeg: &Path,
    video: &Path,
    secs: u32,
) -> Result<(Vec<f32>, Option<f64>), String> {
    let seek = format!("-{secs}");
    let mut luma: Vec<f32> = Vec::new();
    let log = each_frame(ffmpeg, video, "info", &["-sseof", &seek], |_, frame| {
        let total: u64 = frame.iter().map(|&b| u64::from(b)).sum();
        luma.push(total as f32 / frame.len() as f32);
    })?;
    if luma.is_empty() {
        return Err("no video frames decoded".to_string());
    }
    Ok((luma, crate::transcribe::audio::duration(&log)))
}

/// One `fps=1`, 160×90 greyscale decode of `video` to a pipe, each frame
/// handed to `on_frame` with its index as it arrives; answers ffmpeg's log at
/// `level`. `seek` goes before `-i`.
fn each_frame(
    ffmpeg: &Path,
    video: &Path,
    level: &str,
    seek: &[&str],
    mut on_frame: impl FnMut(u32, &[u8]),
) -> Result<String, String> {
    let mut child = Command::new(ffmpeg)
        .args(["-v", level, "-nostdin", "-hide_banner", "-nostats"])
        .args(seek)
        .arg("-i")
        .arg(video)
        .args([
            "-vf",
            &format!("fps=1,scale={FRAME_W}:{FRAME_H},format=gray"),
            "-f",
            "rawvideo",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run ffmpeg: {e}"))?;

    // Drained on its own thread: ffmpeg blocks on a full stderr pipe.
    let mut stderr = child.stderr.take().expect("piped stderr");
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).ok();
        text
    });

    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut current = vec![0u8; FRAME_BYTES];
    let mut index: u32 = 0;

    loop {
        match read_frame(&mut stdout, &mut current) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => {
                child.kill().ok();
                child.wait().ok();
                return Err(format!("reading frames: {e}"));
            }
        }
        on_frame(index, &current);
        index += 1;
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    let text = errors.join().unwrap_or_default();
    if !status.success() {
        let detail = text
            .lines()
            .last()
            .unwrap_or("no detail")
            .trim()
            .to_string();
        return Err(format!("ffmpeg failed: {detail}"));
    }
    Ok(text)
}

/// Fill `frame` completely, or report that the stream ended. A trailing
/// partial frame (ffmpeg cut off mid-write) is dropped.
fn read_frame(source: &mut impl Read, frame: &mut [u8]) -> std::io::Result<bool> {
    let mut filled = 0;
    while filled < frame.len() {
        match source.read(&mut frame[filled..])? {
            0 => return Ok(false),
            n => filled += n,
        }
    }
    Ok(true)
}

fn mean_abs_diff(a: &[u8], b: &[u8]) -> f32 {
    let total: u64 = a
        .iter()
        .zip(b.iter())
        .map(|(x, y)| u64::from(x.abs_diff(*y)))
        .sum();
    total as f32 / a.len() as f32
}

// ── The transcript half ──────────────────────────────────────────────────────

/// The silence before each cue, paired with the second that cue starts at.
/// The timing half of the frontend's `parseVtt` (`app/src/lib/lectures.ts`).
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

// ── Scoring and thinning ─────────────────────────────────────────────────────

/// Raw frame diffs and transcript gaps to a thinned, time-ordered set of
/// boundary candidates: keep loud frames, collapse runs onto their first
/// second, score with a pause bonus, then thin to [`MIN_SPACING`] strongest
/// first (so the important one of two close boundaries survives).
pub fn candidates(diffs: &[(u32, f32)], gaps: &[(u32, f32)], duration_secs: u32) -> Vec<Candidate> {
    // A collapsed run keeps its peak magnitude.
    let mut collapsed: Vec<(u32, f32)> = Vec::new();
    for &(second, diff) in diffs {
        if diff < DIFF_THRESHOLD {
            continue;
        }
        if duration_secs > 0 && second >= duration_secs {
            continue;
        }
        match collapsed.last_mut() {
            Some(last) if second - last.0 <= COLLAPSE_SECS => {
                if diff > last.1 {
                    last.1 = diff;
                }
            }
            _ => collapsed.push((second, diff)),
        }
    }

    let pauses: Vec<u32> = gaps
        .iter()
        .filter(|(_, gap)| *gap >= PAUSE_SECS)
        .map(|(start, _)| *start)
        .collect();

    let mut scored: Vec<Candidate> = collapsed
        .into_iter()
        .map(|(seconds, diff)| {
            let pause = pauses.iter().any(|p| p.abs_diff(seconds) <= PAUSE_WINDOW);
            Candidate {
                seconds,
                score: diff + if pause { PAUSE_BONUS } else { 0.0 },
                diff,
                pause,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.seconds.cmp(&b.seconds))
    });
    let mut kept: Vec<Candidate> = Vec::new();
    for candidate in scored {
        if kept
            .iter()
            .all(|k| k.seconds.abs_diff(candidate.seconds) >= MIN_SPACING)
        {
            kept.push(candidate);
        }
    }
    kept.sort_by_key(|c| c.seconds);
    kept
}

// ── Which stream to read ────────────────────────────────────────────

/// The stream a run read, and what came out of reading it.
pub struct Detection {
    pub source: crate::echo360::SourceNum,
    /// The file the diffs came from, and so the one frame grabs must use.
    pub video: PathBuf,
    /// The raw per-second frame diffs, before [`candidates`] thins them.
    pub diffs: Vec<(u32, f32)>,
    /// [`candidates`] over those diffs, in play order.
    pub candidates: Vec<Candidate>,
}

/// Decode the lecture's slide capture, whichever stream that is (it varies
/// per lecture, so it is measured each time, never persisted).
///
/// Source 1 is kept unless it is dead (at most [`DEAD_SOURCE`] candidates);
/// only then is source 2 decoded. Never pick by candidate count: a room camera
/// saturates the threshold and out-scores a real deck. `source` (`--source`,
/// the app's picker) overrides all of it. `on_frame` fires for both passes.
pub fn detect(
    ffmpeg: &Path,
    lecture_dir: &Path,
    video: &Path,
    gaps: &[(u32, f32)],
    duration_secs: u32,
    source: Option<crate::echo360::SourceNum>,
    mut on_frame: impl FnMut(u32),
) -> Result<Detection, String> {
    // The file on disk, not the `video2_path` column: a stream downloaded but
    // never recorded is still readable.
    let second = crate::echo360::source_path(lecture_dir, 2);

    let read = |path: PathBuf,
                source: crate::echo360::SourceNum,
                on_frame: &mut dyn FnMut(u32)|
     -> Result<Detection, String> {
        let diffs = sample_diffs(ffmpeg, &path, on_frame)?;
        let candidates = candidates(&diffs, gaps, duration_secs);
        Ok(Detection {
            source,
            video: path,
            diffs,
            candidates,
        })
    };

    if source == Some(2) {
        if !second.exists() {
            return Err(format!(
                "{} has no second source on disk — `oculus run -l --videos` fetches both",
                lecture_dir.display()
            ));
        }
        return read(second, 2, &mut on_frame);
    }

    let first = read(video.to_path_buf(), 1, &mut on_frame)?;
    if source.is_none() && first.candidates.len() <= DEAD_SOURCE && second.exists() {
        eprintln!(
            "[oculus] source 1 gave {} candidate(s) — reading source 2 instead",
            first.candidates.len()
        );
        return read(second, 2, &mut on_frame);
    }
    Ok(first)
}

// ── Frames for a later stage ─────────────────────────────────────────────────

/// One probed JPEG per boundary at `<out_dir>/<second>.jpg` (see
/// [`grab_frame`]); the name keeps the boundary second, not the probed offset.
/// `on_grab` gets the running count.
///
/// JPEGs a previous run left that this one will not rewrite are deleted first,
/// so the folder never holds frames from another candidate set or stream.
/// The `live/` subfolder belongs to another job and is left alone.
pub fn extract_frames(
    ffmpeg: &Path,
    video: &Path,
    secs: &[u32],
    out_dir: &Path,
    mut on_grab: impl FnMut(usize),
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    sweep_orphans(out_dir, secs);
    let mut written = Vec::with_capacity(secs.len());
    for &second in secs {
        let out = out_dir.join(format!("{second}.jpg"));
        grab_frame(ffmpeg, video, second, GRAB_WIDTH, &out)?;
        written.push(out);
        on_grab(written.len());
    }
    Ok(written)
}

/// Delete the `<second>.jpg` files in `out_dir` not in `keep`. Best effort;
/// any other name was not written here and is left alone.
fn sweep_orphans(out_dir: &Path, keep: &[u32]) {
    let Ok(entries) = std::fs::read_dir(out_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
            continue;
        }
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let second = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok());
        match second {
            Some(second) if keep.contains(&second) => continue,
            Some(_) => {
                std::fs::remove_file(&path).ok();
            }
            None => continue,
        }
    }
}

/// One JPEG of `second`, written to `out`, at most `width` px wide (never
/// upscaled).
///
/// The boundary second itself is often black (screen share restarting) or a
/// couple of seconds later the room's "connect your laptop" splash, so
/// [`GRAB_OFFSETS`] are probed with the same seek the grab uses and the
/// earliest within [`GRAB_TOLERANCE`] of the most detailed wins. Also used by
/// the dock's live grab (`app::lecture_grab_frames`).
pub fn grab_frame(
    ffmpeg: &Path,
    video: &Path,
    second: u32,
    width: u32,
    out: &Path,
) -> Result<(), String> {
    let at = best_offset(ffmpeg, video, second);
    // Escaped: an unescaped comma would end the filter.
    let scale = format!("scale=min({width}\\,iw):-2");
    let status = Command::new(ffmpeg)
        .args([
            "-v",
            "error",
            "-nostdin",
            "-y",
            "-ss",
            &at.to_string(),
            "-i",
        ])
        .arg(video)
        .args(["-frames:v", "1", "-vf", &scale, "-q:v", "3"])
        .arg(out)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("could not run ffmpeg: {e}"))?;
    if !status.success() || !out.exists() {
        return Err(format!("no frame at {at}s"));
    }
    Ok(())
}

/// Which stream and second a lecture's thumbnail is grabbed from: source 1
/// when it is downloaded, else source 2, a quarter of the way in — past the
/// title card and the room's splash. `None` with nothing downloaded.
pub fn thumbnail_pick(
    streams: &[(crate::echo360::SourceNum, PathBuf)],
    duration: u32,
) -> Option<(PathBuf, u32)> {
    let (_, video) = streams.iter().min_by_key(|(n, _)| *n)?;
    Some((video.clone(), duration / 4))
}

/// Which second to actually grab `boundary`'s frame from. Earliest-good rather
/// than best, so a sparse title slide is not passed over for a busier one.
fn best_offset(ffmpeg: &Path, video: &Path, boundary: u32) -> u32 {
    let probed: Vec<(u32, f32)> = GRAB_OFFSETS
        .iter()
        .filter_map(|off| {
            let at = boundary + off;
            frame_detail(ffmpeg, video, at).map(|detail| (at, detail))
        })
        .collect();
    pick_offset(&probed).unwrap_or(boundary)
}

/// The choosing half of [`best_offset`]: the first probe (in preference order)
/// within [`GRAB_TOLERANCE`] of the most detailed one.
fn pick_offset(probed: &[(u32, f32)]) -> Option<u32> {
    let best = probed
        .iter()
        .map(|(_, detail)| *detail)
        .fold(f32::NEG_INFINITY, f32::max);
    probed
        .iter()
        .find(|(_, detail)| *detail >= best * GRAB_TOLERANCE)
        .map(|(at, _)| *at)
}

/// How much is going on in the frame at `second`: the standard deviation of
/// its grey values at 160×90. A blank scores ~0. `None` means no frame
/// (normally past the end).
fn frame_detail(ffmpeg: &Path, video: &Path, second: u32) -> Option<f32> {
    let out = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-ss", &second.to_string(), "-i"])
        .arg(video)
        .args([
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={FRAME_W}:{FRAME_H},format=gray"),
            "-f",
            "rawvideo",
            "-",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.len() < FRAME_BYTES {
        return None;
    }
    Some(spread(&out.stdout[..FRAME_BYTES]))
}

/// Population standard deviation of a frame's grey values.
fn spread(frame: &[u8]) -> f32 {
    let n = frame.len() as f32;
    let mean = frame.iter().map(|b| f32::from(*b)).sum::<f32>() / n;
    let variance = frame
        .iter()
        .map(|b| {
            let d = f32::from(*b) - mean;
            d * d
        })
        .sum::<f32>()
        / n;
    variance.sqrt()
}

// ── Naming them: the agent job ───────────────────────────────────────────────
//
// A coding agent is handed paths (outline, frames) and reads what it needs.
// It never touches the database: it replies with JSON, which Rust parses,
// validates and writes — chapters are derived data, so there is no write door.

/// One named span of a recording. It ends where the next begins (the last at
/// the lecture's duration), so no end is stored.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Chapter {
    pub start_seconds: u32,
    pub title: String,
    pub summary: String,
}

/// What the agent replies with, before validation. The aliases accept the
/// synonyms models write despite the prompt.
#[derive(serde::Deserialize)]
struct ReplyChapter {
    #[serde(alias = "start_seconds", alias = "seconds", alias = "at")]
    start: f64,
    title: String,
    #[serde(default)]
    summary: String,
}

/// A reply that wrapped the array in an object.
#[derive(serde::Deserialize)]
struct ReplyEnvelope {
    chapters: Vec<ReplyChapter>,
}

/// More than this and it is a slide list, not a shape.
pub const MAX_CHAPTERS: usize = 12;

/// What the chaptering prompt needs; the agent reads the rest off disk.
pub struct Job<'a> {
    pub title: &'a str,
    pub duration_secs: u32,
    /// Relative to the agent's working directory (`agents/`).
    pub lecture_dir: &'a str,
    /// The subject's course folder, same relative shape. An Echo360 title is a
    /// room booking, so this is how the agent finds the deck.
    pub course_dir: Option<&'a str>,
    /// How many slide changes the outline marks.
    pub detected: usize,
    /// Whether the outline has any transcript in it.
    pub has_transcript: bool,
}

/// The transcript and the detected slide changes merged into one document in
/// play order, written to `<lecture dir>/outline.md` for the agent to read.
///
/// Every line is `second  timestamp  text`: the bare second is what a chapter's
/// `start` must be exactly, because a model converting a clock will round.
pub fn outline(title: &str, cues: &[TranscriptCue], changes: &[Candidate]) -> String {
    fn line(out: &mut String, second: u32, text: &str) {
        out.push_str(&format!("{second:>7}  {}  {text}\n", hms(second)));
    }
    fn marker(change: &Candidate) -> String {
        format!(
            "--- slide change · score {:.1}{} ---",
            change.score,
            if change.pause { " · pause" } else { "" }
        )
    }

    let mut out = format!(
        "# {title}\n\nEvery line is `second  timestamp  text`: a transcript cue, or a marker for a second where the slide changed. The two columns are the same moment; the first one is what a chapter start has to be.\n\n"
    );
    let mut marks = changes.iter().peekable();
    for cue in cues {
        let at = cue.start.max(0.0) as u32;
        while marks.peek().is_some_and(|m| m.seconds <= at) {
            let change = marks.next().expect("peeked");
            line(&mut out, change.seconds, &marker(change));
        }
        line(&mut out, at, &cue.text);
    }
    // Markers after the last cue (or all of them, with no transcript).
    for change in marks {
        line(&mut out, change.seconds, &marker(change));
    }
    out
}

/// The prompt for one chaptering turn. Three lines in it are load-bearing: a
/// slide change is not a chapter, the AV splash screen is not a slide, and the
/// `grep` that saves reading the whole outline.
pub fn prompt(job: &Job) -> String {
    format!(
        "Chapter a university lecture recording: choose its real topic boundaries and name \
each one.\n\n\
Lecture: {title}\n\
Duration: {clock} ({mins} minutes)\n\
Recording folder: {dir}\n  \
{dir}/outline.md           what was said and where the slides changed, merged, in play order\n  \
{dir}/frames/<second>.jpg  one grab per slide change, named by its second\n\
{course}\n\
The outline is the whole lecture as one document. Every line is `second  timestamp  text`: \
either a transcript cue, or a `--- slide change ---` marker for a second where the picture \
changed hard enough to be a new slide, carrying how hard it changed and whether the lecturer \
paused there. {detected} slide changes were found — \
`grep -n \"slide change\" {dir}/outline.md` lists them with the lines to read \
around.{silent}\n\n\
A slide change is a place you *may* cut, not a place you should: most of them are the same \
topic carrying on. And a topic can turn where no slide changed at all, so a boundary may be \
any line in the outline, marker or cue.\n\n\
How to work\n\
- Read the outline across a second you are considering and see whether the subject actually \
turns there. That is the evidence; the frames are corroboration. It is a long file — read \
the spans you want rather than all of it.\n\
- Open the frames you are unsure about. A title slide or a section divider usually starts a \
chapter; the next bullet of the same argument does not. View a handful as images — they are \
pictures of slides, so looking at them is the point; do not hash them or reach for an OCR \
tool.\n\
- Some frames are the lecture theatre's own AV splash screen — a \
room-control panel saying something like \"connect your laptop\" — and not a \
slide at all. They survive when the recording dropped out for a while. Ignore \
them completely and never name a chapter after one.\n\n\
Rules\n\
- Give the lecture as many chapters as it genuinely has: usually 5 to 8, never \
more than {max}. Do not pad a coherent fifteen-minute stretch into three \
chapters, and do not merge two genuinely different topics to keep the list \
short.\n\
- A chapter shorter than about three minutes is a slide, not a topic: fold it \
into whichever neighbour it belongs to. But do not let that swallow a real \
segment — a long stretch of housekeeping, a worked example or a Q&A is its own \
chapter if it lasts.\n\
- The first chapter starts at second 0.\n\
- Every other start must be exactly one of the seconds in the outline's first column. Do not \
round one, and do not pick a second between two lines.\n\
- Titles name the topic in the lecturer's own vocabulary, two to six words, \
sentence case: \"Grover's search\", \"Proving unsatisfiability by resolution\". \
Never \"Introduction\", \"Part 2\", \"Continued\", \"Wrap-up\", and never the \
lecture's own title.\n\
- Summaries are one or two sentences saying what is covered and why a student \
would come back to this span. Say something the title does not — a summary \
that restates its title is worth nothing.\n\n\
Reply with JSON and nothing else, in play order:\n\n\
[\n  {{\"start\": 0, \"title\": \"...\", \"summary\": \"...\"}},\n  \
{{\"start\": 742, \"title\": \"...\", \"summary\": \"...\"}}\n]\n",
        title = job.title,
        course = job
            .course_dir
            .map(|d| format!("  {d}/   the subject's own materials, including the slide deck\n"))
            .unwrap_or_default(),
        clock = hms(job.duration_secs),
        mins = job.duration_secs / 60,
        dir = job.lecture_dir,
        detected = job.detected,
        silent = if job.has_transcript {
            ""
        } else {
            " This recording has no transcript, so the outline is those markers and nothing else."
        },
        max = MAX_CHAPTERS,
    )
}

/// `HH:MM:SS`, as every recording prompt prints it beside the bare second.
pub fn hms(secs: u32) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

/// The chapter array out of whatever the agent said. Whether it is allowed is
/// [`validate`]'s question.
pub fn parse_chapters(reply: &str) -> Result<Vec<Chapter>, String> {
    parse_reply(reply, "chapter list", decode)
}

/// The first fragment of `reply` that `decode` accepts, trying the whole
/// reply, each fenced block, then balanced `[…]`/`{…}` runs — models wrap JSON
/// in prose, fences or objects however plainly asked. `what` names the
/// expected shape in the error. Shared with `lecture_end`.
pub(crate) fn parse_reply<T>(
    reply: &str,
    what: &str,
    decode: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    for candidate in json_candidates(reply) {
        if let Some(parsed) = decode(&candidate) {
            return Ok(parsed);
        }
    }
    Err(format!(
        "no {what} in the reply ({} chars): {}",
        reply.chars().count(),
        clip(reply.trim(), 200)
    ))
}

fn decode(text: &str) -> Option<Vec<Chapter>> {
    let items: Vec<ReplyChapter> = serde_json::from_str(text)
        .or_else(|_| serde_json::from_str::<ReplyEnvelope>(text).map(|e| e.chapters))
        .ok()?;
    if items.is_empty() {
        return None;
    }
    Some(
        items
            .into_iter()
            .map(|c| Chapter {
                start_seconds: c.start.max(0.0).round() as u32,
                title: c.title.trim().to_string(),
                summary: c.summary.trim().to_string(),
            })
            .collect(),
    )
}

/// Fragments of `reply` worth trying to parse, most likely first.
fn json_candidates(reply: &str) -> Vec<String> {
    let mut out = vec![reply.trim().to_string()];
    // Fenced blocks; the opening fence's info string is dropped with its line.
    let mut rest = reply;
    while let Some(open) = rest.find("```") {
        let after = &rest[open + 3..];
        let body = match after.find('\n') {
            Some(nl) => &after[nl + 1..],
            None => break,
        };
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

/// Up to eight balanced `open`…`close` spans of `text`, in order — several,
/// because prose can hold brackets of its own. String literals are respected.
fn balanced_runs(text: &str, open: char) -> Vec<String> {
    const BALANCED_RUNS: usize = 8;
    let close = if open == '[' { ']' } else { '}' };
    let mut out: Vec<String> = Vec::new();
    let mut from = 0usize;
    while out.len() < BALANCED_RUNS {
        let Some(offset) = text[from..].find(open) else {
            break;
        };
        let start = from + offset;
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;
        let mut end: Option<usize> = None;
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
            Some(e) => {
                out.push(text[start..e].to_string());
                from = e;
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
        head.to_string()
    }
}

/// Whether a parsed chapter set may be written. One bad chapter rejects the
/// whole set — dropping one would let its neighbour silently swallow its span.
///
/// `boundaries` is every second the outline printed (0, the slide changes and
/// every cue start): wide enough to chapter from speech alone, but closed, so
/// the agent cannot invent a timestamp.
pub fn validate(
    chapters: &[Chapter],
    boundaries: &[u32],
    duration_secs: u32,
) -> Result<(), String> {
    if chapters.is_empty() {
        return Err("no chapters in the reply".to_string());
    }
    if chapters.len() > MAX_CHAPTERS {
        return Err(format!(
            "{} chapters is more than the {MAX_CHAPTERS} allowed — that is a slide list, not a shape",
            chapters.len()
        ));
    }
    if chapters[0].start_seconds != 0 {
        return Err(format!(
            "chapter 1 ({:?}): the first chapter must start at 0, not {}",
            chapters[0].title, chapters[0].start_seconds
        ));
    }
    let mut previous: Option<u32> = None;
    for (n, chapter) in chapters.iter().enumerate() {
        let where_ = format!("chapter {} ({:?}): ", n + 1, chapter.title);
        if chapter.title.is_empty() {
            return Err(format!("chapter {}: a chapter needs a title", n + 1));
        }
        if chapter.summary.is_empty() {
            return Err(format!("{where_}a chapter needs a summary"));
        }
        if !boundaries.contains(&chapter.start_seconds) {
            return Err(format!(
                "{where_}{} is not one of the seconds in the outline",
                chapter.start_seconds
            ));
        }
        if duration_secs > 0 && chapter.start_seconds >= duration_secs {
            return Err(format!(
                "{where_}starts at {}, past the end of a {duration_secs}s recording",
                chapter.start_seconds
            ));
        }
        if let Some(p) = previous {
            if chapter.start_seconds <= p {
                return Err(format!(
                    "{where_}starts at {}, which is not after the chapter before it ({p})",
                    chapter.start_seconds
                ));
            }
        }
        previous = Some(chapter.start_seconds);
    }
    Ok(())
}

// ── Running the whole job ────────────────────────────────────────────────────
//
// One implementation for the CLI and the app; they differ only in where the
// agent selection comes from and how progress is shown.

pub use crate::lecture_jobs::Run;

/// The pipeline's own phases, for a caller that draws progress; what happens
/// inside the agent turn arrives on `on_event` instead.
pub enum Step<'a> {
    /// Fires often; [`run`] throttles it.
    Decoding {
        second: u32,
        duration: u32,
    },
    Detected {
        title: &'a str,
        duration: u32,
        candidates: usize,
    },
    Grabbing {
        done: usize,
        total: usize,
    },
    Asking,
    Writing,
}

pub struct Outcome {
    pub title: String,
    pub duration_seconds: u32,
    /// Second 0 not counted.
    pub candidates: usize,
    /// A 2 here is the only visible sign that source 1 was dead.
    pub source: crate::echo360::SourceNum,
    pub chapters: Vec<Chapter>,
}

/// Detect, grab, ask, validate, write. Blocking and minutes long, so callers
/// run it off any thread that must stay responsive.
///
/// `chapter_status` is `running` from the start of the work, then `error` with
/// the message or `ready` (stamped by `store::save_chapters`). The guards
/// before it fail without claiming, so a refusal leaves no status behind.
pub fn run(
    rt: &tokio::runtime::Handle,
    pool: &sqlx::SqlitePool,
    job: &Run,
    on_step: impl Fn(Step),
    on_event: impl Fn(&crate::harness::HarnessEvent) + Send + Sync + 'static,
) -> Result<Outcome, String> {
    let id = job.lecture_id;
    let source = rt.block_on(crate::lecture_jobs::source(pool, id))?;
    let title = &source.title;
    let duration = source.duration;
    let transcript = &source.transcript;
    let code = &source.code;

    let existing = rt.block_on(crate::store::chapters(pool, id))?;
    if !existing.is_empty() && !job.force {
        return Err(format!(
            "{title} already has {} chapter(s) — re-running replaces them",
            existing.len()
        ));
    }
    let video = source.video()?;
    let ffmpeg = crate::echo360::find_ffmpeg(None)
        .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

    // Claimed before the decode, so the UI shows the run immediately.
    rt.block_on(crate::store::set_chapter_status(
        pool,
        id,
        Some("running"),
        None,
    ))?;

    let outcome = (|| -> Result<Outcome, String> {
        // A missing transcript is not fatal: chaptering works off the picture.
        let vtt = transcript
            .as_deref()
            .and_then(|p| std::fs::read_to_string(p).ok());
        let gaps = vtt.as_deref().map(cue_gaps).unwrap_or_default();
        let cues = vtt.as_deref().map(parse_transcript).unwrap_or_default();

        let dir = crate::echo360::lecture_dir(job.data_dir, id);
        let mut last = std::time::Instant::now();
        let detected = detect(
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
        let found = detected.candidates;
        if found.is_empty() {
            return Err(format!(
                "no boundary candidates in {title} — nothing to chapter"
            ));
        }
        on_step(Step::Detected {
            title: &title,
            duration,
            candidates: found.len(),
        });

        // Second 0 is never detected but is always a boundary.
        let mut frames_at: Vec<u32> = vec![0];
        frames_at.extend(found.iter().map(|c| c.seconds));

        // Frames only at slide changes, but any cue start is a boundary too.
        let mut boundaries = frames_at.clone();
        boundaries.extend(cues.iter().map(|c| c.start.max(0.0) as u32));
        boundaries.sort_unstable();
        boundaries.dedup();

        std::fs::write(dir.join("outline.md"), outline(&title, &cues, &found))
            .map_err(|e| format!("{}: {e}", dir.join("outline.md").display()))?;

        let total = frames_at.len();
        extract_frames(
            &ffmpeg,
            &detected.video,
            &frames_at,
            &dir.join("frames"),
            |done| on_step(Step::Grabbing { done, total }),
        )?;

        let course_dir = code
            .as_deref()
            .map(|c| format!("../courses/{}", crate::paths::safe_dir(c)));

        let text = prompt(&Job {
            title: &title,
            duration_secs: duration,
            lecture_dir: &format!("../lectures/{id}"),
            course_dir: course_dir.as_deref(),
            detected: found.len(),
            has_transcript: !cues.is_empty(),
        });

        on_step(Step::Asking);
        let chapters = crate::lecture_jobs::reply(job.data_dir, job.selection, &text, on_event)
            .and_then(|reply| parse_chapters(&reply))
            .and_then(|chapters| validate(&chapters, &boundaries, duration).map(|()| chapters))?;
        on_step(Step::Writing);
        rt.block_on(crate::store::save_chapters(pool, id, &chapters))?;
        Ok(Outcome {
            title: title.clone(),
            duration_seconds: duration,
            candidates: found.len(),
            source: detected.source,
            chapters,
        })
    })();

    if let Err(e) = &outcome {
        rt.block_on(crate::store::set_chapter_status(
            pool,
            id,
            Some("error"),
            Some(e),
        ))?;
    }
    outcome
}

// ── Tauri ────────────────────────────────────────────────────────────────────

pub mod app {
    use super::*;
    use crate::lecture_jobs::{check_start, reconcile_status, spawn_job, Progress};
    use tauri::{AppHandle, Emitter};

    /// Emitted once when a run ends. Not `lectures-changed`, which fires on
    /// every playback-progress save.
    pub const LECTURE_CHAPTERS_EVENT: &str = "lecture-chapters";

    /// Emitted throughout a run; display only, never persisted.
    pub const LECTURE_CHAPTER_PROGRESS_EVENT: &str = "lecture-chapter-progress";

    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Finished {
        lecture_id: String,
        /// How this request ended. A refusal reports `error` while the row
        /// keeps its `ready`.
        status: &'static str,
        chapters: usize,
        error: Option<String>,
    }

    /// Chapter a lecture with the agent the `lectureChapters` job is
    /// configured with. Returns once the run is claimed; the end arrives as
    /// [`LECTURE_CHAPTERS_EVENT`].
    #[tauri::command]
    pub async fn lecture_find_chapters(
        app: AppHandle,
        lecture_id: String,
        force: Option<bool>,
        source: Option<u8>,
    ) -> Result<(), String> {
        check_start(
            &lecture_id,
            source,
            "chapter_status",
            "that lecture is already being chaptered",
        )
        .await?;

        let data_dir = crate::paths::data_dir();
        let force = force.unwrap_or(false);
        spawn_job(
            "chapters",
            crate::harness::jobs::Job::LectureChapters,
            move |rt, pool, selection| {
                let emit = {
                    let app = app.clone();
                    move |p: Progress| {
                        app.emit(LECTURE_CHAPTER_PROGRESS_EVENT, p).ok();
                    }
                };

                let step = {
                    let id = lecture_id.clone();
                    let emit = emit.clone();
                    move |s: Step| {
                        let p = match s {
                            Step::Decoding { second, duration } => Progress {
                                done: Some(second),
                                total: (duration > 0).then_some(duration),
                                ..Progress::at(&id, "decoding")
                            },
                            Step::Detected {
                                title, candidates, ..
                            } => {
                                eprintln!("[oculus] chapters: {title} — {candidates} candidate(s)");
                                Progress {
                                    done: Some(0),
                                    total: Some(candidates as u32 + 1),
                                    ..Progress::at(&id, "frames")
                                }
                            }
                            Step::Grabbing { done, total } => Progress {
                                done: Some(done as u32),
                                total: Some(total as u32),
                                ..Progress::at(&id, "frames")
                            },
                            Step::Asking => Progress::at(&id, "agent"),
                            Step::Writing => Progress::at(&id, "writing"),
                        };
                        emit(p);
                    }
                };

                // The reply is the chapter JSON, so its first delta is the agent
                // having decided: report that once as `naming`.
                let naming = std::sync::atomic::AtomicBool::new(false);
                let event = {
                    let id = lecture_id.clone();
                    move |ev: &crate::harness::HarnessEvent| {
                        use crate::harness::HarnessEvent as E;
                        let p = match ev {
                            E::ToolStarted { kind, title, .. } => Progress {
                                detail: Some(title.clone()),
                                kind: Some(*kind),
                                ..Progress::at(&id, "agent")
                            },
                            E::AssistantDelta { .. } | E::AssistantMessage { .. } => {
                                if naming.swap(true, std::sync::atomic::Ordering::Relaxed) {
                                    return;
                                }
                                Progress::at(&id, "naming")
                            }
                            _ => return,
                        };
                        emit(p);
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
                let finished = match &outcome {
                    Ok(o) => Finished {
                        lecture_id: lecture_id.clone(),
                        status: "ready",
                        chapters: o.chapters.len(),
                        error: None,
                    },
                    Err(e) => {
                        eprintln!("[oculus] chapters: {e}");
                        Finished {
                            lecture_id: lecture_id.clone(),
                            status: "error",
                            chapters: 0,
                            error: Some(e.clone()),
                        }
                    }
                };
                app.emit(LECTURE_CHAPTERS_EVENT, finished).ok();
            },
        );
        Ok(())
    }

    /// A lecture's downloaded streams, source 1 first, with what the frame
    /// commands name and size them by.
    struct Streams {
        title: String,
        /// Echo360's catalogue length.
        duration: u32,
        dir: PathBuf,
        sources: Vec<(crate::echo360::SourceNum, PathBuf)>,
    }

    async fn downloaded_streams(lecture_id: &str) -> Result<Streams, String> {
        let pool = crate::store::open_pool().await?;
        let row: Option<(String, i64, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT title, duration_seconds, video_path, video2_path FROM lectures WHERE id = ?1",
        )
        .bind(lecture_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| e.to_string())?;
        let (title, duration, first, second) =
            row.ok_or_else(|| format!("no lecture {lecture_id}"))?;

        let dir = crate::echo360::lecture_dir(&crate::paths::data_dir(), lecture_id);
        // The column, else the stream's own path on disk (as `detect` does).
        let sources = [(1, first), (2, second)]
            .into_iter()
            .map(|(n, column)| {
                let path = column
                    .map(PathBuf::from)
                    .unwrap_or_else(|| crate::echo360::source_path(&dir, n));
                (n, path)
            })
            .filter(|(_, path)| path.exists())
            .collect();
        Ok(Streams {
            title,
            duration: u32::try_from(duration).unwrap_or(0),
            dir,
            sources,
        })
    }

    /// The Up Next card's thumbnail (`docs/viewers.md`): one frame a quarter
    /// of the way in ([`thumbnail_pick`]), probed like a chapter's, cached at
    /// `lectures/<id>/thumb.jpg`. Returns its path for the asset protocol, or
    /// `None` while the lecture is not downloaded.
    #[tauri::command]
    pub async fn lecture_thumbnail(lecture_id: String) -> Result<Option<String>, String> {
        let Streams {
            dir,
            duration,
            sources,
            ..
        } = downloaded_streams(&lecture_id).await?;
        let Some((video, second)) = thumbnail_pick(&sources, duration) else {
            return Ok(None);
        };
        let out = dir.join("thumb.jpg");
        if out.exists() {
            return Ok(Some(out.to_string_lossy().into_owned()));
        }
        let ffmpeg = crate::echo360::find_ffmpeg(None)
            .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;
        crate::blocking::run(move || {
            // Written aside and renamed, so a card never loads half a JPEG.
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let part = dir.join("thumb.part.jpg");
            grab_frame(&ffmpeg, &video, second, THUMB_WIDTH, &part)?;
            std::fs::rename(&part, &out).map_err(|e| format!("{}: {e}", out.display()))?;
            Ok(Some(out.to_string_lossy().into_owned()))
        })
        .await
    }

    /// One stream's frame of the moment a dock message carries.
    #[derive(serde::Serialize, Clone)]
    pub struct MomentFrame {
        pub source: crate::echo360::SourceNum,
        /// Relative to `agents/`, the thread's cwd.
        pub path: String,
    }

    /// One JPEG per downloaded stream of the playhead's moment, for a message
    /// sent from the lecture player's dock (`docs/harness.md`).
    ///
    /// Every source is grabbed, not the one on screen: either stream may be the
    /// one with the teaching on it. Returns paths the agent reads
    /// (`../lectures/<id>/frames/live/<seconds>-source<n>.jpg`), in `live/` so
    /// they never collide with a chaptering run's. A stream that fails to decode
    /// is skipped; only no frame at all is an error.
    #[tauri::command]
    pub async fn lecture_grab_frames(
        lecture_id: String,
        seconds: u32,
    ) -> Result<Vec<MomentFrame>, String> {
        let Streams {
            title,
            dir,
            sources,
            ..
        } = downloaded_streams(&lecture_id).await?;
        if sources.is_empty() {
            return Err(format!(
                "{title} is not downloaded — `oculus run -l --videos` fetches it"
            ));
        }
        let ffmpeg = crate::echo360::find_ffmpeg(None)
            .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

        let out_dir = dir.join("frames").join("live");
        std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
        let grabbed: Vec<crate::echo360::SourceNum> = tokio::task::spawn_blocking(move || {
            sources
                .into_iter()
                .filter_map(|(n, video)| {
                    let out = out_dir.join(format!("{seconds}-source{n}.jpg"));
                    match grab_frame(&ffmpeg, &video, seconds, LIVE_GRAB_WIDTH, &out) {
                        Ok(()) => Some(n),
                        Err(e) => {
                            eprintln!("[oculus] frame: source {n} at {seconds}s: {e}");
                            None
                        }
                    }
                })
                .collect()
        })
        .await
        .map_err(|e| e.to_string())?;
        if grabbed.is_empty() {
            return Err(format!("no frame of {title} at {seconds}s"));
        }
        Ok(grabbed
            .into_iter()
            .map(|source| MomentFrame {
                source,
                path: format!("../lectures/{lecture_id}/frames/live/{seconds}-source{source}.jpg"),
            })
            .collect())
    }

    /// Startup: clear `running` left by a killed run.
    pub fn reconcile(_app: &AppHandle) {
        reconcile_status("chapters", |pool| async move {
            crate::store::reconcile_chapter_status(&pool).await
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame of one flat grey value — a held slide, in miniature.
    fn flat(value: u8) -> Vec<u8> {
        vec![value; FRAME_BYTES]
    }

    /// Feed a sequence of frames through the same read-and-diff loop
    /// `sample_diffs` runs, without an ffmpeg between.
    fn diffs_of(frames: &[Vec<u8>]) -> Vec<(u32, f32)> {
        let bytes: Vec<u8> = frames.concat();
        let mut source = std::io::Cursor::new(bytes);
        let mut previous = vec![0u8; FRAME_BYTES];
        let mut current = vec![0u8; FRAME_BYTES];
        let mut out = Vec::new();
        let mut index = 0u32;
        while read_frame(&mut source, &mut current).unwrap() {
            if index > 0 {
                out.push((index, mean_abs_diff(&previous, &current)));
            }
            std::mem::swap(&mut previous, &mut current);
            index += 1;
        }
        out
    }

    #[test]
    fn a_held_slide_is_silent_and_a_cut_is_loud() {
        let frames = vec![flat(40), flat(40), flat(41), flat(200), flat(200)];
        let diffs = diffs_of(&frames);
        assert_eq!(diffs.len(), 4, "one diff per frame after the first");
        assert_eq!(diffs[0], (1, 0.0));
        assert_eq!(diffs[1], (2, 1.0), "a one-level drift is not a change");
        assert_eq!(diffs[2], (3, 159.0), "the cut");
        assert_eq!(diffs[3], (4, 0.0));

        let found = candidates(&diffs, &[], 10);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].seconds, 3);
        assert!(!found[0].pause);
    }

    #[test]
    fn a_trailing_partial_frame_is_dropped() {
        let mut bytes = flat(10);
        bytes.extend(flat(200));
        bytes.extend(vec![0u8; 17]); // ffmpeg cut off mid-write
        let mut source = std::io::Cursor::new(bytes);
        let mut frame = vec![0u8; FRAME_BYTES];
        assert!(read_frame(&mut source, &mut frame).unwrap());
        assert!(read_frame(&mut source, &mut frame).unwrap());
        assert!(!read_frame(&mut source, &mut frame).unwrap());
    }

    #[test]
    fn a_run_of_loud_frames_collapses_onto_its_first() {
        // A dissolve: four consecutive loud frames, peaking in the middle.
        let diffs = vec![
            (100, 20.0),
            (101, 60.0),
            (102, 30.0),
            (103, 8.0),
            // Well clear of the run, and of MIN_SPACING.
            (400, 25.0),
        ];
        let found = candidates(&diffs, &[], 600);
        assert_eq!(
            found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
            vec![100, 400]
        );
        assert_eq!(found[0].diff, 60.0, "the run keeps its peak magnitude");
    }

    #[test]
    fn frames_below_the_threshold_never_become_candidates() {
        let diffs = vec![(10, 0.008), (200, 5.9), (400, 6.1)];
        let found = candidates(&diffs, &[], 600);
        assert_eq!(
            found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
            vec![400]
        );
    }

    #[test]
    fn thinning_keeps_the_strongest_of_a_cluster() {
        // Three changes inside 90 s, the middle one strongest, plus one far
        // enough away to survive on its own.
        let diffs = vec![(30, 10.0), (60, 90.0), (100, 40.0), (300, 12.0)];
        let found = candidates(&diffs, &[], 600);
        assert_eq!(
            found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
            vec![60, 300],
            "greedy strongest-first, then back into play order"
        );
    }

    #[test]
    fn thinning_measures_from_what_it_kept_not_from_the_last_candidate() {
        // 0 is strongest and keeps 80 out; 150 is 150 s from 0 and stays.
        let diffs = vec![(10, 99.0), (80, 50.0), (160, 40.0)];
        let found = candidates(&diffs, &[], 600);
        assert_eq!(
            found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
            vec![10, 160]
        );
    }

    #[test]
    fn a_nearby_pause_is_a_bonus_and_never_a_gate() {
        // Two equal changes; only the second has a silence beside it.
        let diffs = vec![(100, 20.0), (300, 20.0)];
        let gaps = vec![(60, 0.4), (295, 3.2), (400, 0.1)];
        let found = candidates(&diffs, &gaps, 600);
        assert_eq!(found.len(), 2, "the unsupported change survives");
        assert!(!found[0].pause);
        assert!(found[1].pause);
        assert_eq!(found[1].score, 23.0);
        assert_eq!(
            found[1].diff, 20.0,
            "the bonus does not touch the magnitude"
        );

        // A pause on its own is not a boundary.
        assert!(candidates(&[], &gaps, 600).is_empty());
    }

    #[test]
    fn a_pause_outside_the_window_does_not_count() {
        let diffs = vec![(300, 20.0)];
        assert!(!candidates(&diffs, &[(291, 5.0)], 600)[0].pause);
        assert!(candidates(&diffs, &[(292, 5.0)], 600)[0].pause);
        assert!(candidates(&diffs, &[(308, 5.0)], 600)[0].pause);
        assert!(!candidates(&diffs, &[(309, 5.0)], 600)[0].pause);
    }

    #[test]
    fn candidates_past_the_end_are_dropped() {
        let diffs = vec![(100, 30.0), (2519, 30.0)];
        let found = candidates(&diffs, &[], 2519);
        assert_eq!(
            found.iter().map(|c| c.seconds).collect::<Vec<_>>(),
            vec![100]
        );
    }

    #[test]
    fn spread_separates_a_blank_frame_from_a_busy_one() {
        assert_eq!(spread(&flat(0)), 0.0, "a black frame has no spread at all");
        assert_eq!(spread(&flat(255)), 0.0, "and neither does a white one");
        // Half black, half white — the letterboxed slide these captures are.
        let mut split = flat(0);
        split[FRAME_BYTES / 2..].fill(255);
        assert!((spread(&split) - 127.5).abs() < 0.01);
    }

    #[test]
    fn a_frame_grab_steps_over_a_blank_and_over_a_splash_screen() {
        // The cut is black, +2 and +6 are the AV splash, the slide is back by
        // +12. Probes arrive in GRAB_OFFSETS order: 2, 6, 12, 0.
        let probed = [(1388, 81.4), (1392, 81.4), (1398, 102.2), (1386, 0.0)];
        assert_eq!(pick_offset(&probed), Some(1398));

        // +2 is the splash; the first frame within tolerance of the best wins.
        let probed = [(1726, 81.4), (1730, 102.5), (1736, 102.5), (1724, 102.6)];
        assert_eq!(pick_offset(&probed), Some(1730));
    }

    #[test]
    fn a_run_sweeps_the_grabs_it_will_not_rewrite() {
        let dir = crate::test_support::Scratch::new("sweep");
        std::fs::create_dir_all(dir.join("live")).unwrap();
        for name in ["50.jpg", "313.jpg", "1767.jpg", "notes.txt", "keyframe.jpg"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        std::fs::write(dir.join("live").join("900.jpg"), b"x").unwrap();

        sweep_orphans(&dir, &[313, 1767, 2550]);

        let left = |name: &str| dir.join(name).exists();
        assert!(
            !left("50.jpg"),
            "an orphan from a previous candidate set goes"
        );
        assert!(
            left("313.jpg") && left("1767.jpg"),
            "a frame this run rewrites stays"
        );
        assert!(left("notes.txt"), "only JPEGs are swept");
        assert!(
            left("keyframe.jpg"),
            "a name that is not a second was not written here"
        );
        assert!(left("live/900.jpg"), "another job's subfolder is untouched");
    }

    #[test]
    fn a_frame_grab_prefers_the_earliest_good_frame() {
        let probed = [(114, 102.9), (118, 102.9), (124, 102.9), (112, 102.9)];
        assert_eq!(pick_offset(&probed), Some(114));

        // A sparse title slide is not passed over for a denser later one.
        let probed = [(22, 98.0), (26, 99.0), (32, 101.0), (20, 98.5)];
        assert_eq!(pick_offset(&probed), Some(22));
    }

    #[test]
    fn a_frame_grab_with_nothing_to_go_on_still_picks_something() {
        // Every probe blank: the first offset still wins.
        let probed = [(1388, 0.0), (1392, 0.0), (1398, 0.0), (1386, 0.0)];
        assert_eq!(pick_offset(&probed), Some(1388));
        assert_eq!(pick_offset(&[]), None);
    }

    // ── The agent's reply ────────────────────────────────────────────────────

    fn chapter(start: u32, title: &str) -> Chapter {
        Chapter {
            start_seconds: start,
            title: title.to_string(),
            summary: format!("What happens in {title}."),
        }
    }

    const REPLY: &str = r#"[
      {"start": 0, "title": "Qubits and superposition", "summary": "Sets up the state vector."},
      {"start": 742, "title": "Hadamard gates", "summary": "Builds the uniform superposition."}
    ]"#;

    #[test]
    fn a_bare_json_array_is_the_easy_case() {
        let parsed = parse_chapters(REPLY).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].start_seconds, 0);
        assert_eq!(parsed[1].title, "Hadamard gates");
    }

    #[test]
    fn a_reply_wrapped_in_prose_still_parses() {
        let reply = format!(
            "I read the transcript around each candidate. Here are the chapters:\n\n{REPLY}\n\n\
             Let me know if you would like them merged differently."
        );
        let parsed = parse_chapters(&reply).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].start_seconds, 742);
    }

    #[test]
    fn a_fenced_reply_still_parses() {
        let reply = format!("Done — six chapters.\n\n```json\n{REPLY}\n```\n");
        assert_eq!(parse_chapters(&reply).unwrap().len(), 2);
        // And an unlabelled fence, which is just as common.
        let reply = format!("```\n{REPLY}\n```");
        assert_eq!(parse_chapters(&reply).unwrap().len(), 2);
    }

    #[test]
    fn an_object_around_the_array_is_accepted() {
        let reply = format!("{{\"chapters\": {REPLY}}}");
        assert_eq!(
            parse_chapters(&reply).unwrap()[0].title,
            "Qubits and superposition"
        );
    }

    #[test]
    fn a_bracket_inside_a_summary_does_not_end_the_scan() {
        let reply = r#"Here you go:
        [{"start": 0, "title": "Resolution", "summary": "The rule [A ∨ B], [¬B ∨ C] ⊢ [A ∨ C]."}]
        That's it."#;
        let parsed = parse_chapters(reply).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].summary.ends_with("[A ∨ C]."));
    }

    #[test]
    fn a_reply_with_no_chapters_in_it_is_refused() {
        let error = parse_chapters("I could not open the frames, sorry.").unwrap_err();
        assert!(error.contains("no chapter list"), "{error}");
        // Valid JSON that is not a chapter list is no better.
        assert!(parse_chapters("[]").is_err());
        assert!(parse_chapters(r#"{"ok": true}"#).is_err());
        // A truncated reply: the array never closes.
        assert!(parse_chapters(r#"[{"start": 0, "title": "Qubits","#).is_err());
    }

    #[test]
    fn a_bracket_in_the_prose_does_not_hide_the_answer() {
        // An empty list, then a citation, then the real array.
        let reply = format!("I found no splash frames [] — see slide [3].\n\n{REPLY}");
        assert_eq!(parse_chapters(&reply).unwrap().len(), 2);
    }

    #[test]
    fn a_float_second_and_a_synonym_for_start_are_tolerated() {
        let reply = r#"[{"start_seconds": 0.0, "title": "Opening", "summary": "Sets up."}]"#;
        assert_eq!(parse_chapters(reply).unwrap()[0].start_seconds, 0);
    }

    // ── Whether a set may be written ─────────────────────────────────────────

    const BOUNDS: [u32; 5] = [0, 300, 700, 1200, 2000];

    #[test]
    fn a_well_formed_set_validates() {
        let set = vec![
            chapter(0, "Opening"),
            chapter(700, "Middle"),
            chapter(2000, "End"),
        ];
        assert!(validate(&set, &BOUNDS, 2400).is_ok());
    }

    #[test]
    fn a_boundary_the_outline_never_printed_rejects_the_whole_set() {
        let set = vec![
            chapter(0, "Opening"),
            chapter(701, "Middle"),
            chapter(2000, "End"),
        ];
        let error = validate(&set, &BOUNDS, 2400).unwrap_err();
        assert_eq!(
            error,
            "chapter 2 (\"Middle\"): 701 is not one of the seconds in the outline"
        );
    }

    #[test]
    fn a_transcript_cue_start_is_a_boundary_too() {
        let mut bounds = BOUNDS.to_vec();
        bounds.push(701);
        let set = vec![
            chapter(0, "Opening"),
            chapter(701, "Middle"),
            chapter(2000, "End"),
        ];
        assert!(validate(&set, &bounds, 2400).is_ok());
    }

    #[test]
    fn chapters_out_of_order_reject_the_whole_set() {
        let set = vec![
            chapter(0, "Opening"),
            chapter(1200, "Middle"),
            chapter(700, "End"),
        ];
        let error = validate(&set, &BOUNDS, 2400).unwrap_err();
        assert!(
            error.starts_with("chapter 3 (\"End\"): starts at 700, which is not after"),
            "{error}"
        );
        // A repeat would make a zero-length chapter.
        let set = vec![chapter(0, "Opening"), chapter(700, "A"), chapter(700, "B")];
        assert!(validate(&set, &BOUNDS, 2400).is_err());
    }

    #[test]
    fn a_first_chapter_that_does_not_start_at_zero_rejects_the_whole_set() {
        let set = vec![chapter(300, "Opening"), chapter(700, "Middle")];
        let error = validate(&set, &BOUNDS, 2400).unwrap_err();
        assert_eq!(
            error,
            "chapter 1 (\"Opening\"): the first chapter must start at 0, not 300"
        );
    }

    #[test]
    fn thirteen_chapters_reject_the_whole_set() {
        let bounds: Vec<u32> = (0..13).map(|n| n * 300).collect();
        let set: Vec<Chapter> = bounds.iter().map(|s| chapter(*s, "Topic")).collect();
        assert_eq!(set.len(), 13);
        let error = validate(&set, &bounds, 9000).unwrap_err();
        assert!(
            error.starts_with("13 chapters is more than the 12 allowed"),
            "{error}"
        );
        // Twelve is the ceiling, not one short of it.
        assert!(validate(&set[..12], &bounds, 9000).is_ok());
    }

    #[test]
    fn an_empty_or_gutted_set_is_refused() {
        assert!(validate(&[], &BOUNDS, 2400).is_err());
        let mut set = vec![chapter(0, "Opening")];
        set[0].summary.clear();
        assert!(validate(&set, &BOUNDS, 2400)
            .unwrap_err()
            .contains("needs a summary"));
        set[0].summary = "Sets up.".into();
        set[0].title.clear();
        assert!(validate(&set, &BOUNDS, 2400)
            .unwrap_err()
            .contains("needs a title"));
    }

    #[test]
    fn a_chapter_past_the_end_of_the_recording_is_refused() {
        let set = vec![chapter(0, "Opening"), chapter(2000, "End")];
        assert!(validate(&set, &BOUNDS, 2000)
            .unwrap_err()
            .contains("past the end"));
        assert!(validate(&set, &BOUNDS, 2001).is_ok());
    }

    #[test]
    fn the_prompt_names_the_outline_and_the_two_paths() {
        let job = Job {
            title: "Lecture 7: Grover",
            duration_secs: 2534,
            lecture_dir: "../lectures/abc-123",
            course_dir: Some("../courses/MULT20015_2026_SM2"),
            detected: 17,
            has_transcript: true,
        };
        let text = prompt(&job);
        assert!(text.contains("Lecture 7: Grover"));
        assert!(text.contains("00:42:14"), "the duration as a clock");
        assert!(
            text.contains("\n  ../lectures/abc-123/outline.md"),
            "indented under the folder"
        );
        assert!(
            text.contains("\n  ../lectures/abc-123/frames/<second>.jpg"),
            "indented under the folder"
        );
        assert!(text.contains("../courses/MULT20015_2026_SM2/"));
        assert!(text.contains("17 slide changes were found"));
        assert!(
            !text.contains("transcript.vtt"),
            "the outline replaced the raw VTT, it did not join it"
        );
        assert!(text.contains("never more than 12"));
        assert!(
            text.contains("connect your laptop"),
            "the AV splash warning"
        );
        assert!(!text.contains("no transcript"), "it has one");

        let silent = prompt(&Job {
            has_transcript: false,
            ..job
        });
        assert!(
            silent.contains("no transcript, so the outline is those markers"),
            "a lecture with no transcript should not be sent looking for words"
        );
    }

    #[test]
    fn the_outline_merges_the_slide_changes_into_the_transcript_in_play_order() {
        let cues = vec![
            TranscriptCue {
                start: 1.5,
                end: 4.0,
                text: "Good morning.".into(),
            },
            TranscriptCue {
                start: 725.0,
                end: 728.0,
                text: "An equal superposition.".into(),
            },
        ];
        let changes = vec![
            Candidate {
                seconds: 723,
                score: 42.1,
                diff: 39.1,
                pause: true,
            },
            // Past the last spoken word: it must still reach the file.
            Candidate {
                seconds: 2400,
                score: 12.0,
                diff: 12.0,
                pause: false,
            },
        ];
        let text = outline("Lecture 7: Grover", &cues, &changes);
        let lines: Vec<&str> = text.lines().filter(|l| l.contains("00:")).collect();
        assert_eq!(
            lines,
            vec![
                "      1  00:00:01  Good morning.",
                "    723  00:12:03  --- slide change · score 42.1 · pause ---",
                "    725  00:12:05  An equal superposition.",
                "   2400  00:40:00  --- slide change · score 12.0 ---",
            ]
        );
        assert!(text.starts_with("# Lecture 7: Grover"));
    }

    #[test]
    fn an_outline_with_no_transcript_is_still_the_slide_changes() {
        let changes = vec![Candidate {
            seconds: 30,
            score: 9.0,
            diff: 9.0,
            pause: false,
        }];
        let text = outline("Silent", &[], &changes);
        assert!(
            text.contains("     30  00:00:30  --- slide change · score 9.0 ---"),
            "{text}"
        );
    }

    #[test]
    fn cue_gaps_reads_both_timestamp_shapes() {
        let vtt = include_str!("../fixtures/chapters/sample.vtt");
        let gaps = cue_gaps(vtt);
        assert_eq!(
            gaps,
            vec![
                // First cue: the silence is measured from the start of the file.
                (0, 0.5),
                (4, 0.0),
                // MM:SS.mmm, and a real pause before it.
                (66, 3.0),
                (70, 0.5),
                // HH:MM:SS.mmm past the hour.
                (3675, 3603.0),
            ]
        );
    }

    #[test]
    fn cue_gaps_ignores_headers_notes_and_blank_blocks() {
        let vtt = include_str!("../fixtures/chapters/sample.vtt");
        assert_eq!(cue_gaps(vtt).len(), 5);
        assert!(cue_gaps("WEBVTT\n\nnot a cue at all\n").is_empty());
    }

    #[test]
    fn cue_gaps_survives_crlf_and_a_bad_end_time() {
        let vtt = "WEBVTT\r\n\r\n00:00.000 --> broken\r\nhello\r\n\r\n00:10.000 --> 00:12.000\r\nworld\r\n";
        // The broken end falls back to its own start.
        assert_eq!(cue_gaps(vtt), vec![(0, 0.0), (10, 10.0)]);
    }

    #[test]
    fn thumbnail_takes_source_one_a_quarter_in() {
        let one = PathBuf::from("/l/source1.mp4");
        let two = PathBuf::from("/l/source2.mp4");
        let both = [(2, two.clone()), (1, one.clone())];
        assert_eq!(thumbnail_pick(&both, 4000), Some((one, 1000)));
        // Only the camera is on disk: it stands in.
        assert_eq!(thumbnail_pick(&[(2, two.clone())], 3001), Some((two, 750)));
        assert_eq!(thumbnail_pick(&[], 4000), None);
    }
}
