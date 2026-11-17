//! Source metadata reaches the database even when the content and manifest match.

use std::sync::{Arc, Mutex};

use serde_json::json;
use sqlx::{Row, SqlitePool};

use super::{answer, rig};
use crate::db::store::{upsert_file, FileRow};
use crate::sync::{Engine, FileEvent, Reporter, Subject, SyncOptions};

struct Files(Arc<Mutex<Vec<FileEvent>>>);
impl Reporter for Files {
    fn file(&self, file: &FileEvent) {
        self.0.lock().unwrap().push(file.clone());
    }
}

fn utc(day: u8) -> String {
    format!("2026-10-{day:02}T00:00:00.000000000Z")
}

async fn record(pool: &SqlitePool, event: &FileEvent) {
    upsert_file(
        pool,
        &FileRow {
            subject_id: event.subject_id,
            relative_path: &event.relative_path,
            size_bytes: event.size_bytes,
            category: &event.category,
            canvas_id: event.canvas_id,
            source_url: event.source_url.as_deref(),
            source_created_at: event.source_dates.source_created_at.as_deref(),
            source_updated_at: event.source_dates.source_updated_at.as_deref(),
            changed: event.action != "unchanged",
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn every_canvas_phase_backfills_unchanged_rows_without_changing_local_recency() {
    let r = rig(|header, _| {
        let path = header["path"].as_str().unwrap_or("");
        let body = if path.starts_with("/api/v1/courses/1?") {
            json!({"name": "Subject", "syllabus_body": "<p>Syllabus</p>", "created_at": utc(1), "updated_at": utc(9)})
        } else if path == "/api/v1/courses/1/front_page" {
            json!({"body": "<p>Welcome</p>", "created_at": utc(1), "updated_at": utc(2)})
        } else if path.starts_with("/api/v1/courses/1/discussion_topics?") {
            json!([{"title": "Hello", "message": "<p>Hello</p>", "posted_at": utc(3), "created_at": utc(1)}])
        } else if path.starts_with("/api/v1/courses/1/quizzes?") {
            json!([{"id": 7, "title": "Quiz", "created_at": utc(1), "unlock_at": utc(4), "updated_at": utc(5)}])
        } else if path.starts_with("/api/v1/courses/1/assignments?") {
            json!([{"id": 8, "name": "Essay", "created_at": utc(2), "updated_at": utc(6)}])
        } else if path.starts_with("/api/v1/courses/1/modules?") {
            json!([{"name": "Week", "position": 1, "items": [
                {"type": "Page", "page_url": "intro", "title": "Intro"},
                {"type": "File", "content_id": 10, "title": "Notes"},
                {"type": "File", "content_id": 11, "title": "Video"}
            ]}])
        } else if path == "/api/v1/courses/1/pages/intro" {
            json!({"title": "Intro", "body": "<p>Intro</p>", "created_at": utc(2), "updated_at": utc(3)})
        } else if path == "/api/v1/files/10" {
            json!({"filename": "notes.pdf", "content-type": "application/pdf", "size": 8,
                "created_at": utc(1), "unlock_at": utc(2), "modified_at": utc(3)})
        } else if path == "/api/v1/files/11" {
            json!({"filename": "video.mp4", "content-type": "video/mp4", "size": 999,
                "created_at": utc(1), "unlock_at": utc(2), "modified_at": utc(3)})
        } else if path == "/api/v1/files/10/public_url" {
            json!({"public_url": "/download/10"})
        } else if path == "/download/10" {
            return answer(200, &[("content-type", "application/pdf")], b"%PDF-fixture");
        } else {
            panic!("unexpected request: {path}");
        };
        answer(
            200,
            &[("content-type", "application/json")],
            &serde_json::to_vec(&body).unwrap(),
        )
    });
    // Downloaded by the student, with no manifest entry for these remote bytes.
    let video = r.dir.join("courses/SUBJ/files/video.mp4");
    std::fs::create_dir_all(video.parent().unwrap()).unwrap();
    std::fs::write(&video, b"video").unwrap();
    let events = Arc::new(Mutex::new(Vec::new()));
    let engine = || {
        Engine::new(&r.dir, Box::new(Files(events.clone())))
            .with_pdf_parsing(false)
            .with_options(SyncOptions {
                ed: false,
                ed_lessons: false,
                ..SyncOptions::default()
            })
    };
    let subject = Subject {
        id: 1,
        code: "SUBJ".into(),
    };
    engine().scrape(std::slice::from_ref(&subject));
    let first = std::mem::take(&mut *events.lock().unwrap());
    assert_eq!(first.len(), 9);

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
        .execute(&pool)
        .await
        .unwrap();
    for event in &first {
        record(&pool, event).await;
    }
    // Model existing rows immediately after migration, keeping their local dates.
    sqlx::query(
        "UPDATE files SET source_created_at = NULL, source_updated_at = NULL,
        first_seen_at = 'first', content_changed_at = 'content', modified_at = 'modified'",
    )
    .execute(&pool)
    .await
    .unwrap();
    engine().scrape(std::slice::from_ref(&subject));
    let second = std::mem::take(&mut *events.lock().unwrap());
    assert_eq!(second.len(), first.len());
    for event in &second {
        assert_eq!(event.action, "unchanged", "{}", event.relative_path);
        if event.canvas_id == Some(11) {
            assert_eq!(event.size_bytes, 5, "keep the size of the downloaded bytes");
        }
        record(&pool, event).await;
        let row = sqlx::query("SELECT * FROM files WHERE relative_path = ?1")
            .bind(&event.relative_path)
            .fetch_one(&pool)
            .await
            .unwrap();
        let (created, updated) = match event.relative_path.split('/').nth(2).unwrap() {
            "home.md" => (Some(utc(1)), Some(utc(2))),
            "announcements" => (Some(utc(3)), Some(utc(3))),
            "quizzes" => (Some(utc(4)), Some(utc(5))),
            "assignments" => (Some(utc(2)), Some(utc(6))),
            "pages" | "files" => (Some(utc(2)), Some(utc(3))),
            "modules" | "syllabus.md" => (None, None),
            other => panic!("unexpected artifact: {other}"),
        };
        assert_eq!(row.get::<Option<String>, _>("source_created_at"), created);
        assert_eq!(row.get::<Option<String>, _>("source_updated_at"), updated);
        assert_eq!(row.get::<String, _>("first_seen_at"), "first");
        assert_eq!(row.get::<String, _>("content_changed_at"), "content");
        assert_eq!(row.get::<String, _>("modified_at"), "modified");
        let mut undated = event.clone();
        undated.source_dates = Default::default();
        record(&pool, &undated).await;
        let preserved: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT source_created_at, source_updated_at FROM files WHERE relative_path = ?1",
        )
        .bind(&event.relative_path)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(preserved, (created, updated));
    }
    assert_eq!(
        r.paths()
            .iter()
            .filter(|path| *path == "/download/10")
            .count(),
        1,
        "the manifest skip backfills dates without downloading again"
    );
}
