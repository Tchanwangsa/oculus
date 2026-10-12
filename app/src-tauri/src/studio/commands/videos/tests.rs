//! The Studio page's commands over scratch folders: the list with its
//! links, `new`, the detail of a narrated and an unnarrated video, stills,
//! and the thumbnail cache. Narrated videos copy studio-core's `narrated`
//! fixture.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use studio_core::Library;

use super::detail::detail_in;
use super::frame::{frame_png, FrameAt, MAX_WIDTH};
use super::new::{create_in, NewVideoOptions, VoiceBlock};
use super::rows::list_in;
use super::thumbnail::thumbnail_in;
use crate::db::store::StudioLink;
use crate::studio::libraries::libraries_at;
use crate::test_support::Scratch;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../studio-core/tests/fixtures");

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

/// `videos/narrated`: the fixture whose `intro` scene is narrated (a 3 s clip
/// in the manifest, no audio file, which a still never reads).
fn narrated(videos: &Path) -> PathBuf {
    let dir = videos.join("narrated");
    copy_dir(&Path::new(FIXTURES).join("narrated"), &dir);
    dir
}

fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

fn options(title: &str) -> NewVideoOptions {
    NewVideoOptions {
        title: Some(title.into()),
        ..NewVideoOptions::default()
    }
}

#[test]
fn new_makes_a_unique_slug_that_loads_with_the_options_given() {
    let root = Scratch::new("cmd-new");
    let videos = root.join("videos");
    let library = libraries_at(&root.join("library"), &root.join("subjects"), false).unwrap();

    let first = create_in(&videos, &library, options("Bayes rule")).unwrap();
    let second = create_in(&videos, &library, options("Bayes rule")).unwrap();
    assert_eq!(
        (first.slug.as_str(), second.slug.as_str()),
        ("bayes-rule", "bayes-rule-2")
    );
    assert_eq!(second.dir, videos.join("bayes-rule-2"));

    let reel = create_in(
        &videos,
        &library,
        NewVideoOptions {
            title: None,
            subject: Some("MAST20004".into()),
            theme: Some("warm".into()),
            portrait: true,
            voice: Some(VoiceBlock {
                voice: Some("af_heart".into()),
                ..VoiceBlock::default()
            }),
        },
    )
    .unwrap();
    assert_eq!(reel.slug, "untitled");
    let project = studio_core::Project::load_with(&reel.dir, &library).unwrap();
    assert_eq!(project.video.theme, "warm");
    assert_eq!(project.video.subject.as_deref(), Some("MAST20004"));
    assert_eq!(project.video.size.height, 1920);

    // A theme nothing defines would not load; the folder is not left behind.
    let err = create_in(
        &videos,
        &library,
        NewVideoOptions {
            theme: Some("no-such-theme".into()),
            ..options("Ghost")
        },
    )
    .unwrap_err();
    assert!(err.contains("no-such-theme"), "{err}");
    assert!(!videos.join("ghost").exists());
}

#[test]
fn the_list_has_titles_threads_and_the_newest_first() {
    let root = Scratch::new("cmd-list");
    let videos = root.join("videos");
    let library = Library::default();
    let old = create_in(&videos, &library, options("Old one")).unwrap();
    let new = create_in(&videos, &library, options("New one")).unwrap();
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options()
        .write(true)
        .open(old.dir.join("video.json"))
        .unwrap()
        .set_modified(past)
        .unwrap();
    let links = HashMap::from([(
        "old-one".to_owned(),
        StudioLink {
            slug: "old-one".into(),
            thread_id: Some(7),
            length_secs: Some(60),
        },
    )]);

    let rows = list_in(&videos, None, &links).unwrap();
    let slugs: Vec<&str> = rows.iter().map(|r| r.slug.as_str()).collect();
    assert_eq!(slugs, ["new-one", "old-one"]);
    assert_eq!(rows[0].title, "New one");
    assert_eq!(rows[0].thread_id, None);
    assert_eq!(rows[1].thread_id, Some(7));
    assert_eq!(rows[0].dir, new.dir.to_string_lossy());

    let json = serde_json::to_value(&rows[1]).unwrap();
    assert_eq!(json["threadId"], 7);
    assert!(json["subject"].is_null() && json["problem"].is_null());
    assert_eq!(json["scenes"], 1);
}

