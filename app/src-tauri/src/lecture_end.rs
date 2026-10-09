//! Where a lecture's planned content ends: the line the lecturer signs off on,
//! before the Q&A, packing up and dead air a recording runs on into.
//!
//! "The lecturer wrapping up" against "a student saying thanks" is a language
//! judgement, so a model reads the transcript's last 15 minutes in one
//! tool-less turn (`Harness::one_turn`) and cites a line; Rust checks the
//! citation against the transcript and stores the end of that line. A black
//! projector to the end of the file is passed on as a hint, never applied on
//! its own. See docs/chapters.md.

use std::path::{Path, PathBuf};

use crate::chapters::TranscriptCue;
use crate::harness::jobs::JobSelection;
use crate::harness::{opencode, Harness};

/// How much of the recording's end the model reads, and the picture decodes.
pub const TAIL_SECS: u32 = 900;

/// A frame darker than this (mean of 0–255) is black.
const BLACK_LUMA: f32 = 10.0;
/// A black run shorter than this is a cut or a fade, not the projector off.
const MIN_BLACK_SECS: usize = 60;
/// Less picture than this in the whole tail is a dead capture, not an end.
const MIN_PICTURE_SECS: usize = 60;

/// The one-off brief: Claude's appended system prompt, Codex's developer
/// instructions, opencode's `oculus-lecture-end` agent, ahead of agy's message.
pub const INSTRUCTIONS: &str = "You find where a university lecture recording's planned content \
ends.\n\n\
You are shown the last 15 minutes of an automatic transcript, one line per cue: \
`second  clock  speaker: text`. A speaker is named only where the voice changes, and not at \
all when only one is heard. The second and the clock are the same moment.\n\n\
Recordings keep running after the lecturer finishes — students come to the lectern with \
questions, people pack up and chat — and the microphone keeps transcribing. The end is the line \
where the lecturer finishes the lecture: their closing words or wrap-up. It is not a student \
thanking them, and not \"I'll stop the recording\" said after a Q&A that followed an earlier \
wrap-up — then the end is that earlier wrap-up. A Q&A after the close is not lecture, even if it \
is long. A note that the projector goes black says the lecture is over by then; the end is still \
a line of the transcript. If the lecturer is still teaching at the last line, the recording was \
cut off and there is no end.\n\n\
Reply with only JSON — no prose, no tools:\n\
{\"ends_at\": <that line's first column, as a bare number like 1911>, \"quote\": \"<3 to 12 \
words copied exactly from that line>\"}\n\
or, when there is no end:\n\
{\"ends_at\": null, \"quote\": null}";

// ── The window ───────────────────────────────────────────────────────────────

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

// ── The picture hint ─────────────────────────────────────────────────────────

/// What a source's tail shows, from one mean brightness per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tail {
    /// Black from this index to the last frame, after real picture.
    Black(usize),
    /// Picture to the end, or a black run too short to count.
    Picture,
    /// Too little picture in the whole tail: a failed capture says nothing.
    Dead,
}

/// Whether the picture goes black for good: a run under [`BLACK_LUMA`] of at
/// least [`MIN_BLACK_SECS`] that lasts to the last frame, with at least
/// [`MIN_PICTURE_SECS`] of picture before it.
pub fn black_tail(luma: &[f32]) -> Tail {
    let run = luma.iter().rev().take_while(|&&l| l < BLACK_LUMA).count();
    let from = luma.len() - run;
    let picture = luma[..from].iter().filter(|&&l| l >= BLACK_LUMA).count();
    if picture < MIN_PICTURE_SECS {
        Tail::Dead
    } else if run >= MIN_BLACK_SECS {
        Tail::Black(from)
    } else {
        Tail::Picture
    }
}

/// The second the projector goes black for good, from the first source that
/// is not dead. A decode that fails is no hint, never an error: the job needs
/// only the transcript.
fn black_from(ffmpeg: &Path, sources: &[PathBuf], length: u32) -> Option<u32> {
    for video in sources {
        let (luma, file_secs) = match crate::chapters::tail_luma(ffmpeg, video, TAIL_SECS) {
            Ok(decoded) => decoded,
            Err(e) => {
                eprintln!("[oculus] lecture end: {}: {e}", video.display());
                continue;
            }
        };
        // Frames end where the file does, which can run seconds past the row's duration.
        let file_end = file_secs.map_or(length, |s| s.round() as u32);
        let tail_start = file_end.saturating_sub(luma.len() as u32);
        match black_tail(&luma) {
            Tail::Black(at) => return Some(tail_start + at as u32),
            Tail::Picture => return None,
            Tail::Dead => continue,
        }
    }
    None
}

