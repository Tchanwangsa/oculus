//! An old-layout library in a scratch data dir: one subject with every kind
//! of folder the move handles, two lecture folders, the data-dir records
//! that name library paths, and a database from before migration 63.

use std::path::Path;

use sqlx::SqlitePool;

use crate::test_support::{write_creating, Scratch};

pub const CODE: &str = "COMP1_2026";
pub const S: &str = "courses/COMP1_2026";
/// Where the library was when the lecture paths were stored: not this
/// data dir, so a run that followed them would miss.
pub const OLD_HOME: &str = "/elsewhere/Library/Application Support/com.tchan.oculus";

pub fn block<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(f)
}

pub fn file(dir: &Path, rel: &str, body: &str) {
    write_creating(dir.join(rel), body);
}

pub fn read(dir: &Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

pub fn exists(dir: &Path, rel: &str) -> bool {
    std::fs::symlink_metadata(dir.join(rel)).is_ok()
}

pub async fn pool(dir: &Path) -> SqlitePool {
    sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(dir.join("oculus.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap()
}

pub fn library(name: &str) -> Scratch {
    let dir = Scratch::new(name);
    disk(&dir);
    block(database(&dir));
    dir
}

fn disk(d: &Path) {
    let s = |rel: &str| format!("{S}/{rel}");
    file(d, &s("files/lec1.pdf"), "%PDF lec1");
    file(
        d,
        &s("files/lec1.md"),
        "# Lecture 1\n\n![](lec1_images/a.jpg)\n",
    );
    file(d, &s("files/lec1_images/a.jpg"), "jpg a");
    file(
        d,
        &s("files/lec1.pages.json"),
        r#"{"pdf":"lec1.pdf","pages":[]}"#,
    );
    file(
        d,
        &s("files/lec1.emb.json"),
        r#"{"pdf":"lec1.pdf","pages":[]}"#,
    );
    file(d, &s("files/clip.mp4"), "video");
    file(d, &s("files/clip.mp4.vtt"), "WEBVTT");
    file(d, &s("files/clip.vtt"), "WEBVTT");
    std::fs::create_dir_all(d.join(s("files/.lec1_images-staging-1-2/lec1_images"))).unwrap();
    file(
        d,
        &s("modules/01-intro.md"),
        "- [Lecture 1](../files/lec1.pdf)\n- [Welcome](../pages/welcome.md)\n- [Clip](../files/clip.mp4) _(video 9)_\n- [Web](https://example.com/x)\n",
    );
    file(d, &s("pages/welcome.md"), "![](../images/7.png)\n");
    file(d, &s("images/7.png"), "png");
    file(d, &s("home.md"), "![](images/7.png)\n");
    file(d, &s("lessons/1-intro.md"), "[q](1-intro/01-q/q.py)\n");
    file(d, &s("lessons/1-intro/01-q/q.py"), "print(1)\n");
    file(d, &s("ed/0001-x.md"), "a thread\n");
    // An upload, parsed.
    file(d, &s("uploads/scan.pdf"), "%PDF scan");
    file(d, &s("uploads/scan.md"), "![](scan_images/b.jpg)\n");
    file(d, &s("uploads/scan_images/b.jpg"), "jpg b");
    file(d, &s("uploads/scan.pages.json"), r#"{"pdf":"scan.pdf"}"#);
    file(
        d,
        &s("uploads/Course/thing.txt"),
        "the student's own Course folder",
    );
    file(d, &s("uploads/.DS_Store"), "finder");
    // A note with pictures, a link into the synced files and a picture by
    // absolute asset URL from wherever the library was.
    file(
        d,
        &s("documents/week1.md"),
        "![](assets/p.png) see [lec](../files/lec1.pdf)\n![x](asset://localhost/%2Felsewhere%2Fhome%2Fcourses%2FCOMP1_2026%2Ffiles%2Flec1_images%2Fa.jpg)\n",
    );
    file(d, &s("documents/assets/p.png"), "png p");
    // The same file name as the upload: it takes ` (2)` with its parse.
    file(d, &s("documents/scan.pdf"), "%PDF other scan");
    file(d, &s("documents/scan.md"), "![](scan_images/c.jpg)\n");
    file(d, &s("documents/scan_images/c.jpg"), "jpg c");
    file(
        d,
        &s("documents/scan.pages.json"),
        r#"{"pdf":"scan.pdf","pages":[{"markdown":"![](scan_images/c.jpg)"}]}"#,
    );
    file(d, &s("documents/sub/n.md"), "![](assets/q.png)\n");
    file(d, &s("documents/sub/assets/q.png"), "png q");
    file(d, &s("canvases/board.canvas"), "{}");
    file(d, &s("notebooks/nb.notebook"), "{}");
    // The agent folder: the memories link, the student's guidance, an
    // agent's notes, Finder's file.
    std::fs::create_dir_all(d.join(s("agents"))).unwrap();
    std::fs::create_dir_all(d.join("agents/memories").join(CODE)).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        format!("../../../agents/memories/{CODE}"),
        d.join(s("agents/memories")),
    )
    .unwrap();
    file(d, &s("agents/INSTRUCTIONS.md"), "be brief");
    file(d, &s("agents/notes.md"), "see [lec](../files/lec1.pdf)\n");
    file(d, &s("agents/.DS_Store"), "finder");
    std::fs::create_dir_all(d.join(s(".claude/.cc-writes"))).unwrap();
    // Lectures: one with media and a cut-short download, one with only that.
    file(d, "lectures/L1/source1.mp4", "mp4");
    file(d, "lectures/L1/transcript.vtt", "WEBVTT");
    file(d, "lectures/L1/raw2.mp4", "partial");
    file(d, "lectures/L1/frames/0001.jpg", "frame");
    file(d, "lectures/L1/outline.md", "![](frames/0001.jpg)\n");
    file(d, "lectures/L2/raw1.mp4", "partial");
    std::fs::create_dir_all(d.join("lectures/.claude/.cc-writes")).unwrap();
    // Records outside the library that name paths in it.
    file(
        d,
        "canvas-pages/14/pages.json",
        &format!(r#"{{"canvas":"{S}/notebooks/nb.notebook","pageCount":0,"pages":[]}}"#),
    );
    file(d, "courses/loose.pdf", "in no subject");
}

async fn database(d: &Path) {
    let pool = pool(d).await;
    crate::library::migrate::db::ensure_up_to(&pool, 61).await;
    let files = [
        (1, "files/lec1.pdf", "file"),
        (2, "files/clip.mp4", "file"),
        (3, "modules/01-intro.md", "module"),
        (4, "pages/welcome.md", "page"),
        (5, "images/7.png", "image"),
        (6, "home.md", "home"),
        (7, "lessons/1-intro.md", "lessons"),
        (8, "lessons/1-intro/01-q/q.py", "other"),
        (9, "ed/0001-x.md", "ed"),
        (10, "uploads/scan.pdf", "upload"),
        (11, "documents/week1.md", "document"),
        (12, "documents/scan.pdf", "upload"),
        (13, "canvases/board.canvas", "canvas"),
        (14, "notebooks/nb.notebook", "notebook"),
        (15, "documents/sub/n.md", "document"),
    ];
    let mut sql = format!("INSERT INTO subjects (id, code, name) VALUES (1, '{CODE}', 'One');");
    for (id, inner, category) in files {
        let name = inner.rsplit('/').next().unwrap();
        let ext = name.rsplit('.').next().unwrap();
        sql.push_str(&format!(
            "INSERT INTO files (id, subject_id, filename, relative_path, file_type, category)
             VALUES ({id}, 1, '{name}', '{S}/{inner}', '{ext}', '{category}');"
        ));
    }
    sql.push_str(&format!(
        "INSERT INTO pages (file_id, page_no, markdown) VALUES (1, 1, 'one'), (1, 2, 'two'), (1, 3, 'three');
         INSERT INTO document_versions (file_id, kind, text, hash) VALUES (11, 'auto', '![](assets/p.png) old', 'h');
         INSERT INTO sync_runs (id) VALUES (1);
         INSERT INTO sync_run_files (run_id, subject_id, relative_path, action) VALUES
           (1, 1, '{S}/files/lec1.pdf', 'new'), (1, 1, '{S}/files/lec1.pdf', 'updated'), (1, 1, '{S}/home.md', 'new');
         INSERT INTO revision_notes (subject_id, title, source_kind, source_ref)
           VALUES (1, 'Limits', 'file', '{S}/files/lec1.pdf#page=2');
         INSERT INTO chat_proposals (item_id, block, thread_id, kind, state, initial_state) VALUES
           (1, 0, 1, 'note', '{{\"tags\":[\"{S}/files/lec1.pdf\"],\"source\":\"lectures/L1\"}}', '{{}}');
         INSERT INTO agent_changes (thread_id, proposal_id, entity, entity_id, op, before, after)
           VALUES (1, 'p', 'note', '1', 'update', NULL, '{{\"source_ref\":\"{S}/uploads/scan.pdf\"}}');
         INSERT INTO echo_embeds (link_id, subject_id, media_id, secure_link_id, name, file_rel)
           VALUES ('k', 1, 'm', 's', 'Clip', '{S}/files/clip.mp4');
         INSERT INTO lectures (id, lesson_id, subject_id, title, date, video_path, transcript_path) VALUES
           ('L1', 'les1', 1, '{CODE} MO L101', '2026-03-02T10:00:00',
            '{OLD_HOME}/lectures/L1/source1.mp4', '{OLD_HOME}/lectures/L1/transcript.vtt'),
           ('L2', 'les2', 1, '{CODE} TU L102', '2026-03-03T10:00:00', NULL, NULL),
           ('L3', 'les3', 1, '{CODE} WE L103', '2026-03-04T10:00:00', NULL, NULL);"
    ));
    sqlx::raw_sql(&sql).execute(&pool).await.unwrap();
    pool.close().await;
}
