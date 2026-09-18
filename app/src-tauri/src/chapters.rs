//! Lecture chapters: where a recording changes topic.
//!
//! A two-hour Echo360 recording arrives as one unbroken timeline with a
//! transcript beside it and no visible shape. This module finds the moments
//! worth cutting at — the *candidates* — so that a later stage can name them.
//! It decides nothing about titles, summaries or storage; it produces a list of
//! seconds and how confident each one is.
//!
//! **The picture is the signal, not the words.** The stream this reads is 720p
//! screen capture of a slide deck: no camera, no grain, no lighting drift. A
//! held slide is *dead still* — frame-to-frame mean absolute difference sits at
//! p50 ≈ 0.008 — and a slide change is a cliff (p99 ≈ 13, max ≈ 175). That
//! bimodality is why one number and no tuning is enough: measured on a
//! 42-minute lecture, a threshold of 2 and a threshold of 6 produce boundary
//! sets whose first twelve entries are *identical*, and 19 vs 18 boundaries
//! overall. Only at 12 does the detector start dropping real changes. The
//! threshold barely matters, so there is no knob for it.
//!
//! **That holds for a slide capture and for nothing else.** An Echo360 capture
//! publishes up to two streams, and the argument above is about the one
//! pointed at the projector. A room camera never holds still: measured across
//! five lectures it sits at p50 ≈ 1.4 with a maximum of 20, against the slide
//! capture's p50 ≈ 0.01 and maxima past 175. No empty middle, so no threshold
//! to put in it; no cliff, so nothing to find. A camera is outside this
//! detector's design rather than a harder case for it, and [`detect`] answers
//! that by choosing the right stream rather than by tuning.
//!
//! **Transcript pauses are a tiebreak, never a gate.** Only a quarter to a
//! third of slide changes have a ≥2 s silence anywhere near them, so requiring
//! one would throw away most of the real boundaries. A nearby pause adds a
//! small amount to a candidate's score, which changes what survives thinning
//! and nothing else.
//!
//! **Nothing is cached.** One `fps=1` decode pass over a 42-minute lecture
//! costs ~4.9 s wall (it saturates every core; decode dominates, so the sample
//! rate and the 160×90 frame size are effectively free), and a two-hour
//! recording ~15 s. Re-detecting is cheaper than inventing a table to
//! invalidate, so there is no candidates table and no persisted candidate set.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// 160×90, one byte per pixel — the frame geometry the ffmpeg command below
/// asks for, and therefore exactly how many bytes one frame occupies on the
/// pipe.
const FRAME_W: usize = 160;
const FRAME_H: usize = 90;
const FRAME_BYTES: usize = FRAME_W * FRAME_H;

/// Mean absolute difference above which a frame pair counts as a change. Sits
/// in the empty middle of a violently bimodal distribution; see the module
/// note on why it is a constant and not a setting.
const DIFF_THRESHOLD: f32 = 6.0;

/// A dissolve, a build, or a scroll shows up as several consecutive loud
/// frames. Loud frames this close together are one event, reported at its
/// first frame — the moment the change *started* is the moment to cut at.
const COLLAPSE_SECS: u32 = 3;

/// A silence at least this long counts as the lecturer taking a breath between
/// topics.
const PAUSE_SECS: f32 = 2.0;

/// How far from a change-point a pause may sit and still be about it.
const PAUSE_WINDOW: u32 = 8;

/// Added to a candidate's score when a pause supports it. Deliberately small
/// against a magnitude scale that runs to ~175: it reorders near-equals during
/// thinning and can never promote a quiet frame into a boundary.
const PAUSE_BONUS: f32 = 3.0;

/// No two boundaries closer than this. A chapter shorter than a minute and a
/// half is a slide, not a topic.
const MIN_SPACING: u32 = 90;

/// At most this many candidates in a whole recording means the stream is dead,
/// not that the lecture was quiet.
///
/// Sits in an empty middle, exactly as [`DIFF_THRESHOLD`] does: the lecture
/// whose projector capture failed gives *one* candidate across 44 minutes, and
/// a healthy capture gives 16 to 22. Nothing has been observed in between, so
/// this is a which-file decision rather than a knob — moving it anywhere in
/// that gap changes no answer.
const DEAD_SOURCE: usize = 2;

/// Offsets past a boundary to consider when grabbing its frame, in the order
/// they are preferred. A couple of seconds clears the cut itself; the later
/// two step over a dropout; the boundary second is the last resort, because a
/// grab landing exactly on a transition is the case this list exists for.
const GRAB_OFFSETS: [u32; 4] = [2, 6, 12, 0];

/// How close to the most detailed frame in the probe set a frame has to be to
/// be taken instead of it. Relative, not absolute: what counts as a detailed
/// frame depends on the deck, and a title slide and a dense one differ by far
/// less than either differs from a blank.
const GRAB_TOLERANCE: f32 = 0.95;

/// One place the lecture plausibly changes topic.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Candidate {
    /// Offset into the recording, in whole seconds.
    pub seconds: u32,
    /// Visual magnitude plus the pause bonus. Comparable within one lecture;
    /// not an absolute scale.
    pub score: f32,
    /// The raw mean-absolute-difference that triggered it, before any bonus.
    pub diff: f32,
    /// Whether a transcript silence backed this boundary up.
    pub pause: bool,
}

// ── The decode pass ──────────────────────────────────────────────────────────

/// Mean absolute difference between each sampled frame and the one before it.
///
/// One ffmpeg process, one second per sample, greyscale 160×90 raw frames on
/// stdout. Frames are diffed **as they arrive** against a single retained
/// previous frame: a two-hour lecture is 7200 frames ≈ 100 MB, which there is
/// no reason to hold.
///
/// The returned second is the frame's own timestamp (`fps=1` places frame *n*
/// at *n* seconds), so the first entry is at second 1.
///
/// `on_frame` sees that second as the frame is diffed. It is the only place in
/// the whole job that can say how far along a decode is — the pass is fifteen
/// seconds of nothing otherwise — so it fires per frame and the *caller* does
/// the throttling; [`run`] emits four times a second, not four hundred.
pub fn sample_diffs(
    ffmpeg: &Path,
    video: &Path,
    mut on_frame: impl FnMut(u32),
) -> Result<Vec<(u32, f32)>, String> {
    let mut child = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-i"])
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

    // Drained on its own thread: ffmpeg blocks writing to a full stderr pipe,
    // and a deadlock here would look exactly like a slow decode.
    let mut stderr = child.stderr.take().expect("piped stderr");
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).ok();
        text
    });

    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut previous = vec![0u8; FRAME_BYTES];
    let mut current = vec![0u8; FRAME_BYTES];
    let mut diffs: Vec<(u32, f32)> = Vec::new();
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
        if index > 0 {
            diffs.push((index, mean_abs_diff(&previous, &current)));
            on_frame(index);
        }
        std::mem::swap(&mut previous, &mut current);
        index += 1;
    }

    let status = child.wait().map_err(|e| e.to_string())?;
    let text = errors.join().unwrap_or_default();
    if !status.success() {
        let detail = text.lines().last().unwrap_or("no detail").trim().to_string();
        return Err(format!("ffmpeg failed: {detail}"));
    }
    if diffs.is_empty() {
        return Err("no video frames decoded".to_string());
    }
    Ok(diffs)
}

