//! Renders through the real ffmpeg, checked by decoding the output with
//! `-f framemd5` (one hash per decoded frame). Skipped, saying why, when no
//! ffmpeg is found or it lacks what a test needs (Windows' LGPL build has no
//! libx264).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;
use studio_core::StudioError;

use super::super::{
    cache, exports, render, Finished, Options, Outcome, Preset, Progress, RenderError, Toolkit,
};
use crate::studio::test_wav::wav_of;
use crate::test_support::Scratch;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../studio-core/tests/fixtures");

/// The ffmpeg to test with, if there is one that can render `preset`
/// (frames, an encoder and AAC for a video preset; AAC for `audio`).
fn ffmpeg(preset: Preset) -> Option<PathBuf> {
    let Some(found) = crate::sources::echo360::find_ffmpeg(None) else {
        eprintln!("skipped: no ffmpeg");
        return None;
    };
    let toolkit = Toolkit::of(&found).unwrap();
    let ready = match preset.output() {
        super::super::Output::Video(video) => toolkit
            .check_frames()
            .and_then(|()| toolkit.check_audio())
            .and_then(|()| toolkit.codec(video.class, (640, 360), 30).map(drop)),
        super::super::Output::Audio => toolkit.check_audio(),
        super::super::Output::Subtitles => Ok(()),
    };
    match ready.and_then(|()| checkable(&found)) {
        Ok(()) => Some(found),
        Err(e) => {
            eprintln!("skipped: {e}");
            None
        }
    }
}

/// The muxers these tests decode through, which the Windows LGPL build
/// leaves out.
fn checkable(ffmpeg: &Path) -> Result<(), RenderError> {
    let out = Command::new(ffmpeg)
        .args(["-hide_banner", "-muxers"])
        .output()
        .unwrap();
    let list = String::from_utf8_lossy(&out.stdout);
    let has = |name: &str| {
        list.lines()
            .any(|l| l.split_whitespace().nth(1) == Some(name))
    };
    match ["framemd5", "f32le", "null"].into_iter().find(|m| !has(m)) {
        None => Ok(()),
        Some(m) => Err(RenderError::Render(format!(
            "this ffmpeg has no `{m}` muxer to check with"
        ))),
    }
}

