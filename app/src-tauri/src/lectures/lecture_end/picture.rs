//! The picture hint: whether a source ends in a black projector.

use super::TAIL_SECS;
use std::path::{Path, PathBuf};

/// A frame darker than this (mean of 0–255) is black.
const BLACK_LUMA: f32 = 10.0;
/// A black run shorter than this is a cut or a fade, not the projector off.
const MIN_BLACK_SECS: usize = 60;
/// Less picture than this in the whole tail is a dead capture, not an end.
const MIN_PICTURE_SECS: usize = 60;

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
pub(super) fn black_from(ffmpeg: &Path, sources: &[PathBuf], length: u32) -> Option<u32> {
    for video in sources {
        let (luma, file_secs) = match crate::lectures::chapters::tail_luma(ffmpeg, video, TAIL_SECS)
        {
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