// ── The prompt ───────────────────────────────────────────────────────────────

pub struct Prompt<'a> {
    pub title: &'a str,
    pub code: Option<&'a str>,
    pub length: u32,
    pub black_from: Option<u32>,
    pub lines: &'a [Line],
}

/// The turn's message: the lecture, its length, the optional black-projector
/// line, then the window's transcript inline — no files to open.
pub fn prompt(p: &Prompt) -> String {
    let mut out = format!("Lecture: {}\n", p.title);
    if let Some(code) = p.code {
        out.push_str(&format!("Course: {code}\n"));
    }
    out.push_str(&format!(
        "Recording length: {} ({} s)\n",
        clock(p.length),
        p.length
    ));
    if let Some(at) = p.black_from {
        out.push_str(&format!(
            "The projector goes black from {} ({at}) to the end of the recording.\n",
            clock(at)
        ));
    }
    let heading = if p.length > TAIL_SECS {
        "The transcript's last 15 minutes:"
    } else {
        "The whole transcript:"
    };
    out.push_str(&format!("\n{heading}\n\n{}\n", transcript_lines(p.lines)));
    out
}

// ── The reply ────────────────────────────────────────────────────────────────

/// What the model said, before validation.
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub ends_at: Option<f64>,
    pub quote: Option<String>,
}

/// The `{ends_at, quote}` object out of whatever the model said: prose,
/// fences and an envelope around it are tolerated, as in `chapters`.
pub fn parse_reply(reply: &str) -> Result<Reply, String> {
    crate::chapters::parse_reply(reply, "{\"ends_at\", \"quote\"} object", decode)
}

fn decode(text: &str) -> Option<Reply> {
    use serde_json::Value;
    let value: Value = serde_json::from_str(text).ok()?;
    let object = value.as_object()?;
    let key = |o: &serde_json::Map<String, Value>| {
        ["ends_at", "endsAt", "end"]
            .into_iter()
            .find(|k| o.contains_key(*k))
    };
    // An envelope: the one object inside that has the key.
    let object = match key(object) {
        Some(_) => object,
        None => object
            .values()
            .filter_map(Value::as_object)
            .find(|o| key(o).is_some())?,
    };
    let ends_at = match &object[key(object)?] {
        Value::Null => None,
        Value::Number(n) => Some(n.as_f64()?),
        // "1911 31:51": the first column copied with the clock beside it.
        Value::String(s) => Some(s.split_whitespace().next()?.parse::<f64>().ok()?),
        _ => return None,
    };
    let quote = match object.get("quote") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        Some(_) => return None,
    };
    Some(Reply { ends_at, quote })
}

/// A validated end.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// The start second of the line the model cited.
    pub cue_start: u32,
    /// What is stored: the end of the line the quote finishes in.
    pub end: u32,
    pub quote: String,
}

