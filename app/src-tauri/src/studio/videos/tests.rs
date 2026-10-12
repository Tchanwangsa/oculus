//! `new` scaffolds a video that loads and lints clean over the libraries,
//! `list` reads every video, a bare slug resolves under `videos/`, a title
//! makes a unique slug, and the app's scope reaches only a video's folder.

use std::path::{Path, PathBuf};

use super::*;
use crate::studio::libraries::libraries_at;
use crate::test_support::Scratch;
use studio_core::Project;

fn new(slug: &str) -> NewVideo {
    NewVideo {
        slug: slug.into(),
        ..NewVideo::default()
    }
}

#[test]
fn a_new_video_loads_and_lints_clean_over_the_libraries() {
    let root = Scratch::new("videos-new");
    let videos = root.join("videos");
    let dir = create_in(
        &videos,
        &NewVideo {
            slug: "gradient-descent".into(),
            subject: Some("COMP30027".into()),
            ..NewVideo::default()
        },
    )
    .unwrap();
    assert_eq!(dir, videos.join("gradient-descent"));
    for sub in ["components", "assets"] {
        assert!(dir.join(sub).is_dir(), "{sub}");
    }
    assert_eq!(std::fs::read_to_string(dir.join("script.md")).unwrap(), "");

    let library = libraries_at(&root.join("library"), &root.join("subjects"), false).unwrap();
    let project = Project::load_with(&dir, &library).unwrap();
    assert_eq!(project.video.subject.as_deref(), Some("COMP30027"));
    assert_eq!(project.video.fps, 30);
    assert_eq!(project.video.theme, "default");
    assert_eq!(
        (project.video.size.width, project.video.size.height),
        (1920, 1080)
    );
    assert_eq!(
        project.video.scenes[0].props["title"],
        serde_json::json!("Gradient descent")
    );
    let report = project.lint().unwrap();
    assert!(report.is_clean(), "{:?}", report.issues);
}

#[test]
fn portrait_and_title_shape_the_scaffold() {
    let root = Scratch::new("videos-portrait");
    let dir = create_in(
        &root,
        &NewVideo {
            slug: "tall".into(),
            title: Some("Bayes \"rule\"".into()),
            portrait: true,
            ..NewVideo::default()
        },
    )
    .unwrap();
    let video: Video =
        serde_json::from_str(&std::fs::read_to_string(dir.join("video.json")).unwrap()).unwrap();
    assert_eq!((video.size.width, video.size.height), (1080, 1920));
    assert_eq!(video.subject, None);
    assert_eq!(
        video.scenes[0].props["title"],
        serde_json::json!("Bayes \"rule\"")
    );
}

#[test]
fn new_refuses_an_existing_slug_and_bad_names() {
    let root = Scratch::new("videos-refuse");
    create_in(&root, &new("one")).unwrap();
    std::fs::write(root.join("one/script.md"), "## title\n\nKept.\n").unwrap();
    let err = create_in(&root, &new("one")).unwrap_err();
    assert!(err.contains("exists"), "{err}");
    assert_eq!(
        std::fs::read_to_string(root.join("one/script.md")).unwrap(),
        "## title\n\nKept.\n",
        "a refused new leaves the video alone"
    );
    for bad in ["", "Caps", "a b", "../x", "a/b", "dot.s"] {
        assert!(create_in(&root, &new(bad)).is_err(), "{bad:?}");
    }
    let err = create_in(
        &root,
        &NewVideo {
            slug: "ok".into(),
            subject: Some("../etc".into()),
            ..NewVideo::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("subject"), "{err}");
}

#[test]
fn list_reads_every_video_and_filters_by_subject() {
    let root = Scratch::new("videos-list");
    assert!(
        list_in(&root.join("missing"), None).unwrap().is_empty(),
        "no videos folder yet is no videos"
    );
    let subject = |slug: &str, s: Option<&str>| NewVideo {
        slug: slug.into(),
        subject: s.map(String::from),
        ..NewVideo::default()
    };
    create_in(&root, &subject("b-video", Some("MAST20004"))).unwrap();
    create_in(&root, &subject("a-video", None)).unwrap();
    let narrated = root.join("b-video");
    std::fs::create_dir_all(narrated.join("narration")).unwrap();
    std::fs::write(narrated.join("narration/manifest.json"), "{}").unwrap();
    std::fs::create_dir_all(narrated.join("renders")).unwrap();
    std::fs::write(narrated.join("renders/draft.mp4"), "").unwrap();
    std::fs::create_dir_all(root.join("broken")).unwrap();
    std::fs::write(root.join("broken/video.json"), "{").unwrap();

    let rows = list_in(&root, None).unwrap();
    let slugs: Vec<&str> = rows.iter().map(|r| r.slug.as_str()).collect();
    assert_eq!(slugs, ["a-video", "b-video", "broken"]);
    let b = &rows[1];
    assert_eq!(b.subject.as_deref(), Some("MAST20004"));
    assert_eq!(b.scenes, Some(1));
    assert!(b.narrated);
    assert_eq!(b.renders, ["draft.mp4"]);
    assert!(b.modified.is_some());
    assert!(!rows[0].narrated && rows[0].renders.is_empty());
    assert!(rows[2].problem.is_some() && rows[2].scenes.is_none());

    let only = list_in(&root, Some("MAST20004")).unwrap();
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].slug, "b-video");
}