/// Fill `frame` completely, or report that the stream ended. A trailing
/// partial frame is ffmpeg being cut off mid-write; there is nothing to
/// compare it against, so it is dropped.
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

/// The silence before each cue, in seconds, paired with the second that cue
/// starts at.
///
/// This is the timing half of the frontend's `parseVtt`
/// (`app/src/lib/lectures.ts`) and handles the same two timestamp shapes
/// (`HH:MM:SS.mmm` and `MM:SS.mmm`). Cue *text* plays no part in boundary
/// detection, so none is parsed.
pub fn cue_gaps(vtt: &str) -> Vec<(u32, f32)> {
    let normalised = vtt.replace("\r\n", "\n");
    let mut gaps: Vec<(u32, f32)> = Vec::new();
    let mut previous_end = 0.0f32;

    for block in normalised.split("\n\n") {
        let Some(line) = block.lines().find(|l| l.contains(" --> ")) else {
            continue;
        };
        let mut halves = line.split(" --> ");
        let start = halves.next().map(str::trim).and_then(vtt_secs).unwrap_or(-1.0);
        let end = halves
            .next()
            .and_then(|h| h.split_whitespace().next())
            .and_then(vtt_secs)
            .unwrap_or(-1.0);
        if start < 0.0 {
            continue;
        }
        gaps.push((start as u32, (start - previous_end).max(0.0)));
        // A malformed end time must not drag the next gap out to the whole
        // lecture; fall back to the cue's own start.
        previous_end = if end >= start { end } else { start };
    }
    gaps
}

/// Seconds out of a WebVTT timestamp, `HH:MM:SS.mmm` or `MM:SS.mmm`.
///
/// `None` is a malformed stamp, and each reader decides what that is worth:
/// [`cue_gaps`] keeps going with a sentinel it then filters on, and
/// [`parse_transcript`] drops the block. One parser rather than two, because a
/// transcript that reads one way for the detector and another way for the
/// outline would put a chapter's start on a second the outline never printed.
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

/// Parse WebVTT timing *and* text into plain cues.
///
/// [`cue_gaps`] above reads the same file for the same two timestamp shapes and
/// throws the words away, because a pause bonus does not care what was said.
/// This one keeps them: the words are what [`outline`] merges with the slide
/// changes, and what the agent actually reads. Cue identifiers and VTT settings
/// are ignored, simple tags are stripped, and a malformed block is skipped
/// rather than poisoning the rest of the transcript.
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

/// Turn raw frame diffs and transcript gaps into a thinned, time-ordered set
/// of boundary candidates.
///
/// Four steps, in order: keep the loud frames; collapse a run of them into the
/// one that started it; score by magnitude with a small bonus for a nearby
/// silence; then thin to [`MIN_SPACING`] by taking the strongest first and
/// dropping everything in its shadow. Thinning greedily by strength rather than
/// sweeping left to right is what keeps the *important* boundary when two land
/// a minute apart — and it is what was measured, so the re-sort at the end is
/// the only thing that puts the result back in play order.
pub fn candidates(
    diffs: &[(u32, f32)],
    gaps: &[(u32, f32)],
    duration_secs: u32,
) -> Vec<Candidate> {
    candidates_with_spacing(diffs, gaps, duration_secs, MIN_SPACING)
}

/// The shared detector with a caller-selected thinning radius.
///
/// Chapters use 90 seconds because they are topic spans; the reading copy
/// uses a denser radius because its paragraphs follow visual changes. Keeping the radius at
/// this boundary lets both jobs share the measured decode, collapse and score
/// pipeline without pretending they want the same output density.
pub fn candidates_with_spacing(
    diffs: &[(u32, f32)],
    gaps: &[(u32, f32)],
    duration_secs: u32,
    min_spacing: u32,
) -> Vec<Candidate> {
    // Loud frames, with a run collapsed onto its first second. The run keeps
    // the largest magnitude it contained: a build-up that peaks two frames in
    // is still as strong as its peak.
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
            let pause = pauses
                .iter()
                .any(|p| p.abs_diff(seconds) <= PAUSE_WINDOW);
            Candidate {
                seconds,
                score: diff + if pause { PAUSE_BONUS } else { 0.0 },
                diff,
                pause,
            }
        })
        .collect();

    // Strongest first, then drop anything within the caller's radius of something
    // already kept.
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
            .all(|k| k.seconds.abs_diff(candidate.seconds) >= min_spacing)
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
    /// Which of the capture's streams this is, for a caller that wants to say.
    pub source: crate::echo360::SourceNum,
    /// The file the diffs came from — and therefore the file any frame grab
    /// has to use, or the pictures would be of the stream nobody read.
    pub video: PathBuf,
    /// The decode pass itself, for a caller that thins it its own way
    /// (`reading` does).
    pub diffs: Vec<(u32, f32)>,
    /// [`candidates`] over those diffs, in play order.
    pub candidates: Vec<Candidate>,
}