/// Lowercase words: punctuation becomes a space, whitespace collapses, so
/// "I'll stop here." and "i ll stop here" compare equal.
fn normalise(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Where `quote` (normalised) occurs touching line `at`, with the lines
/// either side joined on: the index of the last line it reaches.
fn quote_ends_in(lines: &[Line], at: usize, quote: &str) -> Option<usize> {
    let first = at.saturating_sub(1);
    let last = (at + 1).min(lines.len() - 1);
    let mut haystack = String::from(" ");
    let mut spans = Vec::new();
    for index in first..=last {
        let start = haystack.len();
        haystack.push_str(&normalise(&lines[index].text));
        spans.push((index, start, haystack.len()));
        haystack.push(' ');
    }
    let needle = format!(" {quote} ");
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(&needle) {
        let begin = from + offset + 1;
        let end = begin + quote.len();
        let touched: Vec<usize> = spans
            .iter()
            .filter(|(_, s, e)| begin < *e && end > *s)
            .map(|(index, ..)| *index)
            .collect();
        if touched.contains(&at) {
            return touched.last().copied();
        }
        from = begin;
    }
    None
}

/// How far from the cited second the quote may sit. Small models cite the
/// line before the one they quote; the quote, not the second, is the evidence.
const NEAR_SECS: u32 = 30;

/// Whether a reply may be stored. The quote must be words of a line starting
/// within [`NEAR_SECS`] of `ends_at` (or run from it into the next line); the
/// nearest such line wins. Null for both is "no end".
pub fn validate(reply: &Reply, lines: &[Line]) -> Result<Option<Found>, String> {
    let (at, quote) = match (reply.ends_at, &reply.quote) {
        (None, None) => return Ok(None),
        (None, Some(_)) => {
            return Err(
                "ends_at is null but a quote was given — reply null for both, or cite the line"
                    .into(),
            )
        }
        (Some(at), None) => {
            return Err(format!(
                "ends_at {at} has no quote — copy 3 to 12 words from that line"
            ))
        }
        (Some(at), Some(quote)) => (at, quote),
    };
    let (first, last) = match (lines.first(), lines.last()) {
        (Some(f), Some(l)) => (f.start, l.end),
        _ => return Err("there is no transcript to cite".into()),
    };
    if !at.is_finite() || at < first as f64 || at > last as f64 {
        return Err(format!(
            "ends_at {at} is not a second of the transcript shown"
        ));
    }
    let second = at.round() as u32;
    let words = normalise(quote);
    if words.is_empty() {
        return Err(format!("the quote {quote:?} has no words in it"));
    }
    let mut near: Vec<usize> = (0..lines.len())
        .filter(|&i| lines[i].start.abs_diff(second) <= NEAR_SECS)
        .collect();
    near.sort_by_key(|&i| lines[i].start.abs_diff(second));
    for index in near {
        if let Some(last) = quote_ends_in(lines, index, &words) {
            return Ok(Some(Found {
                cue_start: lines[index].start,
                end: lines[last].end,
                quote: quote.clone(),
            }));
        }
    }
    let elsewhere = (0..lines.len()).find(|&i| quote_ends_in(lines, i, &words).is_some());
    Err(match elsewhere {
        Some(i) => format!(
            "the quote {quote:?} is not near {second}; it is in the line at {}",
            lines[i].start
        ),
        None => format!(
            "the quote {quote:?} is not in the transcript near {second} — copy the words exactly"
        ),
    })
}

/// One turn, parsed and validated; a rejected reply is asked again once with
/// the reason appended. `turn` is the agent call (a fake one in tests). A
/// provider failure is not retried.
pub fn ask(
    mut turn: impl FnMut(&str) -> Result<String, String>,
    prompt: &str,
    lines: &[Line],
) -> Result<Option<Found>, String> {
    let mut failure = String::new();
    for attempt in 0..2 {
        let text = if attempt == 0 {
            prompt.to_string()
        } else {
            format!(
                "{prompt}\nYour previous reply was rejected: {failure}\n\
                 Reply again with only the JSON object."
            )
        };
        let reply = turn(&text)?;
        match parse_reply(&reply).and_then(|r| validate(&r, lines)) {
            Ok(found) => return Ok(found),
            Err(e) => failure = e,
        }
    }
    Err(format!("the reply was rejected twice: {failure}"))
}

// ── Running it ───────────────────────────────────────────────────────────────

/// A lecture's inputs, checked before anything is claimed so a refusal leaves
/// no status behind.
pub struct Lecture {
    pub id: String,
    pub title: String,
    pub code: Option<String>,
    pub duration: u32,
    pub transcript: PathBuf,
    /// Downloaded streams, source 1 first.
    pub videos: Vec<PathBuf>,
}

/// Reads only columns that predate migration 42, so `--dry-run` works on a
/// database without it.
pub async fn load(pool: &sqlx::SqlitePool, data_dir: &Path, id: &str) -> Result<Lecture, String> {
    let source = crate::lecture_jobs::source(pool, id).await?;
    let transcript = source
        .transcript
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!(
                "{} has no transcript — `oculus run -l --videos` downloads it",
                source.title
            )
        })?;
    if !transcript.exists() {
        return Err(format!(
            "{} is on record but missing from disk",
            transcript.display()
        ));
    }
    let dir = crate::echo360::lecture_dir(data_dir, id);
    // The column, else the stream's own path on disk (as `chapters::detect` does).
    let first = source
        .video
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::echo360::source_path(&dir, 1));
    let videos = [first, crate::echo360::source_path(&dir, 2)]
        .into_iter()
        .filter(|path| path.exists())
        .collect();
    Ok(Lecture {
        id: id.to_string(),
        title: source.title,
        code: source.code,
        duration: source.duration,
        transcript,
        videos,
    })
}

