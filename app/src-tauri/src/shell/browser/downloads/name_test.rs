use std::path::{Path, PathBuf};

use super::name::{is_stream_manifest, safe, staged_name, uncounted};

fn proposed(name: &str) -> PathBuf {
    // What wry does with WebKit's suggested name: push it onto ~/Downloads.
    let mut path = PathBuf::from("/Users/student/Downloads");
    path.push(name);
    path
}

#[test]
fn an_ordinary_name_is_kept_as_the_server_gave_it() {
    assert_eq!(
        staged_name(&proposed("Week 3 tutorial.pdf"), None),
        "Week 3 tutorial.pdf"
    );
    assert_eq!(staged_name(&proposed("données.csv"), None), "données.csv");
}

#[test]
fn a_hostile_name_cannot_leave_its_staging_folder() {
    let folder = Path::new("/data/downloads/.incoming/1-1");
    for (hostile, expected) in [
        ("../../etc/passwd", "passwd"),
        ("/etc/passwd", "passwd"),
        ("..", "download"),
        (".", "download"),
        ("...", "download"),
        ("", "download"),
        ("   ", "download"),
        ("a/b/", "download"),
        (r"..\..\Windows\evil.exe", "evil.exe"),
        (r"C:\Users\x\evil.exe", "evil.exe"),
        (".bashrc", "bashrc"),
        ("notes.pdf.", "notes.pdf"),
        ("a:b*c?d\"e<f>g|h.txt", "a_b_c_d_e_f_g_h.txt"),
        ("tab\there\nnewline.txt", "tabherenewline.txt"),
        ("CON.txt", "_CON.txt"),
        ("lpt1", "_lpt1"),
        ("COM10.txt", "COM10.txt"),
    ] {
        let name = staged_name(&proposed(hostile), None);
        assert_eq!(name, expected, "{hostile:?}");
        let staged = folder.join(&name);
        assert_eq!(staged.parent(), Some(folder), "{hostile:?} escaped");
        assert_eq!(staged.components().count(), folder.components().count() + 1);
    }
}

#[test]
fn the_names_webkit_was_measured_suggesting_are_staged_safely() {
    // `browser-downloads-stage0.md` §3.3: what reaches the hook for each
    // header or URL, and what it is staged as.
    for (suggested, staged) in [
        ("_.._evil.sh", "_.._evil.sh"),
        ("a_bc_d*e?.pdf", "a_bc_d_e_.pdf"),
        ("bashrc.zip", "bashrc.zip"),
        ("ทดสอบ €.pdf", "ทดสอบ €.pdf"),
        ("lecture-notes.pdf", "lecture-notes.pdf"),
        ("noname", "noname"),
        ("file", "file"),
        ("Unknown.zip", "Unknown.zip"),
        ("Lecture 1 - Intro.pdf", "Lecture 1 - Intro.pdf"),
        ("my.report.v2.zip", "my.report.v2.zip"),
    ] {
        assert_eq!(staged_name(&proposed(suggested), None), staged);
    }
}

#[test]
fn a_name_webkit_would_fail_on_is_cut_under_the_limit() {
    // WebKit hands over a 304-byte name whole and then fails the download.
    let long = format!("{}.pdf", "n".repeat(300));
    assert_eq!(long.len(), 304);
    let name = staged_name(&proposed(&long), None);
    assert!(name.len() <= 200, "{}", name.len());
    assert!(name.ends_with(".pdf"));
}

#[test]
fn a_streaming_playlist_is_told_by_its_extension_in_any_case() {
    for playlist in [
        "master.m3u8",
        "index.M3U8",
        "list.m3u",
        "manifest.mpd",
        "a.b.MPD",
    ] {
        assert!(is_stream_manifest(playlist), "{playlist}");
    }
    // As staged: a trailing dot is gone before the test.
    assert!(is_stream_manifest(&staged_name(
        &proposed("master.m3u8."),
        None
    )));
    for file in ["lecture.mp4", "m3u8", "notes.m3u8.pdf", "mpd.zip", "impd"] {
        assert!(!is_stream_manifest(file), "{file}");
    }
}

#[test]
fn a_long_name_is_cut_keeping_its_extension() {
    let long = format!("{}.pdf", "a".repeat(400));
    let name = safe(&long);
    assert!(name.len() <= 200, "{}", name.len());
    assert!(name.ends_with(".pdf"));

    let wide = format!("{}.pdf", "é".repeat(300));
    let name = safe(&wide);
    assert!(name.len() <= 200);
    assert!(name.ends_with("é.pdf"), "cut on a character boundary");

    let odd = format!("x.{}", "b".repeat(400));
    assert!(safe(&odd).len() <= 200, "an over-long extension is cut too");
}

#[test]
fn webview2s_counter_is_dropped_when_it_stepped_around_a_real_file() {
    let taken = |name: &str| matches!(name, "notes.pdf" | "a.tar.gz" | "x");
    assert_eq!(uncounted("notes (1).pdf", taken), "notes.pdf");
    assert_eq!(uncounted("a (2).tar.gz", taken), "a.tar.gz");
    assert_eq!(uncounted("x (12)", taken), "x");
    assert_eq!(
        uncounted("other (1).pdf", taken),
        "other (1).pdf",
        "nothing was taken"
    );
    assert_eq!(
        uncounted("Report (final).pdf", |_| true),
        "Report (final).pdf"
    );
    assert_eq!(uncounted("notes (1)x.pdf", |_| true), "notes (1)x.pdf");
    assert_eq!(uncounted(" (1)", |_| true), " (1)");

    let dir = crate::test_support::Scratch::new("download-names");
    std::fs::write(dir.join("notes.pdf"), b"theirs").unwrap();
    let mut path = dir.to_path_buf();
    path.push("notes (1).pdf");
    assert_eq!(staged_name(&path, Some(&dir)), "notes.pdf");
    // WebKit's name is never un-numbered: no folder is given to look in.
    assert_eq!(staged_name(&path, None), "notes (1).pdf");
}
