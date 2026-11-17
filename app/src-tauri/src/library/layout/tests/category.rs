use crate::library::layout::{
    category_of, synced, synced_file, SyncedFile, SyncedKind, CATEGORIES,
};

/// The cases the TypeScript mirror is checked against too.
const FIXTURE: &str = include_str!("../../../../../tests/fixtures/library/categories.json");

#[test]
fn the_shared_fixture_holds() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let path = case[0].as_str().unwrap();
        let (_, inner) = crate::library::layout::split_course(path).unwrap();
        assert_eq!(category_of(inner), case[1].as_str().unwrap(), "{path}");
    }
}

#[test]
fn every_listed_category_is_reachable_and_nothing_else() {
    let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    let hit: Vec<&str> = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            category_of(crate::library::layout::within_course(
                c[0].as_str().unwrap(),
            ))
        })
        .collect();
    for c in &hit {
        assert!(CATEGORIES.contains(c), "{c:?} is not in CATEGORIES");
    }
    for c in CATEGORIES {
        assert!(
            hit.contains(c),
            "{c:?} is listed but no fixture path produces it"
        );
    }
}

#[test]
fn what_the_sync_writes_is_labelled_by_its_kind() {
    for kind in [
        SyncedKind::Files,
        SyncedKind::Media,
        SyncedKind::Lessons,
        SyncedKind::Discussion,
        SyncedKind::Pages,
        SyncedKind::Assignments,
        SyncedKind::Quizzes,
        SyncedKind::Announcements,
        SyncedKind::Modules,
        SyncedKind::Images,
    ] {
        assert_eq!(
            category_of(&synced(kind, "x.md")),
            kind.category(),
            "{kind:?}"
        );
    }
    for file in [SyncedFile::Home, SyncedFile::Syllabus] {
        assert_eq!(category_of(&synced_file(file)), file.category());
    }
    assert_eq!(synced(SyncedKind::Files, "a.pdf"), "files/a.pdf");
    assert_eq!(synced(SyncedKind::Media, "v.mp4"), "files/v.mp4");
    assert_eq!(synced(SyncedKind::Discussion, "0001-x.md"), "ed/0001-x.md");
    assert_eq!(synced_file(SyncedFile::Home), "home.md");
}