/// Everything the turn needs, built before it.
pub struct Prepared {
    pub length: u32,
    pub lines: Vec<Line>,
    pub black_from: Option<u32>,
    pub prompt: String,
}

/// Read the transcript, cut the window, decode the picture's tail.
pub fn prepare(lecture: &Lecture) -> Result<Prepared, String> {
    let vtt = std::fs::read_to_string(&lecture.transcript)
        .map_err(|e| format!("{}: {e}", lecture.transcript.display()))?;
    let cues = crate::chapters::parse_transcript_voiced(&vtt);
    if cues.is_empty() {
        return Err(format!(
            "{} contains no transcript cues",
            lecture.transcript.display()
        ));
    }
    let length = recording_length(lecture.duration, &cues);
    let lines = window(&cues, length);
    if lines.is_empty() {
        return Err(format!(
            "{} has no speech in its last 15 minutes",
            lecture.title
        ));
    }
    let black_from = match crate::echo360::find_ffmpeg(None) {
        Some(ffmpeg) if !lecture.videos.is_empty() => black_from(&ffmpeg, &lecture.videos, length),
        _ => None,
    };
    let prompt = prompt(&Prompt {
        title: &lecture.title,
        code: lecture.code.as_deref(),
        length,
        black_from,
        lines: &lines,
    });
    Ok(Prepared {
        length,
        lines,
        black_from,
        prompt,
    })
}

/// Ask the job's agent where `prepared`'s lecture ends.
pub fn find(
    harness: &Harness,
    selection: &JobSelection,
    prepared: &Prepared,
) -> Result<Option<Found>, String> {
    ask(
        |text| harness.one_turn(selection, INSTRUCTIONS, opencode::LECTURE_END_AGENT, text),
        &prepared.prompt,
        &prepared.lines,
    )
}

/// Write a run's outcome; answers whether a found end marked the lecture Done.
pub async fn record(
    pool: &sqlx::SqlitePool,
    lecture_id: &str,
    outcome: &Result<Option<Found>, String>,
) -> Result<bool, String> {
    match outcome {
        Ok(found) => {
            let end = found.as_ref().map(|f| (f.end, f.quote.as_str()));
            crate::store::save_content_end(pool, lecture_id, end).await
        }
        Err(e) => crate::store::set_content_end_error(pool, lecture_id, e)
            .await
            .map(|()| false),
    }
}

/// Claim `lecture` for a run, or say why not. `rerun` names how to replace a
/// standing result (the CLI's `--force`, the app's re-run).
pub async fn claim(
    pool: &sqlx::SqlitePool,
    lecture: &Lecture,
    force: bool,
    rerun: &str,
) -> Result<(), String> {
    use crate::store::EndClaim;
    match crate::store::claim_content_end(pool, &lecture.id, force).await? {
        EndClaim::Claimed => Ok(()),
        EndClaim::Running => Err(format!("{}'s end is already being found", lecture.title)),
        EndClaim::Found => Err(format!(
            "{}'s end is already found — {rerun}",
            lecture.title
        )),
        EndClaim::NoLecture => Err(format!("no lecture {}", lecture.id)),
    }
}

// ── Tauri ────────────────────────────────────────────────────────────────────

pub mod app {
    use super::*;
    use crate::harness::app::HarnessState;
    use crate::lecture_jobs::{reconcile_status, spawn_job};
    use tauri::{AppHandle, Emitter, State};

    /// Emitted once when a run ends.
    pub const LECTURE_END_EVENT: &str = "lecture-end";

    #[derive(serde::Serialize, Clone)]
    #[serde(rename_all = "camelCase")]
    struct Finished {
        lecture_id: String,
        /// `ready` | `none` | `error`.
        status: &'static str,
        seconds: Option<u32>,
        quote: Option<String>,
        error: Option<String>,
    }

