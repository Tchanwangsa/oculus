//! Which stream to read: the first with enough change, else the other.

use super::decode::sample_diffs;
use super::score::candidates;
use super::{Candidate, DEAD_SOURCE};
use std::path::{Path, PathBuf};

/// The stream a run read, and what came out of reading it.
pub struct Detection {
    pub source: crate::sources::echo360::SourceNum,
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
    source: Option<crate::sources::echo360::SourceNum>,
    mut on_frame: impl FnMut(u32),
) -> Result<Detection, String> {
    // The file on disk, not the `video2_path` column: a stream downloaded but
    // never recorded is still readable.
    let second = crate::sources::echo360::source_path(lecture_dir, 2);

    let read = |path: PathBuf,
                source: crate::sources::echo360::SourceNum,
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
