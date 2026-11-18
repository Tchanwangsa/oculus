//! Paths are built through the layout, so these hold for any folder names.

use std::path::Path;

use super::commands::notebook_cover;
use super::entries::FolderListing;
use super::listing::{list_folder_at, natural_cmp};
use super::recordings::Recording;
use crate::library::layout::{
    agents_rel, course_rel, derived_rel, in_course, own_dir_rel, synced, synced_dir, DerivedKind,
    OwnKind, SyncedKind,
};
use crate::test_support::Scratch;

const CODE: &str = "COMP30026_2026_SM2";

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn mkdir(root: &Path, rel: &str) {
    std::fs::create_dir_all(root.join(rel)).unwrap();
}

fn files_rel(name: &str) -> String {
    in_course(CODE, &synced(SyncedKind::Files, name))
}

fn uploads_rel(name: &str) -> String {
    format!("{}/{name}", own_dir_rel(CODE, OwnKind::Upload))
}

/// A parse record for the source at `rel`, where the parser keeps it.
fn record(root: &Path, rel: &str) {
    put(root, &derived_rel(rel, DerivedKind::Pages), b"{}");
}

fn list(root: &Path, rel: &str) -> FolderListing {
    list_folder_at(root, rel, false, &[]).unwrap()
}

fn file_names(listing: &FolderListing) -> Vec<&str> {
    listing.files.iter().map(|f| f.name.as_str()).collect()
}

fn folder_names(listing: &FolderListing) -> Vec<&str> {
    listing.folders.iter().map(|f| f.name.as_str()).collect()
}

#[test]
fn the_root_lists_subject_folders_only() {
    let root = Scratch::new("drive-root");
    put(&root, &files_rel("a.pdf"), b"%PDF");
    mkdir(&root, &course_rel("INFO30006_2026_SM2"));
    let subjects = parent(&course_rel(CODE));
    mkdir(&root, &format!("{subjects}/.hidden"));
    let loose = format!("{subjects}/cheatsheet.pdf");
    put(&root, &loose, b"%PDF");

    let listing = list(&root, "");
    assert_eq!(
        folder_names(&listing),
        ["COMP30026_2026_SM2", "INFO30006_2026_SM2"]
    );
    assert!(listing.files.is_empty());
    assert!(!listing.writable);
    assert_eq!(listing.folders[1].child_count, 0);
}

#[test]
fn a_subject_hides_app_state_dot_names_and_symlinks() {
    let root = Scratch::new("drive-subject");
    put(&root, &files_rel("a.pdf"), b"%PDF");
    put(&root, &format!("{}/memory.md", agents_rel(CODE)), b"x");
    put(&root, &format!("{}/.DS_Store", course_rel(CODE)), b"x");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        "../../agents/AGENTS.md",
        root.join(course_rel(CODE)).join("AGENTS.md"),
    )
    .unwrap();

    let listing = list(&root, &course_rel(CODE));
    let names = folder_names(&listing);
    let agents = agents_rel(CODE);
    assert!(!names.contains(&agents.rsplit('/').next().unwrap()));
    assert!(!file_names(&listing)
        .iter()
        .any(|n| n.starts_with('.') || *n == "AGENTS.md"));
    let files_dir = in_course(CODE, &synced_dir(SyncedKind::Files));
    let entry = listing
        .folders
        .iter()
        .find(|f| files_dir.starts_with(&f.relative_path))
        .expect("the synced folder holding files is listed");
    assert_eq!(entry.zone, "synced");
    assert!(!entry.editable);
}