/// Decode the lecture's slide capture — whichever of its streams that is.
///
/// **Which source holds the slides is not consistent, even inside one
/// subject.** Four MULT20015 lectures put them on source 1 and a fifth on
/// source 2, because that week the projector capture failed and the only thing
/// recording the screen was the second stream. So the answer is measured per
/// lecture rather than remembered, and nothing is persisted: re-detecting is
/// one decode and always reflects what is on disk, where a column would be a
/// second place for the same fact to be wrong.
///
/// Source 1 is decoded first and kept unless it is *dead* — at most
/// [`DEAD_SOURCE`] candidates in a whole recording. Only then is source 2
/// decoded, so the normal case stays the single pass it has always been and
/// the broken one costs about fifteen seconds more.
///
/// **Whichever gives more candidates is measurably the wrong question.** A room
/// camera saturates the threshold — a quarter of every second clears it — and
/// thinning then lays those down as a near-uniform grid, so on one measured
/// lecture the camera beats the real slide deck 20 to 16 while carrying no
/// topic boundaries at all. Only a dead source 1 is grounds to look at
/// source 2.
///
/// `source` overrides all of it — `--source` on the CLI, and the app's picker
/// — and when it is set nothing else is decoded.
///
/// `on_frame` is [`sample_diffs`]'s and fires for both passes, so a fallback
/// restarts whatever progress the caller draws from it. That is what is
/// actually happening.
pub fn detect(
    ffmpeg: &Path,
    lecture_dir: &Path,
    video: &Path,
    gaps: &[(u32, f32)],
    duration_secs: u32,
    source: Option<crate::echo360::SourceNum>,
    mut on_frame: impl FnMut(u32),
) -> Result<Detection, String> {
    // The file on disk, not the `video2_path` column: they are the same path
    // by construction, and a stream that was downloaded but never recorded is
    // still one this can read.
    let second = crate::echo360::source_path(lecture_dir, 2);

    let read = |path: PathBuf,
                source: crate::echo360::SourceNum,
                on_frame: &mut dyn FnMut(u32)|
     -> Result<Detection, String> {
        let diffs = sample_diffs(ffmpeg, &path, on_frame)?;
        let candidates = candidates(&diffs, gaps, duration_secs);
        Ok(Detection { source, video: path, diffs, candidates })
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

/// One legible JPEG per candidate, at `<out_dir>/<seconds>.jpg`.
///
/// A seek-based single-frame grab is instant — ffmpeg jumps to the keyframe
/// rather than decoding forward — so this stays a handful of processes per
/// candidate rather than a second full pass. 768px wide lands around 30 KB and
/// keeps slide titles and formulas readable, which is what a model will need to
/// name the chapter.
///
/// **The boundary second is the right timestamp and the wrong frame.** The
/// loudest changes in a recording are the screen share stopping and starting,
/// so a grab taken exactly at one catches the black — and a grab a fixed two
/// seconds later can catch the room's "connect your laptop" splash instead.
/// Either is a frame with no lecture content in it, which silently poisons
/// whatever reads it. So each candidate is probed at [`GRAB_OFFSETS`] using the
/// *same* seek the grab will use (input seeking lands on a keyframe, so a
/// windowed decode would measure a different frame than it wrote), and the
/// earliest frame within [`GRAB_TOLERANCE`] of the most detailed one wins.
/// A blank loses on detail; so does a splash screen, without anything here
/// having to know what one looks like. A recording that is blank across the
/// whole probe set still gets a frame — there is nothing better to write, and
/// a dropout that long is visible for what it is.
///
/// The file keeps the **boundary** second in its name, not the offset one:
/// that is the timestamp every other part of this refers to.
///
/// `on_grab` fires with how many are written so far — five probes and a JPEG
/// per boundary is ten seconds on a long lecture, and it is countable, so the
/// panel says `12 / 50` rather than spinning.
///
/// **A JPEG from a previous run that this one will not overwrite is deleted.**
/// Grabs are written by their second, so a re-run whose candidate set moved
/// used to leave the old set's frames behind — and after [`detect`] that can
/// mean frames off a stream this run never looked at. The lecture whose slide
/// capture failed kept a grab of the room's Crestron splash in its folder for
/// exactly that reason: the one candidate the black stream produced. Nothing
/// reads a frame after the turn that asked for it, so the only thing an
/// orphan can do is mislead whoever opens the folder next.
///
/// Only files are swept, and only `.jpg` directly in `out_dir` — the
/// subfolders beside them belong to other jobs (`live/` is the chat dock's,
/// `reading/` is the reading copy's) and each sweeps its own.
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
        grab_frame(ffmpeg, video, second, &out)?;
        written.push(out);
        on_grab(written.len());
    }
    Ok(written)
}

/// Delete the `.jpg` files in `out_dir` that this run is not about to rewrite.
///
/// Best effort on purpose: a frame that cannot be removed is clutter, and
/// failing a ten-minute job over it would be the wrong trade. A name that is
/// not a plain second was not written by [`extract_frames`], so it is left
/// alone rather than guessed at.
fn sweep_orphans(out_dir: &Path, keep: &[u32]) {
    let Ok(entries) = std::fs::read_dir(out_dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
            continue;
        }
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let second = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<u32>().ok());
        match second {
            Some(second) if keep.contains(&second) => continue,
            Some(_) => {
                std::fs::remove_file(&path).ok();
            }
            None => continue,
        }
    }
}

/// One probed JPEG of `second`, written to `out`.
///
/// Shared with the live grab the chat dock takes of the playhead's moment
/// (`app::lecture_grab_frame`) rather than copied: the probing above is the
/// defence against handing a model the room's AV splash screen or a black
/// frame, and a grab the *student* asked about wants it for exactly the same
/// reason. The offset it picks stays out of the filename — the second asked
/// for is the one everything else refers to.
pub fn grab_frame(ffmpeg: &Path, video: &Path, second: u32, out: &Path) -> Result<(), String> {
    let at = best_offset(ffmpeg, video, second);
    let status = Command::new(ffmpeg)
        .args(["-v", "error", "-nostdin", "-y", "-ss", &at.to_string(), "-i"])
        .arg(video)
        .args(["-frames:v", "1", "-vf", "scale=768:-2", "-q:v", "3"])
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

/// Which second to actually grab `boundary`'s frame from.
///
/// Probes cost ~40 ms each, so all of [`GRAB_OFFSETS`] are measured and the
/// earliest one close enough to the best is taken — "earliest" so a sparse
/// title slide is not passed over for a busier slide later in the lecture,
/// which would name the chapter after the wrong thing.
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

/// The choosing half of [`best_offset`], without an ffmpeg in it: the earliest
/// probe within [`GRAB_TOLERANCE`] of the most detailed one. Probes are given
/// in preference order, so "earliest" means first in the list, not lowest.
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
/// its grey values, over the same 160×90 the detector samples at.
///
/// A blank frame scores ~0 and a slide scores in the high tens or hundreds,
/// which is all this has to separate. `None` means ffmpeg produced no frame —
/// past the end of the recording, normally.
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
// Candidates are seconds; chapters are seconds with a name on them. Naming is
// a *coding agent's* job rather than an API call, and that choice is what
// keeps the prompt below small: the agent is handed the candidate list, the
// path to `transcript.vtt` and the path to `frames/`, and reads what it needs.
// An API prompt would have to carry fifty frames to let a model look at five.
//
// The agent never touches the database. It replies with JSON, Rust parses it,
// validates it against the candidate set, and writes the rows — chapters are
// derived data like `pages`, not the student's own planning, so there is no
// `oculus chapter add` and no write door for a model. See `projects.rs` for
// the other shape, and why it is different.

/// One named span of a recording. It ends where the next one begins — and the
/// last at the lecture's duration — so there is no end here to disagree with
/// the next chapter's start.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Chapter {
    pub start_seconds: u32,
    pub title: String,
    pub summary: String,
}

