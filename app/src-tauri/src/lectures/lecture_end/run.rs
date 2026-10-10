//! Running it: a lecture's inputs, one ask, and the claim and record around it.

use super::picture::black_from;
use super::prompt::{prompt, Prompt, INSTRUCTIONS};
use super::reply::{ask, Found};
use super::window::{recording_length, window, Line};
use crate::harness::jobs::JobSelection;
use crate::harness::{opencode, Harness};
use std::path::{Path, PathBuf};

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
    let source = crate::lectures::lecture_jobs::source(pool, id).await?;
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
    let dir = crate::sources::echo360::lecture_dir(data_dir, id);
    // The column, else the stream's own path on disk (as `chapters::detect` does).
    let first = source
        .video
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::sources::echo360::source_path(&dir, 1));
    let videos = [first, crate::sources::echo360::source_path(&dir, 2)]
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
    let cues = crate::lectures::chapters::parse_transcript_voiced(&vtt);
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
    let black_from = match crate::sources::echo360::find_ffmpeg(None) {
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
            crate::db::store::save_content_end(pool, lecture_id, end).await
        }
        Err(e) => crate::db::store::set_content_end_error(pool, lecture_id, e)
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
    use crate::db::store::EndClaim;
    match crate::db::store::claim_content_end(pool, &lecture.id, force).await? {
        EndClaim::Claimed => Ok(()),
        EndClaim::Running => Err(format!("{}'s end is already being found", lecture.title)),
        EndClaim::Found => Err(format!(
            "{}'s end is already found — {rerun}",
            lecture.title
        )),
        EndClaim::NoLecture => Err(format!("no lecture {}", lecture.id)),
    }
}