/// libx264, where the final preset's output is the same on every run.
fn has_x264(ffmpeg: &Path) -> bool {
    let found = Toolkit::of(ffmpeg).unwrap().has_encoder("libx264");
    if !found {
        eprintln!("skipped: this ffmpeg has no libx264");
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

/// The transitions fixture's `Panel`, four scenes of half a second.
fn panels(dir: &Path, titles: [&str; 4]) {
    copy_dir(
        &Path::new(FIXTURES).join("transitions/components"),
        &dir.join("components"),
    );
    let scenes: Vec<_> = titles
        .iter()
        .enumerate()
        .map(|(i, title)| {
            json!({
                "component": "Panel",
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

fn options(ffmpeg: &Path, preset: Preset, out: Option<PathBuf>) -> Options {
    Options {
        preset,
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
fn listed_segments(dir: &Path, preset: Preset) -> Vec<PathBuf> {
    std::fs::read_to_string(cache::concat_list(dir, preset))
        .unwrap()
        .lines()
        .map(|l| PathBuf::from(l.trim_start_matches("file '").trim_end_matches('\'')))
        .collect()
}

#[test]
fn a_cached_concat_decodes_like_a_cold_render() {
    let Some(ffmpeg) = ffmpeg(Preset::Final) else {
        return;
    };
    if !has_x264(&ffmpeg) {
        return;
    }
    let dir = Scratch::new("render-e2e-cache");
    panels(&dir, ["One", "Two", "Three", "Four"]);
    let final_ = options(&ffmpeg, Preset::Final, None);

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
    let parts: Vec<String> = listed_segments(&dir, Preset::Final)
        .iter()
        .flat_map(|s| framemd5(&ffmpeg, s))
        .collect();
    assert_eq!(parts, hashes);

    // Editing scene 1 re-renders it and scene 2, whose transition shows it.
    panels(&dir, ["One", "Deux", "Three", "Four"]);
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
        &options(&ffmpeg, Preset::Final, Some(cold_out.clone())),
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
    let Some(ffmpeg) = ffmpeg(Preset::Draft) else {
        return;
    };
    let dir = Scratch::new("render-e2e-audio");
    copy_dir(&Path::new(FIXTURES).join("narrated"), &dir);
    // The manifest's 3 s clip: 0.2 s of silence, then a square wave.
    let clip = wav_of(24_000, 3.0, |i| match (i < 4_800, (i / 30) % 2) {
        (true, _) => 0,
        (false, 0) => 12_000,
        _ => -12_000,
    });
    std::fs::write(dir.join("narration/fixture-intro.wav"), clip).unwrap();

    let report = complete(&dir, &options(&ffmpeg, Preset::Draft, None));
    // intro: 0.25 + 3 + 1.0 s = 127.5 frames, rounded up; outro 1.5 s.
    assert_eq!(report.frames, 128 + 45);
    assert!(report.audio);
    assert_eq!(framemd5(&ffmpeg, &report.path).len(), 173);

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
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    // lead 0.25 s + 0.2 s of silence = 21,600 samples; AAC smears an edge
    // by a few ms.
    let start = onset(&pcm, 0.05);
    assert!((21_400..=21_700).contains(&start), "onset at {start}");
    let video = 173.0 / 30.0;
    let audio = pcm.len() as f64 / 48_000.0;
    assert!(
        (audio - video).abs() < 0.05,
        "audio {audio} s, video {video} s"
    );
}

#[test]
fn a_cancel_mid_render_leaves_nothing_partial() {
    let Some(ffmpeg) = ffmpeg(Preset::Final) else {
        return;
    };
    let dir = Scratch::new("render-e2e-cancel");
    panels(&dir, ["One", "Two", "Three", "Four"]);
    let cancel = AtomicBool::new(false);
    let result = render(
        &dir,
        &options(&ffmpeg, Preset::Final, None),
        &cancel,
        &mut |p| {
            if let Progress::Frame { frame: 3, .. } = p {
                cancel.store(true, Ordering::Relaxed);
            }
        },
    );
    assert!(matches!(result, Ok(Finished::Cancelled)), "{result:?}");
    assert!(!Preset::Final.default_out(&dir).exists());
    let left: Vec<_> = std::fs::read_dir(cache::segments(&dir))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn a_failing_frame_names_its_scene_and_time() {
    let Some(ffmpeg) = ffmpeg(Preset::Final) else {
        return;
    };
    let dir = Scratch::new("render-e2e-fail");
    panels(&dir, ["One", "Two", "Three", "Four"]);
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
        &options(&ffmpeg, Preset::Final, None),
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

/// The narrated fixture with its clip, a 3 s square wave.
fn narrated_video(name: &str) -> Scratch {
    let dir = Scratch::new(name);
    copy_dir(&Path::new(FIXTURES).join("narrated"), &dir);
    let clip = wav_of(
        24_000,
        3.0,
        |i| if (i / 30) % 2 == 0 { 9_000 } else { -9_000 },
    );
    std::fs::write(dir.join("narration/fixture-intro.wav"), clip).unwrap();
    dir
}

#[test]
fn each_export_writes_its_file_and_knows_when_it_is_current() {
    let Some(ffmpeg) = ffmpeg(Preset::Draft) else {
        return;
    };
    let dir = narrated_video("render-e2e-exports");
    let library = studio_core::Library::default();
    let status = |preset: Preset| {
        exports(&dir, &ffmpeg, &library)
            .into_iter()
            .find(|s| s.preset == preset)
            .unwrap()
    };
    assert!(!status(Preset::Draft).current && status(Preset::Draft).path.is_none());

    let draft = complete(&dir, &options(&ffmpeg, Preset::Draft, None));
    let plain = framemd5(&ffmpeg, &draft.path);
    let vtt = dir.join("renders/draft.vtt");
    assert_eq!(draft.sidecars, std::slice::from_ref(&vtt));
    let text = std::fs::read_to_string(&vtt).unwrap();
    assert!(
        text.starts_with(
            "WEBVTT\n\n00:00:00.100 --> 00:00:03.650\nWe follow the gradient downhill,\n"
        ),
        "{text}"
    );
    assert!(status(Preset::Draft).current);
    assert!(!status(Preset::Final).current);

    let subtitles = complete(&dir, &options(&ffmpeg, Preset::Subtitles, None));
    assert_eq!(subtitles.path, dir.join("renders/subtitles.vtt"));
    assert_eq!(std::fs::read_to_string(&subtitles.path).unwrap(), text);
    let srt = std::fs::read_to_string(dir.join("renders/subtitles.srt")).unwrap();
    assert!(
        srt.starts_with("1\n00:00:00,100 --> 00:00:03,650\n"),
        "{srt}"
    );
    assert!(status(Preset::Subtitles).current);

    if Toolkit::of(&ffmpeg).unwrap().check_audio().is_ok() {
        let audio = complete(&dir, &options(&ffmpeg, Preset::Audio, None));
        assert_eq!(audio.path, dir.join("renders/audio.m4a"));
        let out = Command::new(&ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(&audio.path)
            .args(["-map", "0:a", "-f", "null", "-"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(status(Preset::Audio).current);
        // The audio export's track is the draft's: neither was pruned.
        assert!(status(Preset::Draft).current);
    }

    // Turning captions on changes the frames, not the words.
    let video = std::fs::read_to_string(dir.join("video.json")).unwrap();
    let on = crate::studio::videos::with_captions(&video, true).unwrap();
    std::fs::write(dir.join("video.json"), on).unwrap();
    assert!(!status(Preset::Draft).current);
    assert!(status(Preset::Subtitles).current);
    let captioned = complete(&dir, &options(&ffmpeg, Preset::Draft, None));
    let outcomes: Vec<Outcome> = captioned.segments.iter().map(|s| s.outcome).collect();
    assert_eq!(outcomes, [Outcome::Rendered, Outcome::Rendered]);
    let burned = framemd5(&ffmpeg, &captioned.path);
    assert_eq!(burned.len(), plain.len());
    assert_ne!(burned, plain);
}

#[test]
fn an_export_with_nothing_to_say_is_refused() {
    let dir = Scratch::new("render-e2e-silent");
    panels(&dir, ["One", "Two", "Three", "Four"]);
    let ffmpeg = PathBuf::from("ffmpeg-is-not-needed");
    let e = render(
        &dir,
        &options(&ffmpeg, Preset::Subtitles, None),
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("has no narration to export"), "{e}");
}
