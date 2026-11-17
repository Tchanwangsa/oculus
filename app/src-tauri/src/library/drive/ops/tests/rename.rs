use super::*;
use crate::library::drive::ops::error::Kind;
use crate::library::drive::ops::moves::rename_entry_at;
use crate::library::layout::{derived_rel, DerivedKind};

#[tokio::test]
async fn renaming_a_parsed_pdf_relinks_its_pictures_and_shifts_the_blocks() {
    let lib = Lib::new("drive-rename-parsed").await;
    let from = own(CODE, "a.pdf");
    let id = lib.parsed_pdf(&from).await;

    let moved = rename_entry_at(&lib.dir, &lib.pool, &from, "bb.pdf")
        .await
        .unwrap();
    let to = own(CODE, "bb.pdf");
    assert_eq!(moved[0].1, to);

    assert_eq!(lib.row_of(id).await.unwrap().2, "bb.pdf");
    assert_eq!(lib.pages_of(id).await, ["Intro ![](bb_images/x.png) end"]);
    assert!(lib.exists(&format!("{}/x.png", derived_rel(&to, DerivedKind::Images))));
    assert_eq!(
        lib.read(&derived_rel(&to, DerivedKind::Markdown)),
        "Intro ![](bb_images/x.png) end"
    );
    let record: serde_json::Value =
        serde_json::from_str(&lib.read(&derived_rel(&to, DerivedKind::Pages))).unwrap();
    assert_eq!(record["pdf"], "bb.pdf");
    let page = &record["pages"][0];
    assert_eq!(page["markdown"], "Intro ![](bb_images/x.png) end");
    let blocks: Vec<(u64, u64)> = page["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| (b["start"].as_u64().unwrap(), b["end"].as_u64().unwrap()))
        .collect();
    let text = "Intro ![](bb_images/x.png) end";
    assert_eq!(
        blocks,
        [(0, 5), (6, 26), (text.len() as u64 - 3, text.len() as u64)]
    );
    assert_eq!(&text[6..26], "![](bb_images/x.png)");
    assert!(crate::parse::parse_mode(&lib.path(&to)).is_some());
}

#[tokio::test]
async fn renaming_across_kinds_changes_the_category() {
    let lib = Lib::new("drive-rename-kind").await;
    let from = own(CODE, "Exam/notes.md");
    let id = lib.file(&from, b"# notes").await;
    assert_eq!(lib.row_of(id).await.unwrap().4, "document");

    rename_entry_at(&lib.dir, &lib.pool, &from, "notes.txt")
        .await
        .unwrap();

    let (_, rel, name, file_type, category) = lib.row_of(id).await.unwrap();
    assert_eq!(rel, own(CODE, "Exam/notes.txt"));
    assert_eq!(
        (name.as_str(), file_type.as_str(), category.as_str()),
        ("notes.txt", "txt", "upload")
    );
}

#[tokio::test]
async fn a_folder_rename_moves_its_rows() {
    let lib = Lib::new("drive-rename-folder").await;
    let id = lib.file(&own(CODE, "Exam/q.pdf"), b"%PDF").await;
    let new = rename_entry_at(&lib.dir, &lib.pool, &own(CODE, "Exam"), "Final exam")
        .await
        .unwrap();
    assert_eq!(new[0].1, own(CODE, "Final exam"));
    assert_eq!(
        lib.row_of(id).await.unwrap().1,
        own(CODE, "Final exam/q.pdf")
    );
}

#[tokio::test]
async fn bad_names_reserved_names_and_taken_names_are_refused() {
    let lib = Lib::new("drive-rename-names").await;
    let a = own(CODE, "a.pdf");
    lib.file(&a, b"%PDF").await;
    lib.file(&own(CODE, "b.pdf"), b"%PDF").await;
    for bad in ["", ".hidden", "x/y", "x\\y", "c:d", "nul\0", " pad"] {
        let error = rename_entry_at(&lib.dir, &lib.pool, &a, bad)
            .await
            .unwrap_err();
        assert_eq!(error.kind, Kind::InvalidName, "{bad:?}");
    }
    let error = rename_entry_at(&lib.dir, &lib.pool, &a, "Course")
        .await
        .unwrap_err();
    assert_eq!(
        error.kind,
        Kind::InvalidName,
        "reserved at the subject's root"
    );
    let error = rename_entry_at(&lib.dir, &lib.pool, &a, "b.pdf")
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::Exists);
    let synced = course_file("s.pdf");
    lib.put(&synced, b"%PDF");
    let error = rename_entry_at(&lib.dir, &lib.pool, &synced, "t.pdf")
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::ReadOnly);
    let error = rename_entry_at(&lib.dir, &lib.pool, &own(CODE, "gone.pdf"), "t.pdf")
        .await
        .unwrap_err();
    assert_eq!(error.kind, Kind::NotFound);
}

#[tokio::test]
async fn a_case_only_rename_is_not_a_clash() {
    let lib = Lib::new("drive-rename-case").await;
    let id = lib.file(&own(CODE, "notes.pdf"), b"%PDF").await;
    rename_entry_at(&lib.dir, &lib.pool, &own(CODE, "notes.pdf"), "Notes.pdf")
        .await
        .unwrap();
    assert_eq!(lib.row_of(id).await.unwrap().1, own(CODE, "Notes.pdf"));
    assert!(std::fs::read_dir(lib.path(&root(CODE)))
        .unwrap()
        .flatten()
        .any(|e| e.file_name() == "Notes.pdf"));
}
