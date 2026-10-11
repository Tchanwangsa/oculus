//! A clip run in windows against the same clip in one pass, on the real
//! model. Ignored by default, like `parity`; run one at a time:
//!
//! STUDIO_ALIGN_MODEL=<model.onnx> cargo test --lib studio::timing::tests::chunked -- --ignored --test-threads=1
//!
//! The second test also needs STUDIO_ALIGN_SPIKE=<data/spikes/studio-align>.
//! wav2vec2 attends over all the audio it is given, so a window's emissions
//! differ from the whole clip's everywhere, not only near its cuts: a word
//! edge can move by a frame. These tests bound how many and how far.

use std::path::{Path, PathBuf};

use super::super::align::align_windows;
use super::super::emissions::emissions;
use super::super::targets::VOCAB;
use super::super::windows::{frames, Windows};
use super::super::Alignment;
use crate::studio::audio::decode_wav;

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test")))
}

fn script(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

/// Word edges (starts and ends, in frames) that moved, and the furthest.
fn moved(whole: &Alignment, chunked: &Alignment) -> (usize, usize, usize) {
    assert_eq!(whole.words.len(), chunked.words.len());
    let frame = |s: f64| (s / super::super::FRAME_SECONDS).round() as i64;
    let mut count = 0;
    let mut worst = 0;
    for (a, b) in whole.words.iter().zip(&chunked.words) {
        for (x, y) in [(a.start, b.start), (a.end, b.end)] {
            let d = frame(x).abs_diff(frame(y)) as usize;
            count += usize::from(d > 0);
            worst = worst.max(d);
        }
    }
    (count, whole.words.len() * 2, worst)
}

/// The stitched emissions have the one-pass frame count, and the alignment
/// moves at most `max_moved` of the edges, none by more than a frame.
fn compare(model: &Path, pcm: &[f32], words: &[String], windows: &Windows, max_moved: f64) {
    let plan = windows.plan(pcm);
    assert!(plan.len() > 1, "{plan:?}");
    let em = emissions(model, pcm, windows).unwrap();
    let one = emissions(model, pcm, &Windows::whole()).unwrap();
    assert_eq!(em.len() / VOCAB, frames(pcm.len()));
    assert_eq!(em.len(), one.len());
    drop((em, one));

    let whole = align_windows(model, pcm, words, &Windows::whole()).unwrap();
    let chunked = align_windows(model, pcm, words, windows).unwrap();
    let (count, edges, worst) = moved(&whole, &chunked);
    let confidence = whole
        .words
        .iter()
        .zip(&chunked.words)
        .map(|(a, b)| (a.confidence - b.confidence).abs())
        .fold(0f64, f64::max);
    eprintln!(
        "{} windows {plan:?}: {count}/{edges} edges moved, worst {worst} frame(s), confidence within {confidence:.3}",
        plan.len()
    );
    assert!(worst <= 1, "an edge moved {worst} frames");
    assert!(
        count as f64 <= max_moved * edges as f64,
        "{count} of {edges} edges moved"
    );
    assert_eq!(whole.gaps, chunked.gaps);
}

#[test]
#[ignore = "needs STUDIO_ALIGN_MODEL (the fp32 wav2vec2 ONNX)"]
fn the_committed_clip_in_two_windows_keeps_its_frames_and_its_words() {
    let model = env_path("STUDIO_ALIGN_MODEL");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/studio");
    let wav = decode_wav(&std::fs::read(fixtures.join("fog.wav")).unwrap()).unwrap();
    let text = std::fs::read_to_string(fixtures.join("fog.txt")).unwrap();
    // 6.9 s in two windows of about 3.5 s, with 1 s of context.
    let windows = Windows {
        single_pass: 200,
        target: 200,
        search: 25,
        context: 50,
    };
    compare(&model, &wav.samples, &script(&text), &windows, 0.15);
}

#[test]
#[ignore = "needs STUDIO_ALIGN_MODEL and STUDIO_ALIGN_SPIKE (data/spikes/studio-align)"]
fn the_spikes_57_s_clip_in_the_default_windows_moves_few_edges() {
    let model = env_path("STUDIO_ALIGN_MODEL");
    let spike = env_path("STUDIO_ALIGN_SPIKE");
    let wav = decode_wav(&std::fs::read(spike.join("audio16/long_us_m.wav")).unwrap()).unwrap();
    let text = std::fs::read_to_string(spike.join("inputs/long_us_m.txt")).unwrap();
    compare(
        &model,
        &wav.samples,
        &script(&text),
        &Windows::DEFAULT,
        0.06,
    );
}