#[test]
fn a_bare_slug_resolves_under_videos_and_an_existing_dir_wins() {
    let root = Scratch::new("videos-resolve");
    let videos = root.join("videos");
    let dir = create_in(&videos, &new("week-4")).unwrap();
    assert_eq!(resolve_in(&videos, Path::new("week-4")).unwrap(), dir);
    // A path that exists is taken as given, slug-shaped or not.
    let here = root.join("week-4");
    std::fs::create_dir_all(&here).unwrap();
    assert_eq!(resolve_in(&videos, &here).unwrap(), here);
    assert_eq!(resolve_in(&videos, &dir).unwrap(), dir);
    let err = resolve_in(&videos, Path::new("week-5")).unwrap_err();
    assert!(
        err.contains("no video `week-5`") && err.contains("oculus studio list"),
        "{err}"
    );
    let err = resolve_in(&videos, Path::new("no/such/dir")).unwrap_err();
    assert!(err.contains("no folder no/such/dir"), "{err}");
}

#[test]
fn a_slug_is_kebab_ascii_and_unique_among_the_folders() {
    for (title, slug) in [
        ("Gradient descent: a primer!", "gradient-descent-a-primer"),
        ("  Bayes' rule  ", "bayes-rule"),
        ("Écoulement 3D", "coulement-3d"),
        ("???", ""),
        ("", ""),
    ] {
        assert_eq!(slug_from_title(title), slug, "{title:?}");
    }
    let long = slug_from_title(&"word ".repeat(40));
    assert!(
        valid_slug(&long) && long.len() <= 56 && !long.ends_with('-'),
        "{long}"
    );

    let root = Scratch::new("videos-unique");
    assert_eq!(unique_slug(&root, Some("Bayes rule")), "bayes-rule");
    assert_eq!(unique_slug(&root, None), "untitled");
    create_in(&root, &new("bayes-rule")).unwrap();
    create_in(&root, &new("bayes-rule-2")).unwrap();
    assert_eq!(unique_slug(&root, Some("Bayes rule")), "bayes-rule-3");
    let made = unique_slug(&root, Some(&"x".repeat(100)));
    assert!(valid_slug(&made), "{made}");
}

#[test]
fn the_app_may_name_only_a_video() {
    let root = Scratch::new("videos-scope");
    let videos = root.join("videos");
    let dir = create_in(&videos, &new("week-4")).unwrap();
    std::fs::create_dir_all(root.join("elsewhere")).unwrap();
    std::fs::create_dir_all(dir.join("components/nested")).unwrap();
    let canonical = dir.canonicalize().unwrap();

    assert_eq!(scoped_in(&videos, Path::new("week-4")).unwrap(), canonical);
    assert_eq!(scoped_in(&videos, &dir).unwrap(), canonical);
    assert_eq!(video_in(&videos, "week-4").unwrap(), dir);

    for outside in [
        root.join("elsewhere"),
        videos.clone(),
        dir.join("components"),
        dir.join("../../elsewhere"),
        PathBuf::from("/"),
    ] {
        let err = scoped_in(&videos, &outside).unwrap_err();
        assert!(err.contains("not a video"), "{outside:?}: {err}");
    }
    assert!(scoped_in(&videos, Path::new("week-5")).is_err());
    for bad in ["", "../elsewhere", "Week 4", "a/b"] {
        assert!(video_in(&videos, bad).is_err(), "{bad:?}");
    }

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("elsewhere"), videos.join("link")).unwrap();
        let err = scoped_in(&videos, Path::new("link")).unwrap_err();
        assert!(err.contains("not a video"), "{err}");
        assert!(video_in(&videos, "link").is_err());
    }
}

#[test]
fn a_new_video_takes_a_theme_and_a_voice_block() {
    let root = Scratch::new("videos-theme");
    let dir = create_in(
        &root,
        &NewVideo {
            slug: "warm-one".into(),
            theme: Some("warm".into()),
            voice: Some(studio_core::Voice {
                model: None,
                voice: Some("af_heart".into()),
                speed: Some(1.1),
            }),
            ..NewVideo::default()
        },
    )
    .unwrap();
    let video: Video =
        serde_json::from_str(&std::fs::read_to_string(dir.join("video.json")).unwrap()).unwrap();
    assert_eq!(video.theme, "warm");
    let voice = video.voice.unwrap();
    assert_eq!(voice.voice.as_deref(), Some("af_heart"));
    assert_eq!(voice.speed, Some(1.1));
    assert_eq!(voice.model, None);

    let plain = create_in(&root, &new("plain")).unwrap();
    let text = std::fs::read_to_string(plain.join("video.json")).unwrap();
    assert!(
        text.contains("\"theme\": \"default\"") && !text.contains("voice"),
        "{text}"
    );

    for theme in ["../x", "a b", "", "Warm", "a_b"] {
        let err = create_in(
            &root,
            &NewVideo {
                slug: "bad".into(),
                theme: Some(theme.into()),
                ..NewVideo::default()
            },
        );
        assert!(err.is_err(), "{theme:?}");
    }
}