    /// Find where a lecture's content ends on the `lectureEnd` job's agent,
    /// through the app's own harness (Codex and opencode reuse their running
    /// server). Returns once claimed; the end arrives as [`LECTURE_END_EVENT`].
    #[tauri::command]
    pub async fn lecture_find_end(
        app: AppHandle,
        state: State<'_, HarnessState>,
        lecture_id: String,
        force: Option<bool>,
    ) -> Result<(), String> {
        let pool = crate::store::open_pool().await?;
        let lecture = load(&pool, &crate::paths::data_dir(), &lecture_id).await?;
        claim(
            &pool,
            &lecture,
            force.unwrap_or(false),
            "re-running replaces it",
        )
        .await?;

        let harness = state.harness.clone();
        spawn_job(
            "lecture end",
            crate::harness::jobs::Job::LectureEnd,
            move |rt, pool, selection| {
                let outcome = prepare(&lecture).and_then(|p| find(&harness, &selection, &p));
                let finished = match rt
                    .block_on(record(pool, &lecture.id, &outcome))
                    .and(outcome)
                {
                    Ok(found) => Finished {
                        lecture_id: lecture.id.clone(),
                        status: if found.is_some() { "ready" } else { "none" },
                        seconds: found.as_ref().map(|f| f.end),
                        quote: found.map(|f| f.quote),
                        error: None,
                    },
                    Err(e) => {
                        eprintln!("[oculus] lecture end: {e}");
                        Finished {
                            lecture_id: lecture.id.clone(),
                            status: "error",
                            seconds: None,
                            quote: None,
                            error: Some(e),
                        }
                    }
                };
                app.emit(LECTURE_END_EVENT, finished).ok();
            },
        );
        Ok(())
    }

    /// Startup: clear `running` left by a killed run.
    pub fn reconcile(_app: &AppHandle) {
        reconcile_status("lecture end", |pool| async move {
            crate::store::reconcile_content_end_status(&pool).await
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f32, end: f32, speaker: &str, text: &str) -> (TranscriptCue, Option<String>) {
        (
            TranscriptCue {
                start,
                end,
                text: text.into(),
            },
            Some(speaker.to_string()).filter(|s| !s.is_empty()),
        )
    }

    fn line(start: u32, end: u32, text: &str) -> Line {
        Line {
            start,
            end,
            speaker: Some("Speaker 0".into()),
            text: text.into(),
        }
    }

    fn reply(ends_at: Option<f64>, quote: Option<&str>) -> Reply {
        Reply {
            ends_at,
            quote: quote.map(str::to_string),
        }
    }

    /// A sign-off split over two cues, then a student's question.
    fn sign_off() -> Vec<Line> {
        vec![
            line(1890, 1894, "So that is the proof of the theorem."),
            line(1895, 1899, "OK, that's it for today, thank you,"),
            line(1899, 1902, "and I'll see you all on Thursday."),
            line(1905, 1909, "Thanks! Can I ask about question three?"),
        ]
    }

    #[test]
    fn the_window_is_the_last_fifteen_minutes_of_the_longer_length() {
        let cues = vec![
            cue(10.0, 12.0, "Speaker 0", "Hello."),
            cue(1199.5, 1201.0, "Speaker 0", "Before the window."),
            cue(1200.4, 1203.9, "Speaker 0", "First in the window."),
            cue(2095.2, 2101.7, "Speaker 1", "Last cue."),
        ];
        assert_eq!(
            recording_length(2000, &cues),
            2101,
            "the last cue outruns the row"
        );
        assert_eq!(recording_length(2400, &cues), 2400);
        assert_eq!(
            recording_length(0, &cues),
            2101,
            "an unknown duration takes the cues'"
        );
        let lines = window(&cues, 2100);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], line(1200, 1203, "First in the window."));
        assert_eq!(lines[1].speaker.as_deref(), Some("Speaker 1"));
        assert_eq!(
            window(&cues, 600).len(),
            4,
            "a short recording is read whole"
        );
    }

    #[test]
    fn lines_print_both_columns_and_drop_a_lone_speaker() {
        assert_eq!(
            (clock(65), clock(3725)),
            ("01:05".to_string(), "1:02:05".to_string())
        );
        let mut lines = vec![
            line(1915, 1918, "See you tomorrow."),
            line(3725, 3727, "Bye."),
        ];
        assert_eq!(
            transcript_lines(&lines),
            "1915  31:55  See you tomorrow.\n3725  1:02:05  Bye."
        );
        lines[1].speaker = Some("Speaker 1".into());
        assert_eq!(
            transcript_lines(&lines),
            "1915  31:55  Speaker 0: See you tomorrow.\n3725  1:02:05  Speaker 1: Bye."
        );
        let mut again = line(3728, 3729, "Thanks.");
        again.speaker = Some("Speaker 1".into());
        lines.push(again);
        assert!(transcript_lines(&lines).ends_with("Speaker 1: Bye.\n3728  1:02:08  Thanks."));
    }