#[test]
fn the_detail_of_a_narrated_video_has_scene_times_and_words() {
    let root = Scratch::new("cmd-detail");
    let videos = root.join("videos");
    let dir = narrated(&videos);
    std::fs::create_dir_all(dir.join("renders")).unwrap();
    std::fs::write(dir.join("renders/draft.mp4"), "x").unwrap();
    std::fs::write(dir.join("renders/audio.m4a"), "x").unwrap();
    std::fs::write(dir.join("renders/notes.txt"), "x").unwrap();

    let detail = detail_in(
        &videos,
        &Library::default(),
        &HashMap::new(),
        Some("settings"),
        "narrated",
    )
    .unwrap();
    assert_eq!((detail.width, detail.height, detail.fps), (640, 360, 30));
    assert_eq!(detail.theme, "default");
    assert_eq!(
        detail.voice.as_deref(),
        Some("af_heart"),
        "video.json's voice wins"
    );
    assert!(detail.row.narrated);
    assert_eq!(detail.scenes.len(), 2);
    let intro = &detail.scenes[0];
    assert_eq!(intro.id.as_deref(), Some("intro"));
    assert_eq!(intro.component, "Cued");
    assert_eq!(intro.start, Some(0.0));
    assert!(intro.duration.unwrap() >= 3.0, "{:?}", intro.duration);
    assert_eq!(
        intro.text.as_deref(),
        Some("We follow the gradient downhill, one small step at a time.")
    );
    let outro = &detail.scenes[1];
    assert_eq!(outro.start, intro.duration);
    assert_eq!(outro.duration, Some(1.5));
    assert_eq!(outro.text, None, "a silent scene has no words");
    assert_eq!(detail.renders.len(), 2, "{:?}", detail.renders);
    assert_eq!(
        detail.renders[0].path,
        dir.join("renders/draft.mp4").to_string_lossy()
    );
    assert!(detail.renders[0].modified > 0);

    let json = serde_json::to_value(&detail).unwrap();
    assert_eq!(json["renders"][0]["preset"], "draft");
    assert_eq!(json["renders"][1]["preset"], "audio");
    assert_eq!(json["captions"], false);
    assert_eq!(json["scenes"][1]["duration"], 1.5);
    assert_eq!(json["row"]["slug"], "narrated");
}

#[test]
fn the_detail_of_an_unnarrated_video_has_no_times_and_falls_back_to_the_settings_voice() {
    let root = Scratch::new("cmd-detail-stale");
    let videos = root.join("videos");
    let dir = narrated(&videos);
    // The words changed after narrating: the clip is for older text.
    std::fs::write(dir.join("script.md"), "## intro\n\nA different sentence.\n").unwrap();
    let mut video: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("video.json")).unwrap()).unwrap();
    video.as_object_mut().unwrap().remove("voice");
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();

    let detail = detail_in(
        &videos,
        &Library::default(),
        &HashMap::new(),
        Some("settings"),
        "narrated",
    )
    .unwrap();
    assert_eq!(detail.voice.as_deref(), Some("settings"));
    assert!(detail
        .scenes
        .iter()
        .all(|s| s.start.is_none() && s.duration.is_none()));
    assert_eq!(
        detail.scenes[0].text.as_deref(),
        Some("A different sentence.")
    );
    assert!(detail.renders.is_empty());

    let none = detail_in(
        &videos,
        &Library::default(),
        &HashMap::new(),
        None,
        "narrated",
    )
    .unwrap();
    assert_eq!(none.voice, None);
    assert!(detail_in(&videos, &Library::default(), &HashMap::new(), None, "../x").is_err());
}

#[test]
fn a_frame_is_a_png_of_the_width_asked_and_stale_narration_is_its_own_error() {
    let root = Scratch::new("cmd-frame");
    let videos = root.join("videos");
    let dir = narrated(&videos);
    let library = Library::default();

    let at = FrameAt::Scene { scene: 0, t: 1.0 };
    let bytes = frame_png(&videos, &library, "narrated", at, 320).unwrap();
    assert_eq!(png_size(&bytes), (320, 180));
    let global = frame_png(
        &videos,
        &library,
        "narrated",
        FrameAt::Global { global: 4.0 },
        640,
    )
    .unwrap();
    assert_eq!(png_size(&global), (640, 360));

    for width in [0, MAX_WIDTH + 1] {
        assert!(frame_png(&videos, &library, "narrated", at, width).is_err());
    }
    let err = frame_png(
        &videos,
        &library,
        "narrated",
        FrameAt::Global { global: 99.0 },
        320,
    )
    .unwrap_err();
    assert!(err.contains("99"), "{err}");

    std::fs::write(dir.join("script.md"), "## intro\n\nA different sentence.\n").unwrap();
    let err = frame_png(&videos, &library, "narrated", at, 320).unwrap_err();
    assert!(err.contains("narrat"), "stale narration says so: {err}");
}