/// What the agent replies with, before any of it is believed. Field names are
/// the prompt's contract; the aliases are there because a model asked for
/// `start` sometimes writes `start_seconds` anyway, and refusing that would be
/// a whole turn thrown away over a synonym.
#[derive(serde::Deserialize)]
struct ReplyChapter {
    #[serde(alias = "start_seconds", alias = "seconds", alias = "at")]
    start: f64,
    title: String,
    #[serde(default)]
    summary: String,
}

/// A reply that wrapped the array in an object, which both providers do
/// perhaps one turn in five however plainly the format is asked for.
#[derive(serde::Deserialize)]
struct ReplyEnvelope {
    chapters: Vec<ReplyChapter>,
}

/// More than this and it is a slide list, not a shape. A ceiling rather than a
/// target: most lectures are five to eight things.
pub const MAX_CHAPTERS: usize = 12;

/// The lecture a chaptering run is about. Everything the prompt needs and
/// nothing it does not — the agent reads the rest off disk itself.
pub struct Job<'a> {
    pub title: &'a str,
    pub duration_secs: u32,
    /// Where the recording's folder sits *relative to the agent's working
    /// directory*, which is always `agents/`. Paths it can paste into a
    /// `Read`, not paths it has to rebuild.
    pub lecture_dir: &'a str,
    /// The subject's course folder, same relative shape. An Echo360 title is
    /// a room booking ("MULT20015_2026_SM2 MO L105"), so without this the
    /// agent goes looking for the deck itself — measured, and it costs
    /// several turns of `find` before it gets there.
    pub course_dir: Option<&'a str>,
    /// How many slide changes the detector found. The list itself is not in
    /// the prompt — it is in the outline, in place — but the count is worth
    /// saying, because it tells the agent roughly how dense the markers it is
    /// about to grep for will be.
    pub detected: usize,
    /// Whether the outline has any transcript in it at all. Chaptering
    /// tolerates a lecture that arrived without one: the pause bonus is lost,
    /// no cue start is in the allowed set, and the prompt should not send the
    /// agent looking for words that are not there.
    pub has_transcript: bool,
}

/// The transcript and the detected slide changes as one document, in play
/// order.
///
/// Written to `<lecture dir>/outline.md` before the agent turn and named in the
/// prompt in place of the raw VTT.
///
/// **A file, not prompt text.** A lecture is around 2500 cues, and handing over
/// a path so the agent reads only the spans it wants is the whole reason this
/// job drives a coding agent instead of calling a model API. What merging buys
/// is not brevity but correlation: "the picture changed here" and "the subject
/// turned here" arrive on adjacent lines instead of in two documents with
/// timestamp arithmetic between them.
///
/// Every line is `second  timestamp  text`. Both are printed because both are
/// load-bearing: the clock is what a person reads, and the bare second is what
/// a chapter's `start` has to be exactly — a model asked to convert one to the
/// other will sometimes round, and a rounded second is not in the allowed set.
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
    // Anything after the last spoken word — and, when there is no transcript
    // at all, every marker there is.
    for change in marks {
        line(&mut out, change.seconds, &marker(change));
    }
    out
}

/// The prompt for one chaptering turn.
///
/// Deliberately short. It says what the lecture is, what the outline is, where
/// the frames are, and what a good chapter looks like — then gets out of the
/// way. Three things in it are lessons rather than decoration: the note that a
/// slide change is *not* a chapter (candidate density varies threefold between
/// lectures of the same length, so "one chapter per marker" gives a boundary
/// every two minutes on a busy deck), the note about the room's AV splash
/// screen (a dropout spanning the whole probe window survives frame selection,
/// and a model that does not know what it is looking at will happily name a
/// chapter after it), and the `grep` — named because the alternative is an
/// agent reading 2500 lines to find twenty of them.
///
/// The candidate table this used to carry is gone: the markers are in the
/// outline where they belong, next to the words they interrupt.
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

/// `HH:MM:SS`. The CLI has its own copy for its own output; this one is what
/// every prompt about a recording uses — this module's and the lecture brief
/// in `harness::instructions` — and it must agree with the frame filenames,
/// which are plain seconds, so both are always printed.
pub(crate) fn hms(secs: u32) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

/// The chapter array out of whatever the agent actually said.
///
/// Models wrap JSON in prose, in fences, or in an object, however plainly the
/// format was asked for — the same tolerance `clean_title` in
/// `harness/mod.rs` exists for, and for the same reason: the alternative is
/// throwing away a good answer over its packaging. Four attempts, cheapest
/// first: the whole reply, each fenced block, then the first balanced
/// `[…]` or `{…}` found anywhere in the text.
///
/// This only reads the reply. Whether the chapters are *allowed* is
/// [`validate`]'s question.
pub fn parse_chapters(reply: &str) -> Result<Vec<Chapter>, String> {
    for candidate in json_candidates(reply) {
        if let Some(parsed) = decode(&candidate) {
            return Ok(parsed);
        }
    }
    Err(format!(
        "no chapter list in the reply ({} chars): {}",
        reply.chars().count(),
        clip(reply.trim(), 200)
    ))
}

/// Either shape, as a list of chapters, or `None` if this fragment is not one.
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
                // A model that writes 742.0 means 742; one that writes a
                // negative second is caught by `validate`, not here.
                start_seconds: c.start.max(0.0).round() as u32,
                title: c.title.trim().to_string(),
                summary: c.summary.trim().to_string(),
            })
            .collect(),
    )
}

/// Fragments of `reply` worth trying to parse, in order of how likely they are
/// to be the answer.
fn json_candidates(reply: &str) -> Vec<String> {
    let mut out = vec![reply.trim().to_string()];
    // Fenced blocks, ```json or otherwise. The opening fence's info string is
    // dropped with the rest of its line.
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
    // Anything balanced, anywhere — the prose-wrapped case.
    for open in ['[', '{'] {
        out.extend(balanced_runs(reply, open));
    }
    out
}