#[test]
fn parse_artifacts_and_converted_pdfs_are_hidden_beside_their_source() {
    let root = Scratch::new("drive-artifacts");
    for name in [
        "a.pdf",
        "a.md",
        "b.docx",
        "b.docx.pdf",
        "b.docx.md",
        "c.xlsx",
        "c.xlsx.md",
        "readme.md",
        "zip.zip",
    ] {
        put(&root, &files_rel(name), b"x");
    }
    record(&root, &files_rel("a.pdf"));
    put(
        &root,
        &derived_rel(&files_rel("a.pdf"), DerivedKind::Embeddings),
        b"{}",
    );
    record(&root, &files_rel("b.docx.pdf"));
    put(&root, &files_rel("a_images/1.jpg"), b"x");
    put(&root, &files_rel("b.docx_images/1.jpg"), b"x");
    put(&root, &files_rel(".a_images-staging-1-2/x"), b"x");

    let listing = list(&root, &parent(&files_rel("a.pdf")));
    assert_eq!(
        file_names(&listing),
        ["a.pdf", "b.docx", "c.xlsx", "readme.md", "zip.zip"]
    );
    assert!(listing.folders.is_empty());
    assert!(!listing.writable);
    assert!(listing
        .files
        .iter()
        .all(|f| f.zone == "synced" && !f.editable));
}

#[test]
fn an_own_note_beside_an_unparsed_pdf_stays() {
    let root = Scratch::new("drive-own");
    put(&root, &uploads_rel("n.pdf"), b"%PDF");
    put(&root, &uploads_rel("n.md"), b"# mine");
    let dir = own_dir_rel(CODE, OwnKind::Upload);

    let listing = list(&root, &dir);
    assert_eq!(file_names(&listing), ["n.md", "n.pdf"]);
    assert!(listing.writable);
    assert!(listing.files.iter().all(|f| f.zone == "own" && f.editable));

    record(&root, &uploads_rel("n.pdf"));
    assert_eq!(file_names(&list(&root, &dir)), ["n.pdf"]);
}

#[test]
fn folders_carry_counts_and_sizes_and_sort_naturally() {
    let root = Scratch::new("drive-counts");
    put(&root, &uploads_rel("Week 10/a.pdf"), b"%PDF");
    put(&root, &uploads_rel("Week 2/a.pdf"), b"%PDF");
    put(&root, &uploads_rel("Week 2/a.md"), b"x");
    record(&root, &uploads_rel("Week 2/a.pdf"));
    put(&root, &uploads_rel("Week 2/b.txt"), b"hello");
    mkdir(&root, &uploads_rel("Empty"));

    let listing = list(&root, &own_dir_rel(CODE, OwnKind::Upload));
    assert_eq!(folder_names(&listing), ["Empty", "Week 2", "Week 10"]);
    let counts: Vec<u32> = listing.folders.iter().map(|f| f.child_count).collect();
    assert_eq!(counts, [0, 2, 1]);
    assert!(listing
        .folders
        .iter()
        .all(|f| f.editable && f.writable && f.zone == "own"));

    let week = list(&root, &uploads_rel("Week 2"));
    let txt = week.files.iter().find(|f| f.name == "b.txt").unwrap();
    assert_eq!(txt.size, 5);
    assert!(txt.modified.is_some());
}

#[test]
fn a_recursive_listing_is_flat_and_still_hides_artifacts() {
    let root = Scratch::new("drive-recursive");
    put(&root, &uploads_rel("x.pdf"), b"%PDF");
    put(&root, &uploads_rel("A/y.pdf"), b"%PDF");
    put(&root, &uploads_rel("A/y.md"), b"x");
    record(&root, &uploads_rel("A/y.pdf"));
    put(&root, &uploads_rel("A/B/z.txt"), b"x");
    put(&root, &uploads_rel(".hidden/w.txt"), b"x");

    let dir = own_dir_rel(CODE, OwnKind::Upload);
    let listing = list_folder_at(&root, &dir, true, &[]).unwrap();
    let mut files: Vec<&str> = listing
        .files
        .iter()
        .map(|f| f.relative_path.as_str())
        .collect();
    files.sort();
    let expect = [
        uploads_rel("A/B/z.txt"),
        uploads_rel("A/y.pdf"),
        uploads_rel("x.pdf"),
    ];
    assert_eq!(files, expect.iter().map(String::as_str).collect::<Vec<_>>());
    let a = listing.folders.iter().find(|f| f.name == "A").unwrap();
    assert_eq!(a.child_count, 2);
    assert!(listing.recordings.is_empty());

    let everything = list_folder_at(&root, "", true, &[]).unwrap();
    assert_eq!(everything.files.len(), 3);
    assert!(everything
        .folders
        .iter()
        .any(|f| f.relative_path == course_rel(CODE)));
}