    #[test]
    fn the_prompt_carries_the_length_the_hint_and_the_lines() {
        let lines = sign_off();
        let text = prompt(&Prompt {
            title: "Lecture 12",
            code: Some("COMP30026"),
            length: 2290,
            black_from: Some(1925),
            lines: &lines,
        });
        assert!(text.starts_with(
            "Lecture: Lecture 12\nCourse: COMP30026\nRecording length: 38:10 (2290 s)\n"
        ));
        assert!(text
            .contains("The projector goes black from 32:05 (1925) to the end of the recording.\n"));
        assert!(text.contains("1899  31:39  and I'll see you all on Thursday."));
        let bare = prompt(&Prompt {
            title: "T",
            code: None,
            length: 600,
            black_from: None,
            lines: &lines,
        });
        assert!(!bare.contains("Course:") && !bare.contains("projector"));
        assert!(bare.contains("The whole transcript:"));
    }

    fn luma(picture: usize, black: usize, tail_picture: usize) -> Vec<f32> {
        let mut out = vec![120.0; picture];
        out.extend(std::iter::repeat(3.0).take(black));
        out.extend(std::iter::repeat(90.0).take(tail_picture));
        out
    }

    #[test]
    fn a_black_run_counts_only_to_the_end_and_after_real_picture() {
        assert_eq!(black_tail(&luma(500, 400, 0)), Tail::Black(500));
        assert_eq!(
            black_tail(&luma(0, 900, 0)),
            Tail::Dead,
            "black from the window's start"
        );
        assert_eq!(
            black_tail(&luma(30, 870, 0)),
            Tail::Dead,
            "too little picture before it"
        );
        assert_eq!(
            black_tail(&luma(860, 40, 0)),
            Tail::Picture,
            "too short to be the projector off"
        );
        assert_eq!(
            black_tail(&luma(400, 300, 200)),
            Tail::Picture,
            "picture comes back"
        );
        assert_eq!(black_tail(&luma(900, 0, 0)), Tail::Picture);
        assert_eq!(black_tail(&[]), Tail::Dead);
    }

    #[test]
    fn replies_parse_through_fences_prose_floats_and_nulls() {
        let fenced = "```json\n{\"ends_at\": 1895, \"quote\": \"that's it for today\"}\n```";
        assert_eq!(
            parse_reply(fenced).unwrap(),
            reply(Some(1895.0), Some("that's it for today"))
        );
        let prose = "Looking at the tail, the lecturer wraps up here:\n{\"ends_at\": 1895.0, \"quote\": \"thank you\"} — after that it is Q&A.";
        assert_eq!(
            parse_reply(prose).unwrap(),
            reply(Some(1895.0), Some("thank you"))
        );
        assert_eq!(
            parse_reply("{\"ends_at\": \"1899\", \"quote\": \"see you\"}")
                .unwrap()
                .ends_at,
            Some(1899.0)
        );
        assert_eq!(
            parse_reply("{\"ends_at\": \"1911 31:51\", \"quote\": \"see you\"}")
                .unwrap()
                .ends_at,
            Some(1911.0)
        );
        assert_eq!(
            parse_reply("{\"result\": {\"ends_at\": 12.5, \"quote\": \"bye\"}}").unwrap(),
            reply(Some(12.5), Some("bye"))
        );
        assert_eq!(
            parse_reply("{\"ends_at\": null, \"quote\": null}").unwrap(),
            reply(None, None)
        );
        assert_eq!(
            parse_reply("{\"ends_at\": null}").unwrap(),
            reply(None, None)
        );
        assert!(parse_reply("The lecture ends at 31:35.").is_err());
        assert!(parse_reply("{\"start\": 3}").is_err());
    }

    #[test]
    fn validation_takes_the_end_of_the_cited_line() {
        let lines = sign_off();
        let found = validate(&reply(Some(1895.0), Some("That's it for today")), &lines).unwrap();
        assert_eq!(
            found,
            Some(Found {
                cue_start: 1895,
                end: 1899,
                quote: "That's it for today".into()
            })
        );
        let rounded = validate(&reply(Some(1895.4), Some("thank you")), &lines)
            .unwrap()
            .unwrap();
        assert_eq!(rounded.end, 1899);
        assert_eq!(validate(&reply(None, None), &lines).unwrap(), None);
    }