/// At most [`BALANCED_RUNS`] balanced `open`…`close` spans of `text`, in order.
///
/// Several rather than the first because prose around the answer can contain a
/// bracket of its own — a citation, an empty list, a worked example — and the
/// real array would then never be tried. String literals and their escapes are
/// respected, so a bracket inside a summary cannot end a scan early.
fn balanced_runs(text: &str, open: char) -> Vec<String> {
    const BALANCED_RUNS: usize = 8;
    let close = if open == '[' { ']' } else { '}' };
    let mut out: Vec<String> = Vec::new();
    let mut from = 0usize;
    while out.len() < BALANCED_RUNS {
        let Some(offset) = text[from..].find(open) else { break };
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
            // Unbalanced from here on: nothing later can close either.
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

/// Whether a parsed chapter set may be written at all.
///
/// **One bad chapter rejects the whole set.** A chapter list is a shape rather
/// than a pile of rows — drop the third of nine and the second chapter now
/// silently swallows twenty minutes it was never named for — so a single
/// failure means nothing is written, exactly as a rejected item rolls back a
/// whole task breakdown in `projects::create_tasks`. The error names the
/// chapter, because "not a candidate boundary" on its own is unfixable.
///
/// `boundaries` is every second the outline printed: second 0, the detected
/// slide changes, and — when the lecture has a transcript — every cue start.
/// **Wider than the candidate set, and still closed.** A lecture whose slide
/// capture failed has one detected change and a perfectly good transcript, so
/// a menu of one is the defect rather than the safeguard; but the agent still
/// cannot invent a timestamp, because every second it may pick came off a
/// file, so the all-or-nothing rollback below still means something.
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
// Detection, frames, the agent turn, validation and the write, in one place so
// that the CLI and the app run the *same* job rather than two that drift. The
// callers differ only in where they get the selection from (a flag, or the
// `lectureChapters` row of the model registry) and what they do while it runs
// (print, or emit an event at the end).

/// One chaptering run.
pub struct Run<'a> {
    pub data_dir: &'a Path,
    /// A full lecture id. Prefix matching is the CLI's door, not this one's.
    pub lecture_id: &'a str,
    /// Which agent, model and level to drive. Nothing is defaulted here: the
    /// caller has already resolved it (`harness::jobs`).
    pub selection: &'a crate::harness::jobs::JobSelection,
    /// Replace an existing chapter set instead of refusing to touch it.
    pub force: bool,
    /// Read this stream instead of letting [`detect`] choose. `None` is the
    /// normal case and the one the app takes unless the picker was used.
    pub source: Option<crate::echo360::SourceNum>,
}

/// Where a run has got to, for a caller that draws progress.
///
/// The job is one long bar of nothing otherwise: eight to eleven minutes with
/// a single `ready` at the end of it. These are the moments the job genuinely
/// changes what it is doing — everything *inside* the agent turn arrives on
/// `on_event` instead, because the turn's own tool calls are the only honest
/// account of those nine minutes.
///
/// Two of them are countable and one is not, which is the whole reason this is
/// an enum rather than a percentage: a decode knows how many frames are left,
/// an agent does not.
pub enum Step<'a> {
    /// The decode pass has reached `second` of a `duration`-second recording.
    /// Fires often; [`run`] throttles it before the caller sees it.
    Decoding { second: u32, duration: u32 },
    /// The candidate set exists — the lecture's title, its length, and how
    /// many boundaries were found.
    Detected {
        title: &'a str,
        duration: u32,
        candidates: usize,
    },
    /// `done` of `total` boundary frames are on disk.
    Grabbing { done: usize, total: usize },
    /// The prompt is with the agent. From here until the reply, `on_event` is
    /// the only thing that knows anything.
    Asking,
    /// The reply parsed and validated; the rows are going in.
    Writing,
}

/// What a finished run has to say for itself.
pub struct Outcome {
    pub title: String,
    pub duration_seconds: u32,
    /// How many boundaries the detector offered, second 0 not counted.
    pub candidates: usize,
    /// Which stream it read them off. Worth reporting rather than assuming:
    /// a 2 here is the only visible sign that source 1 was dead.
    pub source: crate::echo360::SourceNum,
    pub chapters: Vec<Chapter>,
}

