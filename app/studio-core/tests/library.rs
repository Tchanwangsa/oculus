//! The libraries a load reads: project over subject over global for
//! components, subject over global over bundled for themes.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

use serde_json::json;
use std::path::{Path, PathBuf};
use studio_core::{ComponentSource, Library, Project, StudioError};

/// A fresh folder under the system temp dir.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("studio-library-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn tsx(label: &str) -> String {
    format!(
        "export const props = {{}};\nexport default () => <Text style={{{{ fontSize: 20 }}}}>{label}</Text>;\n"
    )
}

/// A video with one scene of `Card`, its project components, and the three
/// libraries under `root`: `global`, and `subjects/SUB` under `subjects`.
struct Fixture {
    video: PathBuf,
    global: PathBuf,
    subjects: PathBuf,
}

fn fixture(name: &str, video_json: serde_json::Value) -> Fixture {
    let root = dir(name);
    let video = root.join("video");
    let global = root.join("global");
    let subjects = root.join("subjects");
    put(&video, "video.json", &video_json.to_string());
    put(&video, "components/Card.tsx", &tsx("project card"));
    put(&subjects, "SUB/components/Steps.tsx", &tsx("subject steps"));
    put(&subjects, "SUB/components/Card.tsx", &tsx("subject card"));
    put(&global, "components/Steps.tsx", &tsx("global steps"));
    put(&global, "components/Title.tsx", &tsx("global title"));
    put(
        &global,
        "themes/warm.json",
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("themes/default.json"))
            .unwrap()
            .replace("\"#5e6ad2\"", "\"#c2410c\"")
            .replace("\"accent\": \"#5e6ad2\"", "\"accent\": \"#c2410c\""),
    );
    Fixture {
        video,
        global,
        subjects,
    }
}

fn library(f: &Fixture) -> Library {
    Library {
        global: Some(f.global.clone()),
        subjects: Some(f.subjects.clone()),
        ..Library::default()
    }
}

fn video(subject: Option<&str>, theme: &str) -> serde_json::Value {
    let mut v = json!({
        "size": { "width": 64, "height": 36 }, "theme": theme,
        "scenes": [{ "component": "Card", "hold": 1 }]
    });
    if let Some(s) = subject {
        v["subject"] = json!(s);
    }
    v
}

#[test]
fn a_component_resolves_project_then_subject_then_global() {
    let f = fixture("cascade", video(Some("SUB"), "default"));
    let p = Project::load_with(&f.video, &library(&f)).unwrap();
    let read = |name: &str| p.components.read(name).unwrap();
    assert!(read("Card").contains("project card"));
    assert!(read("Steps").contains("subject steps"));
    assert!(read("Title").contains("global title"));
    let levels: Vec<(String, String)> = p.components.levels();
    let level = |name: &str| {
        levels
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, l)| l.as_str())
    };
    assert_eq!(level("Card"), Some("project"));
    assert_eq!(level("Steps"), Some("subject SUB"));
    assert_eq!(level("Title"), Some("global"));
    std::fs::remove_dir_all(f.video.parent().unwrap()).ok();
}

#[test]
fn a_subject_passed_to_the_load_replaces_the_videos() {
    let f = fixture("override", video(None, "default"));
    let lib = Library {
        subject: Some("SUB".to_owned()),
        ..library(&f)
    };
    let p = Project::load_with(&f.video, &lib).unwrap();
    assert!(
        p.components
            .read("Steps")
            .unwrap()
            .contains("subject steps")
    );
    // Without the override the video has no subject: global only.
    let plain = Project::load_with(&f.video, &library(&f)).unwrap();
    assert!(
        plain
            .components
            .read("Steps")
            .unwrap()
            .contains("global steps")
    );
    std::fs::remove_dir_all(f.video.parent().unwrap()).ok();
}

#[test]
fn no_subjects_root_leaves_the_subject_library_unread() {
    let f = fixture("nosubjects", video(Some("SUB"), "default"));
    let lib = Library {
        global: Some(f.global.clone()),
        ..Library::default()
    };
    let p = Project::load_with(&f.video, &lib).unwrap();
    assert!(p.components.read("Steps").unwrap().contains("global steps"));
    std::fs::remove_dir_all(f.video.parent().unwrap()).ok();
}

#[test]
fn a_theme_name_resolves_subject_then_global_then_bundled() {
    let f = fixture("themes", video(Some("SUB"), "warm"));
    // Only the global library has warm.json.
    let p = Project::load_with(&f.video, &library(&f)).unwrap();
    assert_eq!(p.theme.tokens["color"]["accent"], "#c2410c");
    // The subject's file beats the global one.
    put(
        &f.subjects,
        "SUB/themes/warm.json",
        &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("themes/default.json"))
            .unwrap()
            .replace("\"accent\": \"#5e6ad2\"", "\"accent\": \"#0f766e\""),
    );
    let p = Project::load_with(&f.video, &library(&f)).unwrap();
    assert_eq!(p.theme.tokens["color"]["accent"], "#0f766e");
    // A bundled name still resolves with no library.
    let bundled = Project::load(&f.video).err().unwrap();
    assert!(
        bundled.to_string().contains("no bundled theme `warm`"),
        "{bundled}"
    );
    std::fs::remove_dir_all(f.video.parent().unwrap()).ok();
}

#[test]
fn a_theme_override_replaces_the_videos() {
    let f = fixture("themeoverride", video(None, "default"));
    let lib = Library {
        theme: Some("warm".to_owned()),
        ..library(&f)
    };
    let p = Project::load_with(&f.video, &lib).unwrap();
    assert_eq!(p.theme.tokens["color"]["accent"], "#c2410c");
    std::fs::remove_dir_all(f.video.parent().unwrap()).ok();
}

#[test]
fn a_subject_must_be_a_course_folder_name() {
    let f = fixture("badsubject", video(Some("../etc"), "default"));
    let err = Project::load_with(&f.video, &library(&f)).err().unwrap();
    assert!(err.to_string().contains("subject"), "{err}");
    std::fs::remove_dir_all(f.video.parent().unwrap()).ok();
}

#[test]
fn a_missing_component_lists_every_library_name() {
    let f = fixture("missing", video(Some("SUB"), "default"));
    let p = Project::load_with(&f.video, &library(&f)).unwrap();
    let err = p.components.read("Ttl").unwrap_err();
    let StudioError::ComponentNotFound { message, .. } = &err else {
        panic!("{err:?}")
    };
    assert!(
        message.ends_with("available: Card, Steps, Title"),
        "{message}"
    );
    assert!(message.contains("did you mean `Title`?"), "{message}");
    std::fs::remove_dir_all(f.video.parent().unwrap()).ok();
}