    #[test]
    fn a_quote_running_into_the_next_line_ends_with_that_line() {
        let lines = sign_off();
        let found = validate(
            &reply(Some(1895.0), Some("thank you, and I'll see you all")),
            &lines,
        )
        .unwrap()
        .unwrap();
        assert_eq!((found.cue_start, found.end), (1895, 1902));
        // Two cues share second 1899: one ends there, one starts there.
        let shared = validate(
            &reply(Some(1899.0), Some("see you all on Thursday")),
            &lines,
        )
        .unwrap()
        .unwrap();
        assert_eq!((shared.cue_start, shared.end), (1899, 1902));
    }

    #[test]
    fn validation_rejects_what_the_transcript_does_not_say() {
        let mut lines = sign_off();
        // A second beside the quoted line still finds it: the quote is the evidence.
        let beside = validate(&reply(Some(1890.0), Some("that's it for today")), &lines)
            .unwrap()
            .unwrap();
        assert_eq!((beside.cue_start, beside.end), (1895, 1899));
        let wrong = validate(&reply(Some(1895.0), Some("see you next week")), &lines).unwrap_err();
        assert!(wrong.contains("not in the transcript near 1895"), "{wrong}");
        lines.push(line(1960, 1963, "See you next week."));
        let far = validate(&reply(Some(1895.0), Some("see you next week")), &lines).unwrap_err();
        assert!(far.contains("it is in the line at 1960"), "{far}");
        assert!(validate(&reply(Some(5000.0), Some("thank you")), &lines).is_err());
        assert!(validate(&reply(Some(1895.0), None), &lines).is_err());
        assert!(validate(&reply(None, Some("thank you")), &lines).is_err());
        assert!(validate(&reply(Some(1895.0), Some("...")), &lines).is_err());
        assert!(validate(&reply(Some(-3.0), Some("thank you")), &lines).is_err());
        // Words, not substrings.
        assert!(validate(&reply(Some(1890.0), Some("theorem s")), &lines).is_err());
    }

    #[test]
    fn a_rejected_reply_is_asked_again_once_with_the_reason() {
        let lines = sign_off();
        let mut seen: Vec<String> = Vec::new();
        let replies = [
            "{\"ends_at\": 1895, \"quote\": \"see you next week\"}",
            "{\"ends_at\": 1895, \"quote\": \"thank you\"}",
        ];
        let found = ask(
            |text| {
                seen.push(text.to_string());
                Ok(replies[seen.len() - 1].to_string())
            },
            "PROMPT",
            &lines,
        )
        .unwrap();
        assert_eq!(found.map(|f| f.end), Some(1899));
        assert_eq!(seen.len(), 2);
        assert!(
            seen[1].starts_with(
                "PROMPT\nYour previous reply was rejected: the quote \"see you next week\" is not"
            ),
            "{}",
            seen[1]
        );

        let twice = ask(|_| Ok("no idea".to_string()), "PROMPT", &lines).unwrap_err();
        assert!(twice.starts_with("the reply was rejected twice"), "{twice}");
        let mut calls = 0;
        let down = ask(
            |_| {
                calls += 1;
                Err("not signed in".to_string())
            },
            "PROMPT",
            &lines,
        );
        assert_eq!(
            (down, calls),
            (Err("not signed in".to_string()), 1),
            "a provider failure is not retried"
        );
    }

    #[test]
    fn voice_tags_name_the_speaker_and_leave_the_text_plain() {
        let vtt = "WEBVTT\n\n00:31:35.000 --> 00:31:38.500\n<v Speaker 0>That's it for today.\n\n\
                   NOTE CONF {\"raw\":[99]}\n\n31:40.000 --> 31:42.000\n<v.loud Speaker 1>Thanks!\n\n\
                   31:43.000 --> 31:44.000\nNo tag here.\n";
        let cues = crate::chapters::parse_transcript_voiced(vtt);
        assert_eq!(cues.len(), 3);
        assert_eq!(cues[0].0.text, "That's it for today.");
        assert_eq!(cues[0].1.as_deref(), Some("Speaker 0"));
        assert_eq!(cues[1].1.as_deref(), Some("Speaker 1"));
        assert_eq!(cues[2].1, None);
        assert_eq!(crate::chapters::parse_transcript(vtt).len(), 3);
    }
}
