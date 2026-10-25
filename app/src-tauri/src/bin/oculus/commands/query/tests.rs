use super::*;

#[test]
fn codes_with_spaces_match_however_they_are_typed() {
    assert!(matches_code("MULT20015_2026_SM2", "mult20015"));
    assert!(matches_code("CS 101 Fall 2026", "cs-101"));
    assert!(matches_code("CS 101", "CS_101"));
    assert!(!matches_code("CS 1010", "CS 101"));
}

#[test]
fn query_scopes_include_every_term_and_never_hide_an_unknown_code() {
    let subjects: Vec<store::SubjectRow> = [
        "COMP10001_2026_SM1",
        "COMP10001_2026_SM2",
        "MULT20015_2026_SM2",
    ]
    .iter()
    .enumerate()
    .map(|(n, code)| store::SubjectRow {
        id: n as i64 + 1,
        code: code.to_string(),
        name: String::new(),
        term_name: None,
        is_current: n != 0,
        selected: true,
        last_synced_at: None,
    })
    .collect();
    assert!(subject_ids(&subjects, &[]).unwrap().is_empty());
    assert_eq!(
        subject_ids(&subjects, &["COMP10001".into()]).unwrap(),
        [1, 2]
    );
    assert_eq!(
        subject_ids(&subjects, &["COMP10001_2026_SM2".into()]).unwrap(),
        [2]
    );
    assert!(subject_ids(&subjects, &["MISS10000".into()])
        .unwrap_err()
        .contains("no subject matched"));
}

#[test]
fn page_specs_cover_points_ranges_and_open_ends() {
    let spec = parse_page_spec("3,7-9,20-").unwrap();
    for wanted in [3, 7, 8, 9, 20, 4000] {
        assert!(page_wanted(&spec, wanted), "{wanted} should be selected");
    }
    for unwanted in [2, 4, 6, 10, 19] {
        assert!(!page_wanted(&spec, unwanted), "{unwanted} should not be");
    }
    assert!(parse_page_spec("9-4").is_err());
    assert!(parse_page_spec("twelve").is_err());
    assert!(parse_page_spec("").is_err());
}

#[test]
fn fixed_strings_do_not_read_as_patterns() {
    assert!(build_regex("a.c", false, false).unwrap().is_match("abc"));
    assert!(!build_regex("a.c", true, false).unwrap().is_match("abc"));
    assert!(build_regex("a.c", true, false).unwrap().is_match("A.C"));
    assert!(!build_regex("a.c", true, true).unwrap().is_match("A.C"));
}

fn file(code: &str, rel: &str) -> LibFile {
    LibFile {
        id: 0,
        code: code.to_string(),
        filename: rel.rsplit('/').next().unwrap().to_string(),
        relative_path: rel.to_string(),
        file_type: "pdf".to_string(),
        category: None,
        size_bytes: None,
        parse_status: None,
        indexed_pages: 0,
    }
}

#[test]
fn file_json_keeps_the_public_names_without_a_database_id() {
    let row = file("COMP30026", "courses/COMP30026/files/week-01.pdf");
    assert_eq!(
        serde_json::to_value(&row).unwrap(),
        serde_json::json!({
            "subject": "COMP30026",
            "path": "courses/COMP30026/files/week-01.pdf",
            "filename": "week-01.pdf",
            "file_type": "pdf",
            "category": null,
            "size_bytes": null,
            "parse_status": null,
            "indexed_pages": 0
        })
    );
}

/// A filename shared by two subjects resolves by full path, and is reported
/// rather than guessed otherwise.
#[test]
fn resolution_prefers_the_most_exact_tier() {
    let files = vec![
        file("COMP30026", "courses/COMP30026/files/week-01.pdf"),
        file("MULT20015", "courses/MULT20015/files/week-01.pdf"),
        file("MULT20015", "courses/MULT20015/files/notes.pdf"),
    ];
    assert_eq!(
        resolve_file(&files, "courses/MULT20015/files/week-01.pdf")
            .unwrap()
            .code,
        "MULT20015"
    );
    assert_eq!(resolve_file(&files, "notes.pdf").unwrap().code, "MULT20015");
    assert_eq!(resolve_file(&files, "NOTES.PDF").unwrap().code, "MULT20015");
    assert_eq!(
        resolve_file(&files, "COMP30026/files/week").unwrap().code,
        "COMP30026"
    );
    assert!(resolve_file(&files, "week-01.pdf").is_err());
    assert!(resolve_file(&files, "nothing-like-this").is_err());
}

fn filed(rel: &str, category: &str) -> LibFile {
    LibFile {
        category: Some(category.to_string()),
        ..file("COMP30026", rel)
    }
}

/// A category narrows before the match limit.
#[test]
fn categories_narrow_the_scan() {
    let rows = || {
        vec![
            filed("courses/COMP30026/ed/0001-teams.md", "ed"),
            filed(
                "courses/COMP30026/announcements/2026-07-14-welcome.md",
                "announcement",
            ),
            filed("courses/COMP30026/files/week-01.pdf", "file"),
        ]
    };

    let kept = filter_categories(rows(), &["ed".into()]).ok().unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].category.as_deref(), Some("ed"));

    // Repeatable, and case is not the caller's problem.
    let pair = filter_categories(rows(), &["ED".into(), "announcement".into()])
        .ok()
        .unwrap();
    assert_eq!(pair.len(), 2);

    // No flag is every category, not none.
    assert_eq!(filter_categories(rows(), &[]).ok().unwrap().len(), 3);
}

