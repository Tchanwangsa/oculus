//! The whole move over the fixture library, checked on disk and in the
//! database.

use sqlx::Row;

use super::fixture::{block, exists, library, pool, read, CODE, S};
use crate::library::migrate::{dry_run, resolve_moved, run, Hooks, Outcome};

fn s(rel: &str) -> String {
    format!("{S}/{rel}")
}

const RECORDING: &str = "courses/COMP1_2026/Course/Recordings/2026-03-02-mo-l101";

#[test]
fn an_old_library_moves_into_the_tree() {
    let d = library("migrate-run");

    let plan = dry_run(&d).unwrap().expect("a plan");
    assert!(!plan.steps.is_empty());
    assert!(exists(&d, &s("files/lec1.pdf")), "a dry run moves nothing");
    assert!(!exists(&d, ".library-move"), "a dry run writes nothing");

    let Outcome::Moved(summary) = run(&d, &Hooks::default()).unwrap() else {
        panic!("expected a move");
    };
    assert!(summary.line().contains("moved the library"));

    // Synced folders and files into Course/, with their old names gone.
    for rel in [
        "Course/Files/lec1.pdf",
        "Course/Files/lec1.md",
        "Course/Files/lec1_images/a.jpg",
        "Course/Modules/01-intro.md",
        "Course/Pages/welcome.md",
        "Course/Images/7.png",
        "Course/home.md",
        "Course/Lessons/1-intro/01-q/q.py",
        "Course/Discussion/0001-x.md",
    ] {
        assert!(exists(&d, &s(rel)), "{rel}");
    }
    for old in [
        "files", "modules", "pages", "images", "home.md", "lessons", "ed",
    ] {
        assert!(!exists(&d, &s(old)), "{old} is gone");
    }
    // A video and both its captions into Media/.
    for rel in [
        "Course/Media/clip.mp4",
        "Course/Media/clip.mp4.vtt",
        "Course/Media/clip.vtt",
    ] {
        assert!(exists(&d, &s(rel)), "{rel}");
    }
    // Parse records into .oculus/derived/, at the PDF's folder path.
    assert!(exists(
        &d,
        &s(".oculus/derived/Course/Files/lec1.pages.json")
    ));
    assert!(exists(&d, &s(".oculus/derived/Course/Files/lec1.emb.json")));
    assert!(!exists(&d, &s("Course/Files/lec1.pages.json")));
    assert!(exists(&d, &s(".oculus/derived/scan.pages.json")));
    // Debris deleted.
    assert!(!exists(&d, &s("Course/Files/.lec1_images-staging-1-2")));
    assert!(!exists(&d, &s(".claude")));

    // Own folders flattened; the clash renamed with its parse; the
    // student's `Course` stepped aside.
    for rel in [
        "scan.pdf",
        "scan.md",
        "scan_images/b.jpg",
        "week1.md",
        ".assets/p.png",
        "scan (2).pdf",
        "scan (2).md",
        "scan (2)_images/c.jpg",
        "sub/n.md",
        "sub/.assets/q.png",
        "board.canvas",
        "nb.notebook",
        "Course (mine)/thing.txt",
    ] {
        assert!(exists(&d, &s(rel)), "{rel}");
    }
    for old in ["uploads", "documents", "canvases", "notebooks", "agents"] {
        assert!(!exists(&d, &s(old)), "{old} is gone");
    }
    assert_eq!(
        read(&d, &s("scan (2).md")),
        "![](<scan (2)_images/c.jpg>)\n"
    );
    let record: serde_json::Value =
        serde_json::from_str(&read(&d, &s(".oculus/derived/scan (2).pages.json"))).unwrap();
    assert_eq!(record["pdf"], "scan (2).pdf");
    assert_eq!(record["pages"][0]["markdown"], "![](scan (2)_images/c.jpg)");

    // Links re-pointed as the sync now writes them.
    assert_eq!(
        read(&d, &s("Course/Modules/01-intro.md")),
        "- [Lecture 1](../Files/lec1.pdf)\n- [Welcome](../Pages/welcome.md)\n- [Clip](../Media/clip.mp4) _(video 9)_\n- [Web](https://example.com/x)\n"
    );
    assert_eq!(
        read(&d, &s("Course/Pages/welcome.md")),
        "![](../Images/7.png)\n"
    );
    assert_eq!(read(&d, &s("Course/home.md")), "![](Images/7.png)\n");
    assert_eq!(
        read(&d, &s("Course/Files/lec1.md")),
        "# Lecture 1\n\n![](lec1_images/a.jpg)\n"
    );
    assert_eq!(read(&d, &s("sub/n.md")), "![](.assets/q.png)\n");
    let week1 = read(&d, &s("week1.md"));
    assert!(
        week1.starts_with("![](.assets/p.png) see [lec](Course/Files/lec1.pdf)\n"),
        "{week1}"
    );
    let data = crate::library::paths::slash_path(&*d).replace('/', "%2F");
    assert!(
        week1.contains(&format!(
        "asset://localhost/{data}%2Fcourses%2FCOMP1_2026%2FCourse%2FFiles%2Flec1_images%2Fa.jpg"
    )),
        "{week1}"
    );

    // The agent folder sorted; the memories link remade at its new depth.
    assert_eq!(read(&d, &s(".oculus/agents/INSTRUCTIONS.md")), "be brief");
    assert_eq!(
        read(&d, &s("Agent notes/notes.md")),
        "see [lec](../Course/Files/lec1.pdf)\n"
    );
    #[cfg(unix)]
    {
        let link = std::fs::read_link(d.join(s(".oculus/agents/memories"))).unwrap();
        assert_eq!(
            link,
            std::path::Path::new(&format!("../../../../agents/memories/{CODE}"))
        );
        assert!(
            d.join(s(".oculus/agents/memories")).is_dir(),
            "the link resolves"
        );
    }

    // Lectures: the recording into Recordings/, the partial dropped, the
    // emptied folder gone, the old root gone.
    assert!(exists(&d, &format!("{RECORDING}/source1.mp4")));
    assert!(exists(&d, &format!("{RECORDING}/frames/0001.jpg")));
    assert!(!exists(&d, &format!("{RECORDING}/raw2.mp4")));
    assert!(!exists(&d, "lectures"));

    // Records outside the library.
    assert!(read(&d, "canvas-pages/14/pages.json").contains(&s("nb.notebook")));
    assert!(exists(&d, "courses/loose.pdf"), "loose files stay");
    assert!(summary.left.iter().any(|l| l.contains("loose.pdf")));

    block(async {
        let pool = pool(&d).await;
        let row = |id: i64| {
            let pool = pool.clone();
            async move {
                sqlx::query("SELECT relative_path, category, filename FROM files WHERE id = ?1")
                    .bind(id)
                    .fetch_one(&pool)
                    .await
                    .map(|r| {
                        (
                            r.get::<String, _>(0),
                            r.get::<String, _>(1),
                            r.get::<String, _>(2),
                        )
                    })
                    .unwrap()
            }
        };
        assert_eq!(row(1).await.0, s("Course/Files/lec1.pdf"));
        assert_eq!(row(2).await.0, s("Course/Media/clip.mp4"));
        assert_eq!(
            row(8).await.1,
            "lessons",
            "a lesson's other file is a lesson now"
        );
        assert_eq!(
            row(10).await,
            (s("scan.pdf"), "upload".into(), "scan.pdf".into())
        );
        assert_eq!(
            row(12).await,
            (s("scan (2).pdf"), "upload".into(), "scan (2).pdf".into())
        );
        assert_eq!(row(11).await.1, "document");
        // Every row is on disk.
        let paths: Vec<String> = sqlx::query_scalar("SELECT relative_path FROM files")
            .fetch_all(&pool)
            .await
            .unwrap();
        for p in &paths {
            assert!(exists(&d, p), "row {p} on disk");
        }
        // The pages kept their file, in place.
        let pages: Vec<(i64, String)> = sqlx::query_as(
            "SELECT p.page_no, f.relative_path FROM pages p JOIN files f ON f.id = p.file_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(pages.len(), 3);
        assert!(pages.iter().all(|(_, p)| *p == s("Course/Files/lec1.pdf")));
        let version: String = sqlx::query_scalar("SELECT text FROM document_versions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(version, "![](.assets/p.png) old");
        let run_files: Vec<String> =
            sqlx::query_scalar("SELECT relative_path FROM sync_run_files ORDER BY id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            run_files,
            [
                s("Course/Files/lec1.pdf"),
                s("Course/Files/lec1.pdf"),
                s("Course/home.md")
            ]
        );
        let source: String = sqlx::query_scalar("SELECT source_ref FROM revision_notes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(source, s("Course/Files/lec1.pdf#page=2"));
        let state: String = sqlx::query_scalar("SELECT state FROM chat_proposals")
            .fetch_one(&pool)
            .await
            .unwrap();
        let state: serde_json::Value = serde_json::from_str(&state).unwrap();
        assert_eq!(state["tags"][0], s("Course/Files/lec1.pdf"));
        assert_eq!(state["source"], RECORDING);
        let after: String = sqlx::query_scalar("SELECT after FROM agent_changes")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(after.contains(&s("scan.pdf")) && !after.contains("uploads"));
        let embed: String = sqlx::query_scalar("SELECT file_rel FROM echo_embeds")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(embed, s("Course/Media/clip.mp4"));

        // Lectures: the folder on the row, the stored paths into it here.
        let l1 = sqlx::query(
            "SELECT folder_rel, video_path, transcript_path FROM lectures WHERE id = 'L1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(l1.get::<String, _>(0), RECORDING);
        let video: String = l1.get(1);
        assert_eq!(
            video,
            d.join(RECORDING).join("source1.mp4").to_string_lossy()
        );
        assert!(std::path::Path::new(&video).is_file());
        assert!(std::path::Path::new(&l1.get::<String, _>(2)).is_file());
        let empty: Vec<Option<String>> =
            sqlx::query_scalar("SELECT folder_rel FROM lectures WHERE id IN ('L2', 'L3')")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(empty, [None, None], "no folder, no folder_rel");

        // Old citations resolve.
        for (old, new) in [
            (s("files/lec1.pdf:L4"), s("Course/Files/lec1.pdf:L4")),
            (s("documents/scan.pdf#page=2"), s("scan (2).pdf#page=2")),
            (s("files/clip.mp4"), s("Course/Media/clip.mp4")),
            (
                "lectures/L1/frames/0001.jpg".into(),
                format!("{RECORDING}/frames/0001.jpg"),
            ),
            (s("agents/notes.md"), s("Agent notes/notes.md")),
            (
                s("files/lec1.pages.json"),
                s(".oculus/derived/Course/Files/lec1.pages.json"),
            ),
        ] {
            assert_eq!(
                resolve_moved(&pool, &old).await.unwrap(),
                Some(new),
                "{old}"
            );
        }
        assert_eq!(
            resolve_moved(&pool, &s("Course/Files/lec1.pdf"))
                .await
                .unwrap(),
            None
        );
        let layout: String =
            sqlx::query_scalar("SELECT value FROM settings WHERE key = 'library_layout'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(layout, "drive");
    });

    // A second run, and a dry run, do nothing.
    assert!(matches!(
        run(&d, &Hooks::default()).unwrap(),
        Outcome::AlreadyDone
    ));
    assert!(dry_run(&d).unwrap().is_none());
}
