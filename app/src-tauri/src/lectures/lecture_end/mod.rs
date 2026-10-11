//! Where a lecture's planned content ends: the line the lecturer signs off on,
//! before the Q&A, packing up and dead air a recording runs on into.
//!
//! "The lecturer wrapping up" against "a student saying thanks" is a language
//! judgement, so a model reads the transcript's last 15 minutes in one
//! tool-less turn (`Harness::one_turn`) and cites a line; Rust checks the
//! citation against the transcript and stores the end of that line. A black
//! projector to the end of the file is passed on as a hint, never applied on
//! its own. See docs/chapters.md.

pub mod app;
mod picture;
mod prompt;
mod reply;
mod run;
#[cfg(test)]
mod tests;
mod window;

pub use picture::{black_tail, Tail};
pub use prompt::{prompt, Prompt, INSTRUCTIONS};
pub use reply::{ask, parse_reply, validate, Found, Reply};
pub use run::{claim, find, load, prepare, record, Lecture, Prepared};
pub use window::{clock, recording_length, transcript_lines, window, Line};

/// How much of the recording's end the model reads, and the picture decodes.
pub const TAIL_SECS: u32 = 900;