/// A word that is not a category is refused; a real category these rows lack
/// is an empty answer.
#[test]
fn a_typo_is_refused_but_an_honest_miss_is_empty() {
    let rows = || vec![filed("courses/COMP30026/ed/0001-teams.md", "ed")];

    let err = filter_categories(rows(), &["eds".into()]).err().unwrap();
    assert!(err.contains("no category \"eds\""), "{err}");
    // The refusal names the real ones, from the one list that defines them.
    assert!(err.contains("announcement"), "{err}");
    assert!(err.contains("quiz"), "{err}");

    // Real category, none in these rows: empty, not an error.
    assert!(filter_categories(rows(), &["quiz".into()])
        .ok()
        .unwrap()
        .is_empty());

    // An empty row set does not excuse the typo.
    assert!(filter_categories(vec![], &["eds".into()]).is_err());
    assert!(filter_categories(vec![], &["quiz".into()])
        .ok()
        .unwrap()
        .is_empty());
}

/// Both commands must accept the same words and offer the same list.
#[test]
fn the_category_help_lists_what_the_flag_accepts() {
    let help = category_help();
    for c in paths::CATEGORIES {
        assert!(help.contains(c), "{c:?} missing from {help:?}");
    }
}

mod notebook_read {
    use super::super::notebook::{age, notebook_view};
    use app_lib::library::paths::{
        notebook_page_image, notebook_pages_dir, NotebookPage, NotebookPages, PAGES_MANIFEST,
    };

    fn root(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("oculus-cli-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// A notebook with `pages` pictures written the way the app writes them.
    fn written(root: &std::path::Path, id: i64, pages: usize, missing: &[usize]) {
        let dir = notebook_pages_dir(root, id);
        std::fs::create_dir_all(&dir).unwrap();
        let entries: Vec<NotebookPage> = (1..=pages)
            .map(|page| NotebookPage {
                page,
                id: format!("p{page}"),
                image: notebook_page_image(page),
                updated_at: 1_000 + page as u64,
            })
            .collect();
        for entry in entries.iter().filter(|e| !missing.contains(&e.page)) {
            std::fs::write(dir.join(&entry.image), b"png").unwrap();
        }
        let manifest = NotebookPages {
            canvas: "courses/X/canvases/Work.canvas".into(),
            page_count: pages,
            pages: entries,
        };
        std::fs::write(
            dir.join(PAGES_MANIFEST),
            serde_json::to_string(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn lists_every_page_with_its_picture_and_age() {
        let root = root("nb-list");
        written(&root, 7, 3, &[]);
        let view = notebook_view(&root, 7, "courses/X/canvases/Work.canvas", None, 1_060).unwrap();
        assert_eq!(view.page_count, 3);
        assert_eq!(
            view.pages.iter().map(|p| p.page).collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert!(std::path::Path::new(&view.pages[0].image)
            .ends_with(std::path::Path::new("canvas-pages/7/page-001.png")));
        assert_eq!(view.pages[0].age_seconds, 59);
        assert_eq!(view.pages[2].age_seconds, 57);
    }

    #[test]
    fn pages_narrows_the_list_and_a_missing_page_names_the_range() {
        let root = root("nb-pages");
        written(&root, 7, 3, &[]);
        let path = "courses/X/canvases/Work.canvas";
        let two = notebook_view(&root, 7, path, Some(&[(2, 2)]), 2_000).unwrap();
        assert_eq!(two.pages.iter().map(|p| p.page).collect::<Vec<_>>(), [2]);
        let error = notebook_view(&root, 7, path, Some(&[(9, 9)]), 2_000)
            .err()
            .unwrap();
        assert!(error.contains("has pages 1-3"), "{error}");
    }

    #[test]
    fn a_notebook_with_no_pictures_yet_says_how_to_get_them() {
        let root = root("nb-none");
        let error = notebook_view(&root, 7, "courses/X/canvases/Work.canvas", None, 1)
            .err()
            .unwrap();
        assert!(
            error.contains("no page pictures yet") && error.contains("open it there"),
            "{error}"
        );
    }

    #[test]
    fn a_picture_that_has_gone_missing_is_left_out_not_listed() {
        let root = root("nb-missing");
        written(&root, 7, 3, &[2]);
        let view = notebook_view(&root, 7, "courses/X/canvases/Work.canvas", None, 5_000).unwrap();
        assert_eq!(
            view.pages.iter().map(|p| p.page).collect::<Vec<_>>(),
            [1, 3]
        );
        written(&root, 8, 1, &[1]);
        assert!(notebook_view(&root, 8, "courses/X/canvases/Work.canvas", None, 5_000).is_err());
    }

    #[test]
    fn ages_read_naturally() {
        assert_eq!(age(0), "just now");
        assert_eq!(age(30), "30s ago");
        assert_eq!(age(150), "2m ago");
        assert_eq!(age(7_300), "2h ago");
        assert_eq!(age(200_000), "2d ago");
    }
}
