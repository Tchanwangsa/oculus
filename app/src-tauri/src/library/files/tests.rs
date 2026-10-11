use std::path::Path;

use super::commands::file_has_content;
use super::documents::{
    document_asset_ref, document_assets_dir, document_name, note_temp_path, write_note,
};
use super::uploads::{free_name, step_aside};

#[test]
fn content_metadata_distinguishes_files_and_stays_inside_the_library() {
    let scratch = crate::test_support::Scratch::new("file-content");
    std::fs::write(scratch.join("empty.md"), b"").unwrap();
    std::fs::write(scratch.join("full.md"), b"markdown").unwrap();
    std::fs::create_dir(scratch.join("directory")).unwrap();
    assert!(!file_has_content(&scratch, "empty.md").unwrap());
    assert!(file_has_content(&scratch, "full.md").unwrap());
    assert!(!file_has_content(&scratch, "missing.md").unwrap());
    assert!(!file_has_content(&scratch, "directory").unwrap());
    assert!(file_has_content(&scratch, "../outside.md").is_err());
    assert!(file_has_content(&scratch, "/etc/passwd").is_err());
    #[cfg(unix)]
    {
        let outside = crate::test_support::Scratch::new("file-content-outside");
        std::fs::write(outside.join("private.md"), b"outside").unwrap();
        std::os::unix::fs::symlink(outside.join("private.md"), scratch.join("escape.md")).unwrap();
        assert!(file_has_content(&scratch, "escape.md").is_err());
    }
}

/// `assets/` sits beside the note and is not itself reachable as a note.
#[test]
fn a_note_s_pictures_sit_beside_it() {
    let note = Path::new("/data/courses/MULT20015/documents/Week 3.md");
    assert_eq!(
        document_assets_dir(note).unwrap(),
        Path::new("/data/courses/MULT20015/documents/assets")
    );
    let other = Path::new("/data/courses/MULT20015/documents/Ideas.md");
    assert_eq!(
        document_assets_dir(note).unwrap(),
        document_assets_dir(other).unwrap()
    );

    assert_eq!(
        document_asset_ref("20260922-101112-0a1b2c3d.png"),
        "assets/20260922-101112-0a1b2c3d.png"
    );
    assert!(!crate::library::paths::is_document_rel(
        "courses/MULT20015/documents/assets/20260922-101112-0a1b2c3d.png"
    ));
}

/// A save replaces the note whole and leaves no staging file; a leftover
/// staging name is never taken for a note.
#[test]
fn a_note_is_replaced_whole() {
    let dir = crate::test_support::Scratch::new("note-write");
    let note = dir.join("Week 3.md");
    std::fs::write(&note, b"a much longer old text").unwrap();
    write_note(&note, b"new").unwrap();
    assert_eq!(std::fs::read(&note).unwrap(), b"new");
    assert_eq!(std::fs::read_dir(&*dir).unwrap().count(), 1);

    let tmp = note_temp_path(&note).unwrap();
    let name = tmp.file_name().unwrap().to_str().unwrap();
    assert!(
        name.starts_with(".Week 3.md.") && name.ends_with(".tmp"),
        "{name}"
    );
    assert!(!crate::library::paths::is_document_rel(&format!(
        "courses/X/documents/{name}"
    )));
}

#[test]
fn an_upload_never_lands_on_a_name_already_taken() {
    let dir = crate::test_support::Scratch::new("uploads");

    assert_eq!(free_name(&dir, "notes.pdf", b"one"), "notes.pdf");
    std::fs::write(dir.join("notes.pdf"), b"one").unwrap();

    assert_eq!(free_name(&dir, "notes.pdf", b"one"), "notes.pdf");
    assert_eq!(free_name(&dir, "notes.pdf", b"two"), "notes-2.pdf");
    assert_eq!(free_name(&dir, "README", b"x"), "README");
}

#[test]
fn a_title_becomes_a_markdown_filename() {
    assert_eq!(document_name("Week 3 notes"), "Week_3_notes.md");
    assert_eq!(document_name("  padded  "), "padded.md");
    assert_eq!(document_name(""), "Untitled.md");
    assert_eq!(document_name("   "), "Untitled.md");
    assert_eq!(document_name("..."), "Untitled.md");
    assert_eq!(document_name("???"), "Untitled.md");
    assert_eq!(document_name("Draft."), "Draft.md");
    assert_eq!(document_name("../../etc/passwd"), "____etc_passwd.md");
    assert_eq!(document_name("a/b"), "a_b.md");
}

#[test]
fn a_new_document_never_lands_on_a_name_already_taken() {
    let dir = crate::test_support::Scratch::new("documents");

    let name = document_name("notes");
    assert_eq!(step_aside(&dir, &name, |_| false), "notes.md");
    std::fs::write(dir.join("notes.md"), b"").unwrap();

    assert_eq!(step_aside(&dir, &name, |_| false), "notes-2.md");
    std::fs::write(dir.join("notes-2.md"), b"some text").unwrap();
    assert_eq!(step_aside(&dir, &name, |_| false), "notes-3.md");
}