#[test]
fn a_frame_position_reads_from_the_pages_json() {
    let scene: FrameAt = serde_json::from_str(r#"{"scene":2,"t":1.5}"#).unwrap();
    assert_eq!(scene, FrameAt::Scene { scene: 2, t: 1.5 });
    let global: FrameAt = serde_json::from_str(r#"{"global":12.25}"#).unwrap();
    assert_eq!(global, FrameAt::Global { global: 12.25 });
}

fn thumbs(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join(".cache/thumbs"))
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_thumbnail_is_cached_by_its_scene_and_replaced_when_the_scene_changes() {
    let root = Scratch::new("cmd-thumb");
    let videos = root.join("videos");
    let dir = narrated(&videos);
    let library = Library::default();

    let first = thumbnail_in(&videos, &library, "narrated").unwrap();
    assert_eq!(png_size(&first), (640, 360));
    let names = thumbs(&dir);
    assert_eq!(names.len(), 1, "{names:?}");

    // A hit reads the file: an altered cache file comes back as it is.
    let file = dir.join(".cache/thumbs").join(&names[0]);
    std::fs::write(&file, &first[..first.len() - 1]).unwrap();
    let hit = thumbnail_in(&videos, &library, "narrated").unwrap();
    assert_eq!(hit.len(), first.len() - 1);
    assert_eq!(thumbs(&dir), names);

    // Editing scene 0's props is a new key: one fresh file, the old one gone.
    let mut video: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("video.json")).unwrap()).unwrap();
    video["scenes"][0]["props"]["title"] = "Another title".into();
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
    let second = thumbnail_in(&videos, &library, "narrated").unwrap();
    assert_ne!(second, first);
    let after = thumbs(&dir);
    assert_eq!(after.len(), 1);
    assert_ne!(after, names);
    assert!(!dir.join("renders").exists(), "renders/ is never touched");
}

#[test]
fn an_unnarrated_video_gets_a_silent_thumbnail_and_a_failure_says_so() {
    let root = Scratch::new("cmd-thumb-silent");
    let videos = root.join("videos");
    let library = libraries_at(&root.join("library"), &root.join("subjects"), false).unwrap();

    // The scaffold has a hold and no narration at all.
    let made = create_in(&videos, &library, options("Fresh")).unwrap();
    let bytes = thumbnail_in(&videos, &library, "fresh").unwrap();
    assert_eq!(png_size(&bytes), (640, 360));
    assert_eq!(thumbs(&made.dir).len(), 1);

    // Stale narration, and a first scene that cues a word: silent, it has
    // no words to cue, and the thumbnail says why.
    let dir = narrated(&videos);
    std::fs::write(dir.join("script.md"), "## intro\n\nA different sentence.\n").unwrap();
    let err = thumbnail_in(&videos, &Library::default(), "narrated").unwrap_err();
    assert!(err.starts_with("no thumbnail for `narrated`:"), "{err}");
    assert!(err.contains("not narrated"), "{err}");

    // The same video with a first scene that needs no words draws silent.
    let mut video: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("video.json")).unwrap()).unwrap();
    video["scenes"][0] = serde_json::json!({ "id": "intro", "component": "Outro", "hold": 1.0 });
    std::fs::write(dir.join("video.json"), video.to_string()).unwrap();
    let bytes = thumbnail_in(&videos, &Library::default(), "narrated").unwrap();
    assert_eq!(png_size(&bytes), (640, 360));

    // A component that does not draw fails, naming the video.
    std::fs::write(
        dir.join("components/Outro.tsx"),
        "export default function Outro( {",
    )
    .unwrap();
    let err = thumbnail_in(&videos, &Library::default(), "narrated").unwrap_err();
    assert!(err.starts_with("no thumbnail for `narrated`:"), "{err}");
}