#[test]
fn a_recording_folder_lists_as_its_lecture() {
    let root = Scratch::new("drive-recordings");
    let dir = parent(&files_rel("x"));
    let folder_rel = format!("{dir}/2026-03-02-intro");
    put(&root, &format!("{folder_rel}/source1.mp4"), b"x");
    mkdir(&root, &format!("{dir}/Other"));
    let recordings = [
        Recording {
            lecture_id: "m1".into(),
            subject_id: 7,
            title: "Intro".into(),
            date: "2026-03-02".into(),
            folder_rel: folder_rel.clone(),
        },
        // Not downloaded yet: it still lists.
        Recording {
            lecture_id: "m2".into(),
            subject_id: 7,
            title: "Week 2".into(),
            date: "2026-03-09".into(),
            folder_rel: format!("{dir}/2026-03-09-week-2"),
        },
        Recording {
            lecture_id: "elsewhere".into(),
            subject_id: 7,
            title: "x".into(),
            date: "2026-03-01".into(),
            folder_rel: format!("{dir}/Other/deeper"),
        },
    ];

    let listing = list_folder_at(&root, &dir, false, &recordings).unwrap();
    assert_eq!(folder_names(&listing), ["Other"]);
    let ids: Vec<&str> = listing
        .recordings
        .iter()
        .map(|r| r.lecture_id.as_str())
        .collect();
    assert_eq!(ids, ["m1", "m2"]);
    assert_eq!(listing.recordings[0].relative_path, folder_rel);
}

#[test]
fn paths_outside_what_drive_shows_are_refused() {
    let root = Scratch::new("drive-refuse");
    put(&root, &files_rel("a.pdf"), b"%PDF");
    put(&root, &format!("{}/memory.md", agents_rel(CODE)), b"x");
    mkdir(&root, &format!("{}/.oculus", course_rel(CODE)));
    mkdir(&root, "studio");

    for rel in [
        "/etc",
        "../x",
        "studio",
        &parent(&course_rel(CODE)),
        &agents_rel(CODE),
        &format!("{}/.oculus", course_rel(CODE)),
        &files_rel("a.pdf"),
        &format!("{}/missing", course_rel(CODE)),
    ] {
        assert!(list_folder_at(&root, rel, false, &[]).is_err(), "{rel}");
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_folder_is_refused() {
    let root = Scratch::new("drive-symlink");
    put(&root, &uploads_rel("real/a.txt"), b"x");
    let link = root.join(uploads_rel("link"));
    std::os::unix::fs::symlink(root.join(uploads_rel("real")), &link).unwrap();

    assert!(list_folder_at(&root, &uploads_rel("link"), false, &[]).is_err());
    let listing = list(&root, &own_dir_rel(CODE, OwnKind::Upload));
    assert_eq!(folder_names(&listing), ["real"]);
}

#[test]
fn names_sort_as_finder_does() {
    let mut names = vec!["week 10", "Week 2", "week 1", "Appendix", "a"];
    names.sort_by(|a, b| natural_cmp(a, b));
    assert_eq!(names, ["a", "Appendix", "week 1", "Week 2", "week 10"]);
}

#[test]
fn a_notebook_cover_is_its_first_page_picture() {
    let root = Scratch::new("drive-cover");
    assert!(notebook_cover(&root, 3).is_none());
    let dir = crate::library::paths::notebook_pages_dir(&root, 3);
    std::fs::create_dir_all(&dir).unwrap();
    let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
    std::fs::write(dir.join("page-002.png"), b"\x89PNG\r\n\x1a\nsecond").unwrap();
    std::fs::write(dir.join("page-001.png"), &png).unwrap();
    std::fs::write(
        dir.join("pages.json"),
        r#"{"whiteboard":"x","pageCount":2,"pages":[
            {"page":2,"id":"b","image":"page-002.png","updatedAt":1},
            {"page":1,"id":"a","image":"page-001.png","updatedAt":1}]}"#,
    )
    .unwrap();
    assert_eq!(notebook_cover(&root, 3), Some(png));
    assert!(notebook_cover(&root, 0).is_none());
}

fn parent(rel: &str) -> String {
    rel.rsplit_once('/').unwrap().0.to_string()
}
