//! The starter set lints clean, in its real cascade, with both bundled
//! themes: token use, required props, and every scene renders.
#![allow(clippy::unwrap_used, clippy::panic)] // helpers outside #[test] fns

use serde_json::{Value, json};
use std::path::PathBuf;
use studio_core::{
    Engine, Library, Project, RenderOptions, STARTER_COMPONENTS, Size, Theme, seed_library,
};

/// A video with one scene per starter component (its first example's
/// props), in the given theme, over a global library seeded from the starter.
fn starter_video(theme: &str) -> (PathBuf, Library) {
    let root = std::env::temp_dir().join(format!("studio-starter-{theme}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let video = root.join("video");
    let global = root.join("library");
    std::fs::create_dir_all(&video).unwrap();
    seed_library(&global, false).unwrap();
    let scenes: Vec<Value> = STARTER_COMPONENTS
        .iter()
        .map(|(name, tsx)| {
            let props = first_example(name, tsx);
            json!({ "component": name, "props": props, "hold": 1.0 })
        })
        .collect();
    let v = json!({
        "size": { "width": 640, "height": 360 },
        "theme": theme,
        "scenes": scenes,
    });
    std::fs::write(video.join("video.json"), v.to_string()).unwrap();
    let library = Library {
        global: Some(global),
        ..Library::default()
    };
    (video, library)
}

/// The props of a component's first declared example.
fn first_example(name: &str, tsx: &str) -> Value {
    let theme = Theme::bundled("default").unwrap();
    let size = Size {
        width: 640,
        height: 360,
    };
    let mut engine = Engine::new(theme, size, None, RenderOptions::default()).unwrap();
    let info = engine.load_component(name, tsx).unwrap();
    info.examples.values().next().cloned().unwrap()
}

#[test]
fn the_starter_set_lints_clean_with_each_bundled_theme() {
    for theme in ["default", "warm"] {
        let (video, library) = starter_video(theme);
        let project = Project::load_with(&video, &library).unwrap();
        let report = project.lint().unwrap();
        assert!(report.is_clean(), "{theme}:\n{report}");
        std::fs::remove_dir_all(video.parent().unwrap()).ok();
    }
}
