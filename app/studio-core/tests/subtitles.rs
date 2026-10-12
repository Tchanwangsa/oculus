//! Subtitle cues from the `narrated` fixture's hand-written manifest.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

mod common;

use common::fixture;
use studio_core::{Project, captions};

#[test]
fn subtitles_are_the_phrases_in_video_time() {
    let project = Project::load(fixture("narrated")).unwrap();
    let cues = project.caption_cues().unwrap();
    assert_eq!(cues.len(), 1);
    assert_eq!(
        cues[0].lines,
        [
            "We follow the gradient downhill,",
            "one small step at a time."
        ]
    );
    // The first word at 0.25 s, shown 0.15 s before it; hidden 0.4 s after
    // the last word ends at 3.25 s.
    assert!((cues[0].start - 0.1).abs() < 1e-9, "{cues:?}");
    assert!((cues[0].end - 3.65).abs() < 1e-9, "{cues:?}");
    assert!(captions::vtt(&cues).starts_with("WEBVTT\n\n00:00:00.100 --> 00:00:03.650\n"));
}