/// Detect, grab, ask, validate, write.
///
/// Blocking from end to end and eight to eleven minutes long — the agent turn
/// is nearly all of it — so both callers run it off the thread that has to
/// stay responsive: the CLI is that thread, and the app spawns one.
///
/// `on_step` follows the pipeline through its phases ([`Step`]); `on_event`
/// sees every harness event of the turn, which is how the CLI draws the
/// agent's tool rows and how the app's panel says what the agent is reading.
/// The split is deliberate: the phases are this function's own, the turn's
/// detail belongs to the harness and neither caller should have to guess at
/// one from the other.
///
/// The status column tracks the run from the moment the work starts: `running`
/// until a terminal answer, then `error` with the message on it, or `ready`
/// stamped by `store::save_chapters`. The guards above it — no video, chapters
/// already there — fail before anything is claimed, so a refusal never leaves
/// a status behind.
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
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no lecture {id}"))?;
    let title: String = row.get("title");
    let duration = row.get::<i64, _>("duration_seconds").max(0) as u32;
    let video: Option<String> = row.get("video_path");
    let transcript: Option<String> = row.get("transcript_path");
    let code: Option<String> = row.get("code");

    let existing = rt.block_on(crate::store::chapters(pool, id))?;
    if !existing.is_empty() && !job.force {
        return Err(format!(
            "{title} already has {} chapter(s) — re-running replaces them",
            existing.len()
        ));
    }
    let video = video.ok_or_else(|| {
        format!("{title} is not downloaded — `oculus run -l --videos` fetches it")
    })?;
    let video = PathBuf::from(&video);
    if !video.exists() {
        return Err(format!("{} is on record but missing from disk", video.display()));
    }
    let ffmpeg = crate::echo360::find_ffmpeg(None)
        .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

    // Claimed before the decode rather than before the turn: detection is
    // fifteen seconds a student can see happening, and a button that only
    // lights up once the agent starts reads as a button that did nothing.
    rt.block_on(crate::store::set_chapter_status(pool, id, Some("running"), None))?;

    let outcome = (|| -> Result<Outcome, String> {
        // Detection is seconds and nothing is cached, so this is the same pass
        // `oculus lecture candidates` makes; see the module docs.
        //
        // One frame is one second of the recording, so the decode reports
        // itself four times a second rather than per frame: a 107-minute
        // lecture would otherwise send 6400 events through a Tauri channel to
        // move a percentage that only has a hundred places to be.
        // Read once, used three ways: the pauses that score a candidate, the
        // words the outline is made of, and the cue starts the validator will
        // accept. A missing or unreadable transcript costs all three and is
        // not worth failing over — chaptering works off the picture alone.
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
            return Err(format!("no boundary candidates in {title} — nothing to chapter"));
        }
        on_step(Step::Detected {
            title: &title,
            duration,
            candidates: found.len(),
        });

        // Second 0 is never a detected candidate — the first change is
        // typically twenty seconds in — but a lecture always starts somewhere,
        // so the opening is prepended as an always-available boundary and
        // accepted as one by the validator.
        let mut frames_at: Vec<u32> = vec![0];
        frames_at.extend(found.iter().map(|c| c.seconds));

        // What the agent may actually start a chapter at. Frames are grabbed
        // at the slide changes only — a frame per cue would be two and a half
        // thousand JPEGs of the same slide — but a *boundary* may also be a
        // cue start, which is what lets a lecture with a dead slide capture be
        // chaptered from what was said.
        let mut boundaries = frames_at.clone();
        boundaries.extend(cues.iter().map(|c| c.start.max(0.0) as u32));
        boundaries.sort_unstable();
        boundaries.dedup();

        std::fs::write(dir.join("outline.md"), outline(&title, &cues, &found))
            .map_err(|e| format!("{}: {e}", dir.join("outline.md").display()))?;

        let total = frames_at.len();
        extract_frames(&ffmpeg, &detected.video, &frames_at, &dir.join("frames"), |done| {
            on_step(Step::Grabbing { done, total })
        })?;

        // An Echo360 title is a room booking rather than a topic, so the
        // subject's own folder — where the deck is — is worth naming.
        let course_dir = code
            .as_deref()
            .map(|c| format!("../courses/{}", crate::paths::safe_dir(c)));

        let text = prompt(&Job {
            title: &title,
            duration_secs: duration,
            // Every agent turn runs from the library's `agents/` folder, so
            // this is the path the agent can paste straight into a read.
            lecture_dir: &format!("../lectures/{id}"),
            course_dir: course_dir.as_deref(),
            detected: found.len(),
            has_transcript: !cues.is_empty(),
        });

        on_step(Step::Asking);
        let reply = Arc::new(Mutex::new(String::new()));
        let collect = reply.clone();
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
            move |ev| {
                if let HarnessEvent::AssistantMessage { text } = ev {
                    collect.lock().unwrap().push_str(text);
                }
                on_event(ev);
            },
        );

        let reply = reply.lock().unwrap().clone();
        let chapters = turn
            .and_then(|()| parse_chapters(&reply))
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
        // The failure is kept on the row, not just reported: a player showing
        // "chaptering failed" needs to say why and offer a retry, and a bare
        // status cannot carry a message.
        rt.block_on(crate::store::set_chapter_status(pool, id, Some("error"), Some(e)))?;
    }
    outcome
}

// ── Tauri ────────────────────────────────────────────────────────────────────

pub mod app {
    use super::*;
    use tauri::{AppHandle, Emitter};

    /// What the webview gets when a run ends, and the only chapter event there
    /// is. Deliberately not `lectures-changed`: that one fires on every
    /// progress save while a recording plays, and a result eight minutes in the
    /// making would be indistinguishable from a scrub.
    pub const LECTURE_CHAPTERS_EVENT: &str = "lecture-chapters";

    /// Where the run has got to, emitted throughout. Separate from
    /// [`LECTURE_CHAPTERS_EVENT`] because the two have different lifetimes: a
    /// finish is a fact the panel re-reads the database on, a step is a line it
    /// paints and forgets.
    pub const LECTURE_CHAPTER_PROGRESS_EVENT: &str = "lecture-chapter-progress";

    /// One step of a run in flight.
    ///
    /// Everything here is display: nothing is persisted, and a panel that
    /// missed the last one is only ever one event behind. That is why a run
    /// already in flight when the app started shows no step until its next one
    /// — there is no column to read it from, and inventing one would mean
    /// writing to the row four times a second during the decode.
    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Progress {
        lecture_id: String,
        /// `decoding` | `frames` | `agent` | `naming` | `writing`.
        phase: &'static str,
        /// The one line under the phase — a tool's own title while the agent
        /// works, and nothing at all for a phase that speaks for itself.
        detail: Option<String>,
        /// What that tool was, so the panel can use the timeline's verbs
        /// ("Reading", "Looking up") rather than a second vocabulary.
        kind: Option<crate::harness::ToolKind>,
        /// Countable phases only; the agent turn has no denominator.
        done: Option<u32>,
        total: Option<u32>,
    }

    impl Progress {
        fn at(lecture_id: &str, phase: &'static str) -> Progress {
            Progress {
                lecture_id: lecture_id.to_string(),
                phase,
                detail: None,
                kind: None,
                done: None,
                total: None,
            }
        }
    }

    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Finished {
        lecture_id: String,
        /// How *this* request ended, in the column's own vocabulary. They
        /// agree except on a refusal — a lecture that already has chapters
        /// reports `error` here and keeps its `ready` row, because nothing
        /// was touched.
        status: &'static str,
        chapters: usize,
        error: Option<String>,
    }

