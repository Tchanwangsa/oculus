//! Renders through the real ffmpeg, checked by decoding the output with
//! `-f framemd5` (one hash per decoded frame). Skipped when no ffmpeg is
//! found.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;
use studio_core::StudioError;

use super::super::{cache, render, Finished, Options, Outcome, Progress, Quality, RenderError};
use crate::studio::test_wav::wav_of;
use crate::test_support::Scratch;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../studio-core/tests/fixtures");

fn ffmpeg() -> Option<PathBuf> {
    let found = crate::sources::echo360::find_ffmpeg(None);
    if found.is_none() {
        eprintln!("skipped: no ffmpeg");
    }
    found
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The transitions fixture's `Slide`, four scenes of half a second.
fn slides(dir: &Path, titles: [&str; 4]) {
    copy_dir(
        &Path::new(FIXTURES).join("transitions/components"),
        &dir.join("components"),
    );
    let scenes: Vec<_> = titles
        .iter()
        .enumerate()
        .map(|(i, title)| {
            json!({
                "component": "Slide",
                "props": { "title": title, "variant": if i % 2 == 0 { "a" } else { "b" } },
                "hold": 0.5,
                "transition": if i == 0 { json!(null) } else { json!({ "kind": "push", "dur": 0.2 }) },
            })
        })
        .collect();
    let mut video = json!({
        "size": { "width": 640, "height": 360 },
        "theme": "default",
        "motion": { "motionBlur": "off" },
        "scenes": scenes,
    });
    video["scenes"][0]
        .as_object_mut()
        .unwrap()
        .remove("transition");
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
}

fn options(ffmpeg: &Path, quality: Quality, out: Option<PathBuf>) -> Options {
    Options {
        quality,
        threads: Some(2),
        out,
        ffmpeg: ffmpeg.to_owned(),
        library: studio_core::Library::default(),
    }
}

fn complete(dir: &Path, options: &Options) -> super::super::Report {
    match render(dir, options, &AtomicBool::new(false), &mut |_| {}) {
        Ok(Finished::Complete(report)) => report,
        Ok(Finished::Cancelled) => panic!("cancelled"),
        Err(e) => panic!("{e}"),
    }
}

/// One md5 per decoded video frame, in order.
fn framemd5(ffmpeg: &Path, file: &Path) -> Vec<String> {
    let out = Command::new(ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-map", "0:v", "-f", "framemd5", "-"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.rsplit(',').next().unwrap().trim().to_owned())
        .collect()
}

/// The segments a render joined, from its concat list.
fn listed_segments(dir: &Path, quality: Quality) -> Vec<PathBuf> {
    std::fs::read_to_string(cache::concat_list(dir, quality))
        .unwrap()
        .lines()
        .map(|l| PathBuf::from(l.trim_start_matches("file '").trim_end_matches('\'')))
        .collect()
}

#[test]
fn a_cached_concat_decodes_like_a_cold_render() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = Scratch::new("render-e2e-cache");
    slides(&dir, ["One", "Two", "Three", "Four"]);
    let final_ = options(&ffmpeg, Quality::Final, None);

    let first = complete(&dir, &final_);
    assert_eq!(first.frames, 60);
    assert!(first
        .segments
        .iter()
        .all(|s| s.outcome == Outcome::Rendered && s.frames == 15));
    let hashes = framemd5(&ffmpeg, &first.path);
    assert_eq!(hashes.len(), 60);
    // The joins neither drop nor repeat a frame: the joined file decodes to
    // each segment's frames in turn.
    let parts: Vec<String> = listed_segments(&dir, Quality::Final)
        .iter()
        .flat_map(|s| framemd5(&ffmpeg, s))
        .collect();
    assert_eq!(parts, hashes);

    // Editing scene 1 re-renders it and scene 2, whose transition shows it.
    slides(&dir, ["One", "Deux", "Three", "Four"]);
    let second = complete(&dir, &final_);
    let outcomes: Vec<Outcome> = second.segments.iter().map(|s| s.outcome).collect();
    use Outcome::{Cached, Rendered};
    assert_eq!(outcomes, [Cached, Rendered, Rendered, Cached]);
    assert_eq!(second.pruned.len(), 2, "{:?}", second.pruned);
    let edited = framemd5(&ffmpeg, &second.path);
    assert_ne!(edited, hashes);

    std::fs::remove_dir_all(dir.join(".cache")).unwrap();
    let cold_out = dir.join("cold.mp4");
    let cold = complete(
        &dir,
        &options(&ffmpeg, Quality::Final, Some(cold_out.clone())),
    );
    assert!(cold.segments.iter().all(|s| s.outcome == Rendered));
    assert_eq!(cold.path, cold_out);
    assert_eq!(framemd5(&ffmpeg, &cold_out), edited);
}

/// Where the first sample louder than `level` is.
fn onset(pcm: &[f32], level: f32) -> usize {
    pcm.iter().position(|s| s.abs() > level).unwrap()
}

#[test]
fn narration_starts_at_its_scenes_lead() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = Scratch::new("render-e2e-audio");
    copy_dir(&Path::new(FIXTURES).join("narrated"), &dir);
    // The manifest's 3 s clip: 0.2 s of silence, then a square wave.
    let clip = wav_of(24_000, 3.0, |i| match (i < 4_800, (i / 30) % 2) {
        (true, _) => 0,
        (false, 0) => 12_000,
        _ => -12_000,
    });
    std::fs::write(dir.join("narration/fixture-intro.wav"), clip).unwrap();

    let quality = if cfg!(target_os = "macos") {
        Quality::Draft
    } else {
        Quality::Final
    };
    let report = complete(&dir, &options(&ffmpeg, quality, None));
    // intro: 0.25 + 3 + 0.6 s = 115.5 frames, rounded up; outro 1.5 s.
    assert_eq!(report.frames, 116 + 45);
    assert!(report.audio);
    assert_eq!(framemd5(&ffmpeg, &report.path).len(), 161);

    let out = Command::new(&ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(&report.path)
        .args([
            "-map", "0:a", "-f", "f32le", "-ac", "1", "-ar", "48000", "-",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let pcm: Vec<f32> = out
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    // lead 0.25 s + 0.2 s of silence = 21,600 samples; AAC smears an edge
    // by a few ms.
    let start = onset(&pcm, 0.05);
    assert!((21_400..=21_700).contains(&start), "onset at {start}");
    let video = 161.0 / 30.0;
    let audio = pcm.len() as f64 / 48_000.0;
    assert!(
        (audio - video).abs() < 0.05,
        "audio {audio} s, video {video} s"
    );
}

#[test]
fn a_cancel_mid_render_leaves_nothing_partial() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = Scratch::new("render-e2e-cancel");
    slides(&dir, ["One", "Two", "Three", "Four"]);
    let cancel = AtomicBool::new(false);
    let result = render(
        &dir,
        &options(&ffmpeg, Quality::Final, None),
        &cancel,
        &mut |p| {
            if let Progress::Frame { frame: 3, .. } = p {
                cancel.store(true, Ordering::Relaxed);
            }
        },
    );
    assert!(matches!(result, Ok(Finished::Cancelled)), "{result:?}");
    assert!(!super::super::default_out(&dir, Quality::Final).exists());
    let left: Vec<_> = std::fs::read_dir(cache::segments(&dir))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn a_failing_frame_names_its_scene_and_time() {
    let Some(ffmpeg) = ffmpeg() else { return };
    let dir = Scratch::new("render-e2e-fail");
    slides(&dir, ["One", "Two", "Three", "Four"]);
    std::fs::write(
        dir.join("components/Breaks.tsx"),
        "export const props = {};\nexport default ({}, { t }) => {\n  if (t > 0.3) throw new Error(\"late\");\n  return <Box />;\n};\n",
    )
    .unwrap();
    let mut video: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("video.json")).unwrap()).unwrap();
    video["scenes"][2] =
        json!({ "component": "Breaks", "hold": 0.5, "transition": { "kind": "cut" } });
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();

    let result = render(
        &dir,
        &options(&ffmpeg, Quality::Final, None),
        &AtomicBool::new(false),
        &mut |_| {},
    );
    match result {
        Err(RenderError::Studio(StudioError::Frame { scene, scene_t, .. })) => {
            assert_eq!(scene, 2);
            // The first frame past 0.3 s: 10 / 30.
            assert!((scene_t - 10.0 / 30.0).abs() < 1e-9, "{scene_t}");
        }
        other => panic!("{other:?}"),
    }
    let segments = cache::segments(&dir);
    let parts = std::fs::read_dir(&segments)
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".part"))
        .count();
    assert_eq!(parts, 0);
}
