//! Parity with the alignment spike's Python reference, on the real model.
//! Ignored by default (the model is 378 MB and never downloaded by tests);
//! run one at a time, since a session holds up to a gigabyte:
//!
//! STUDIO_ALIGN_MODEL=<model.onnx> cargo test --lib studio::timing::tests::parity -- --ignored --test-threads=1
//!
//! The second test also needs STUDIO_ALIGN_SPIKE=<data/spikes/studio-align>.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::Value;

use super::super::{align_with, Alignment, FRAME_SECONDS, START_BIAS};
use crate::studio::audio::{decode_wav, resample_to_16k, ALIGN_RATE};

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run this test")))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/studio")
        .join(name)
}

fn script(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Every word lands on the reference's frames, and its confidence agrees to
/// `tolerance` (ORT 1.28 here vs the reference's runtime).
fn assert_matches(out: &Alignment, expected: &Value, tolerance: f64) {
    let want = expected["words"].as_array().unwrap();
    assert_eq!(out.words.len(), want.len());
    let mut worst = 0f64;
    for (got, want) in out.words.iter().zip(want) {
        let start_frame = want["start_frame"].as_u64().unwrap() as f64;
        let end_frame = want["end_frame"].as_u64().unwrap() as f64;
        let start = (start_frame * FRAME_SECONDS + START_BIAS).max(0.0);
        let end = (end_frame + 1.0) * FRAME_SECONDS;
        assert!(
            (got.start - start).abs() < 1e-9 && (got.end - end).abs() < 1e-9,
            "{}: {}-{} s, reference frames {start_frame}-{end_frame}",
            got.text,
            got.start,
            got.end
        );
        let diff = (got.confidence - want["mean_logp"].as_f64().unwrap()).abs();
        worst = worst.max(diff);
    }
    assert!(worst < tolerance, "confidence differs by {worst}");
    eprintln!(
        "{} words, 0 frame differences, confidence within {worst:.2e}",
        out.words.len()
    );
}

#[test]
#[ignore = "needs STUDIO_ALIGN_MODEL (the fp32 wav2vec2 ONNX)"]
fn the_committed_clip_aligns_frame_for_frame_with_python() {
    let model = env_path("STUDIO_ALIGN_MODEL");
    let wav = decode_wav(&std::fs::read(fixture("fog.wav")).unwrap()).unwrap();
    assert_eq!(wav.sample_rate, ALIGN_RATE);
    let text = std::fs::read_to_string(fixture("fog.txt")).unwrap();
    let expected = read_json(&fixture("fog.expected.json"));

    let started = Instant::now();
    let out = align_with(&model, &wav.samples, &script(&text)).unwrap();
    eprintln!(
        "{:.2} s clip aligned in {:.3} s",
        wav.duration,
        started.elapsed().as_secs_f64()
    );
    assert_matches(&out, &expected, 1e-3);
    assert!(out.gaps.is_empty(), "{:?}", out.gaps);
    assert!(out.words.iter().all(|w| w.confidence > -2.0));
    assert_eq!(out.words[1].text, "you're");

    // A dash in the script is left out and the rest keep their times, with
    // indices into the script as written.
    let mut dashed = script(&text);
    dashed.insert(6, "—".into());
    let with_dash = align_with(&model, &wav.samples, &dashed).unwrap();
    assert_eq!(with_dash.words.len(), out.words.len());
    assert_eq!(with_dash.words[6].index, 7);
    assert_eq!(with_dash.words[6].text, "You");
    for (a, b) in with_dash.words.iter().zip(&out.words) {
        assert_eq!((a.start, a.end), (b.start, b.end));
    }
}

#[test]
#[ignore = "needs STUDIO_ALIGN_MODEL and STUDIO_ALIGN_SPIKE (data/spikes/studio-align)"]
fn the_spikes_full_clip_aligns_as_it_did_there() {
    let model = env_path("STUDIO_ALIGN_MODEL");
    let spike = env_path("STUDIO_ALIGN_SPIKE");
    let clip = "us_f__p2_fog";
    let text = std::fs::read_to_string(spike.join(format!("inputs/{clip}.txt"))).unwrap();

    // ffmpeg's 16 kHz file against Python onnxruntime on the same model.
    let wav =
        decode_wav(&std::fs::read(spike.join(format!("audio16/{clip}.wav"))).unwrap()).unwrap();
    let started = Instant::now();
    let out = align_with(&model, &wav.samples, &script(&text)).unwrap();
    eprintln!(
        "{:.2} s clip aligned in {:.3} s",
        wav.duration,
        started.elapsed().as_secs_f64()
    );
    let python = read_json(&spike.join(format!("out/var_fp32/{clip}.json")));
    assert_matches(&out, &python, 1e-3);

    // The server's 24 kHz WAV through this crate's decode and resample,
    // against the spike's own hound + rubato route.
    let source =
        decode_wav(&std::fs::read(spike.join(format!("audio/{clip}.wav"))).unwrap()).unwrap();
    assert_eq!(source.sample_rate, 24_000);
    let pcm = resample_to_16k(&source.samples, source.sample_rate).unwrap();
    let routed = read_json(&spike.join(format!("out/route_wav/{clip}.json")));
    assert_eq!(pcm.len() as u64, routed["samples_16k"].as_u64().unwrap());
    let out = align_with(&model, &pcm, &script(&text)).unwrap();
    assert_matches(&out, &routed, 1e-3);
}