    /// Chapter a lecture with the agent the `lectureChapters` job is
    /// configured with.
    ///
    /// The job is eight to eleven minutes of ffmpeg and one very long agent
    /// turn, so the command starts it on a thread of its own and returns as
    /// soon as the run is claimed. Progress is the `chapter_status` column —
    /// `running` from here, then `ready` or `error` — and the end is
    /// [`LECTURE_CHAPTERS_EVENT`]. Nothing goes through the harness event
    /// stream: a headless run reports on thread id 0, and the app started this
    /// one, so it already knows whose it is.
    #[tauri::command]
    pub async fn lecture_find_chapters(
        app: AppHandle,
        lecture_id: String,
        force: Option<bool>,
        source: Option<u8>,
    ) -> Result<(), String> {
        // `None` is the normal call and means let the detector choose; a value
        // only ever comes from the source picker, which only offers the two.
        if let Some(n) = source {
            if n != 1 && n != 2 {
                return Err(format!("{n} is not a source — a capture has 1 and sometimes 2"));
            }
        }
        let pool = crate::store::open_pool().await?;
        // The only check worth making the caller wait for: a second run over
        // the same lecture would spend a second subscription turn and race the
        // first one's write.
        let running: Option<String> =
            sqlx::query_scalar("SELECT chapter_status FROM lectures WHERE id = ?1")
                .bind(&lecture_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| e.to_string())?
                .flatten();
        if running.as_deref() == Some("running") {
            return Err("that lecture is already being chaptered".into());
        }
        drop(pool);

        let data_dir = crate::paths::data_dir();
        let force = force.unwrap_or(false);
        std::thread::spawn(move || {
            // Its own runtime, so the pool this run's queries use belongs to
            // the thread that blocks on them — the shape the harness consumer
            // thread has for the same reason.
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => return eprintln!("[oculus] chapters: {e}"),
            };
            let pool = match rt.block_on(crate::store::open_pool()) {
                Ok(p) => p,
                Err(e) => return eprintln!("[oculus] chapters: {e}"),
            };
            let selection = rt.block_on(crate::harness::jobs::selection(
                &pool,
                crate::harness::jobs::Job::LectureChapters,
            ));
            // One emitter for both halves of the report: the pipeline's own
            // phases arrive as `Step`, the nine minutes inside the agent turn
            // arrive as harness events, and the panel should not be able to
            // tell which of the two it is drawing.
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
                            // A lecture whose row has no duration still gets a
                            // phase; it just cannot have a fraction.
                            total: (duration > 0).then_some(duration),
                            ..Progress::at(&id, "decoding")
                        },
                        Step::Detected { title, candidates, .. } => {
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

            // The reply *is* the chapter JSON, so the first delta of it is the
            // agent having made up its mind — a real phase change, and the one
            // that would otherwise leave the panel sitting on whichever file
            // happened to be read last for a minute or more.
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
        });
        Ok(())
    }

    /// One JPEG of the moment the playhead is at, for a message sent from
    /// the lecture player's dock ([`docs/harness.md`]).
    ///
    /// **It returns the path the *agent* can read**, not one the webview can
    /// open: `../lectures/<id>/frames/live/<seconds>.jpg`, relative to
    /// `agents/`, which every thread runs from. The page never opens the file
    /// — it puts this string in the message, and the CLI opens it.
    ///
    /// Frames live in their own `live/` subfolder so a message's grab can
    /// never collide with a chaptering run's, which are named by boundary
    /// second in the folder above. They are overwritten freely: the same
    /// second asked for twice is the same frame.
    ///
    /// The probing is [`grab_frame`]'s, so a message sent while the screen
    /// share is between slides still attaches something with lecture content
    /// on it rather than a black frame. Four probes and a grab is ~200 ms.
    #[tauri::command]
    pub async fn lecture_grab_frame(lecture_id: String, seconds: u32) -> Result<String, String> {
        let pool = crate::store::open_pool().await?;
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT title, video_path FROM lectures WHERE id = ?1")
                .bind(&lecture_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| e.to_string())?;
        let (title, video) = row.ok_or_else(|| format!("no lecture {lecture_id}"))?;
        // The same refusal `run` makes, for the same reason: there is no
        // frame to take, and naming the download is more use than a missing
        // file's path.
        let video = video.ok_or_else(|| {
            format!("{title} is not downloaded — `oculus run -l --videos` fetches it")
        })?;
        let video = PathBuf::from(video);
        if !video.exists() {
            return Err(format!("{} is on record but missing from disk", video.display()));
        }
        let ffmpeg = crate::echo360::find_ffmpeg(None)
            .ok_or("no ffmpeg found — install it, or run `bun run ffmpeg`")?;

        let dir = crate::echo360::lecture_dir(&crate::paths::data_dir(), &lecture_id)
            .join("frames")
            .join("live");
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let out = dir.join(format!("{seconds}.jpg"));
        tokio::task::spawn_blocking(move || grab_frame(&ffmpeg, &video, seconds, &out))
            .await
            .map_err(|e| e.to_string())??;
        Ok(format!("../lectures/{lecture_id}/frames/live/{seconds}.jpg"))
    }

    /// Startup: a run killed mid-turn left `running` on the row with no
    /// `chaptered_at`, and nothing is going to finish it — the same sweep
    /// `harness::app::reconcile` makes over threads.
    pub fn reconcile(app: &AppHandle) {
        let _ = app;
        tauri::async_runtime::spawn(async {
            if let Ok(pool) = crate::store::open_pool().await {
                if let Ok(n) = crate::store::reconcile_chapter_status(&pool).await {
                    if n > 0 {
                        eprintln!("[oculus] chapters: cleared {n} interrupted run(s)");
                    }
                }
            }
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
        assert_eq!(found.iter().map(|c| c.seconds).collect::<Vec<_>>(), vec![400]);
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
        assert_eq!(found.iter().map(|c| c.seconds).collect::<Vec<_>>(), vec![10, 160]);
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
        assert_eq!(found[1].diff, 20.0, "the bonus does not touch the magnitude");

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
        assert_eq!(found.iter().map(|c| c.seconds).collect::<Vec<_>>(), vec![100]);
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
        // Measured on the reference lecture at its 1386 s boundary: the cut
        // itself is black, +2 and +6 are the room's AV splash (the same static
        // image every time, hence the identical value), and the slide is back
        // by +12. Probes arrive in GRAB_OFFSETS order: 2, 6, 12, 0.
        let probed = [(1388, 81.4), (1392, 81.4), (1398, 102.2), (1386, 0.0)];
        assert_eq!(pick_offset(&probed), Some(1398));

        // At 1724 s the boundary frame is a perfectly good slide and +2 is the
        // splash; the first frame within tolerance of the best wins.
        let probed = [(1726, 81.4), (1730, 102.5), (1736, 102.5), (1724, 102.6)];
        assert_eq!(pick_offset(&probed), Some(1730));
    }

    #[test]
    fn a_run_sweeps_the_grabs_it_will_not_rewrite() {
        let dir = std::env::temp_dir().join(format!("oculus-sweep-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("live")).unwrap();
        for name in ["50.jpg", "313.jpg", "1767.jpg", "notes.txt", "keyframe.jpg"] {
            std::fs::write(dir.join(name), b"x").unwrap();
        }
        // The chat dock's own grabs are a folder, not a file, and are not this
        // run's business.
        std::fs::write(dir.join("live").join("900.jpg"), b"x").unwrap();

        sweep_orphans(&dir, &[313, 1767, 2550]);

        let left = |name: &str| dir.join(name).exists();
        assert!(!left("50.jpg"), "an orphan from a previous candidate set goes");
        assert!(left("313.jpg") && left("1767.jpg"), "a frame this run rewrites stays");
        assert!(left("notes.txt"), "only JPEGs are swept");
        assert!(left("keyframe.jpg"), "a name that is not a second was not written here");
        assert!(left("live/900.jpg"), "another job's subfolder is untouched");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_frame_grab_prefers_the_earliest_good_frame() {
        // The ordinary case: nothing wrong anywhere in the probe set, so the
        // grab happens a couple of seconds past the cut and goes no further.
        let probed = [(114, 102.9), (118, 102.9), (124, 102.9), (112, 102.9)];
        assert_eq!(pick_offset(&probed), Some(114));

        // A sparse title slide must not be passed over for a denser slide
        // twelve seconds into the chapter — that would name it after the
        // wrong thing. Within tolerance is good enough.
        let probed = [(22, 98.0), (26, 99.0), (32, 101.0), (20, 98.5)];
        assert_eq!(pick_offset(&probed), Some(22));
    }

    #[test]
    fn a_frame_grab_with_nothing_to_go_on_still_picks_something() {
        // Every probe blank — a long dropout. There is nothing better to
        // write, so the first offset wins rather than the caller getting
        // nothing.
        let probed = [(1388, 0.0), (1392, 0.0), (1398, 0.0), (1386, 0.0)];
        assert_eq!(pick_offset(&probed), Some(1388));
        // Past the end of the recording, ffmpeg returns no frames at all.
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
        assert_eq!(parse_chapters(&reply).unwrap()[0].title, "Qubits and superposition");
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
        let set = vec![chapter(0, "Opening"), chapter(700, "Middle"), chapter(2000, "End")];
        assert!(validate(&set, &BOUNDS, 2400).is_ok());
    }

    #[test]
    fn a_boundary_the_outline_never_printed_rejects_the_whole_set() {
        let set = vec![chapter(0, "Opening"), chapter(701, "Middle"), chapter(2000, "End")];
        let error = validate(&set, &BOUNDS, 2400).unwrap_err();
        assert_eq!(
            error,
            "chapter 2 (\"Middle\"): 701 is not one of the seconds in the outline"
        );
    }

    #[test]
    fn a_transcript_cue_start_is_a_boundary_too() {
        // The widened set: the slide changes, plus wherever anybody spoke.
        // This is what lets a lecture whose slide capture was black still be
        // chaptered — the words are intact even when the picture is not.
        let mut bounds = BOUNDS.to_vec();
        bounds.push(701);
        let set = vec![chapter(0, "Opening"), chapter(701, "Middle"), chapter(2000, "End")];
        assert!(validate(&set, &bounds, 2400).is_ok());
    }

    #[test]
    fn chapters_out_of_order_reject_the_whole_set() {
        let set = vec![chapter(0, "Opening"), chapter(1200, "Middle"), chapter(700, "End")];
        let error = validate(&set, &BOUNDS, 2400).unwrap_err();
        assert!(error.starts_with("chapter 3 (\"End\"): starts at 700, which is not after"), "{error}");
        // A repeat is not contiguous either — two chapters starting at the same
        // second means one of them is zero seconds long.
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
        assert!(error.starts_with("13 chapters is more than the 12 allowed"), "{error}");
        // Twelve is the ceiling, not one short of it.
        assert!(validate(&set[..12], &bounds, 9000).is_ok());
    }

    #[test]
    fn an_empty_or_gutted_set_is_refused() {
        assert!(validate(&[], &BOUNDS, 2400).is_err());
        let mut set = vec![chapter(0, "Opening")];
        set[0].summary.clear();
        assert!(validate(&set, &BOUNDS, 2400).unwrap_err().contains("needs a summary"));
        set[0].summary = "Sets up.".into();
        set[0].title.clear();
        assert!(validate(&set, &BOUNDS, 2400).unwrap_err().contains("needs a title"));
    }

    #[test]
    fn a_chapter_past_the_end_of_the_recording_is_refused() {
        let set = vec![chapter(0, "Opening"), chapter(2000, "End")];
        assert!(validate(&set, &BOUNDS, 2000).unwrap_err().contains("past the end"));
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
        assert!(text.contains("\n  ../lectures/abc-123/outline.md"), "indented under the folder");
        assert!(text.contains("\n  ../lectures/abc-123/frames/<second>.jpg"), "indented under the folder");
        assert!(text.contains("../courses/MULT20015_2026_SM2/"));
        assert!(text.contains("17 slide changes were found"));
        assert!(
            !text.contains("transcript.vtt"),
            "the outline replaced the raw VTT, it did not join it"
        );
        assert!(text.contains("never more than 12"));
        assert!(text.contains("connect your laptop"), "the AV splash warning");
        assert!(!text.contains("no transcript"), "it has one");

        let silent = prompt(&Job { has_transcript: false, ..job });
        assert!(
            silent.contains("no transcript, so the outline is those markers"),
            "a lecture with no transcript should not be sent looking for words"
        );
    }

    #[test]
    fn the_outline_merges_the_slide_changes_into_the_transcript_in_play_order() {
        let cues = vec![
            TranscriptCue { start: 1.5, end: 4.0, text: "Good morning.".into() },
            TranscriptCue { start: 725.0, end: 728.0, text: "An equal superposition.".into() },
        ];
        let changes = vec![
            Candidate { seconds: 723, score: 42.1, diff: 39.1, pause: true },
            // Past the last spoken word: it must still reach the file.
            Candidate { seconds: 2400, score: 12.0, diff: 12.0, pause: false },
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
        let changes = vec![Candidate { seconds: 30, score: 9.0, diff: 9.0, pause: false }];
        let text = outline("Silent", &[], &changes);
        assert!(text.contains("     30  00:00:30  --- slide change · score 9.0 ---"), "{text}");
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
                // HH:MM:SS.mmm past the hour — and a silence is just a big
                // gap, however big.
                (3675, 3603.0),
            ]
        );
    }

    #[test]
    fn cue_gaps_ignores_headers_notes_and_blank_blocks() {
        let vtt = include_str!("../fixtures/chapters/sample.vtt");
        // WEBVTT, the NOTE block and the numbered cue identifiers all carry no
        // " --> ", so none of them becomes a gap.
        assert_eq!(cue_gaps(vtt).len(), 5);
        assert!(cue_gaps("WEBVTT\n\nnot a cue at all\n").is_empty());
    }

    #[test]
    fn cue_gaps_survives_crlf_and_a_bad_end_time() {
        let vtt = "WEBVTT\r\n\r\n00:00.000 --> broken\r\nhello\r\n\r\n00:10.000 --> 00:12.000\r\nworld\r\n";
        // The broken end falls back to its own start, so the next gap is 10 s
        // rather than the whole file.
        assert_eq!(cue_gaps(vtt), vec![(0, 0.0), (10, 10.0)]);
    }
}
