//! Lecture chapters: where a recording changes topic, and the agent job that
//! names them.
//!
//! Detection is visual: a slide capture is dead still between slides and a
//! cliff at a change, so one fixed threshold separates them (a room camera has
//! no such gap, which is why [`detect`] picks the stream rather than tuning).
//! Transcript pauses only nudge a candidate's score. Nothing is cached —
//! re-detecting is one fast decode. Measurements behind every constant here
//! are in `docs/chapters.md`.

mod agent;
pub mod app;
mod decode;
mod frames;
mod run;
mod score;
mod stream;
#[cfg(test)]
mod tests;
mod transcript;

pub use crate::lectures::lecture_jobs::Run;
pub(crate) use agent::parse_reply;
pub use agent::{hms, outline, parse_chapters, prompt, validate, Chapter, Job, MAX_CHAPTERS};
pub use decode::{sample_diffs, tail_luma};
pub use frames::{extract_frames, grab_frame, thumbnail_pick};
pub use run::{run, Outcome, Step};
pub use score::candidates;
pub use stream::{detect, Detection};
pub use transcript::{cue_gaps, parse_transcript, parse_transcript_voiced, TranscriptCue};

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
