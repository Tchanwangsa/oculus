//! Exercise the registered schema upgrade against an isolated legacy library.
//! No user database, credentials, external services, or GUI are needed.
use sqlx::{Connection, Executor, Row, SqliteConnection};

fn migrations() -> Vec<(u32, &'static str)> {
    // Use the app's actual registration, including SQL constants, rather than
    // maintaining a second schema fixture that can silently drift from it.
    include_str!("../src/lib.rs")
        .split("tauri_plugin_sql::Migration {")
        .skip(1)
        .map(|entry| {
            let version = entry.split_once("version:").unwrap().1
                .split_once(',').unwrap().0.trim().parse().unwrap();
            let sql = match version {
                35 => app_lib::retrieval::PAGES_FTS_SQL,
                37 => app_lib::projects::UNFILED_TASKS_SQL,
                _ => entry.split_once("sql: r#\"").unwrap().1
                    .split_once("\"#,").unwrap().0,
            };
            (version, sql)
        })
        .collect()
}

async fn apply(connection: &mut SqliteConnection, version: u32, sql: &str) {
    // SQLx's migrator executes each migration in its own transaction with
    // foreign keys enabled, including the self-referencing tasks table rebuild.
    let mut tx = connection.begin().await.unwrap();
    tx.execute(sql).await.unwrap_or_else(|error| panic!("migration {version}: {error}"));
    tx.commit().await.unwrap();
}

async fn legacy_library() -> SqliteConnection {
    let mut connection = SqliteConnection::connect("sqlite::memory:").await.unwrap();
    connection.execute("PRAGMA foreign_keys = ON").await.unwrap();
    let migrations = migrations();
    assert_eq!(migrations.iter().map(|(v, _)| *v).collect::<Vec<_>>(), (1..=37).collect::<Vec<_>>());
    for (version, sql) in migrations.into_iter().filter(|(v, _)| *v <= 33) {
        apply(&mut connection, version, sql).await;
    }
    connection.execute(r#"
INSERT INTO subjects (id, code, name, term_name, is_current)
VALUES (1, 'TEST30001', 'Preserved subject', '2026 Semester 2', 1);
INSERT INTO files (id, subject_id, filename, relative_path, file_type, parse_status, embed_status)
VALUES (4, 1, 'lecture.pdf', 'Lectures/lecture.pdf', 'pdf', 'done', 'done');
INSERT INTO pages (id, file_id, page_no, markdown, embedding, embed_model, embed_dim, embedded_at)
VALUES (12, 4, 1, 'Eigenvalues survive the upgrade', X'003C0000', 'legacy-embedding', 2, '2026-09-18');
INSERT INTO projects (id, subject_id, name, columns, tags, event_id)
VALUES (8, 1, 'My assignment', '[{"id":"todo","name":"Todo","kind":"active"}]', '["revision"]', 'local_9');
INSERT INTO project_tasks (id, project_id, parent_id, title, body, column_id, position, due_at, source)
VALUES (21, 8, NULL, 'Parent task', 'Keep my own notes', 'todo', 2.5, '2026-10-01', 'manual'),
       (22, 8, 21, 'Child task', 'Keep subtask notes', 'todo', 3.5, '2026-09-30', 'manual');
INSERT INTO lectures (id, lesson_id, subject_id, title, date, progress_seconds, recap_status, last_watched_at)
VALUES ('lecture-one', 'lesson-one', 1, 'Lecture one', '2026-09-17', 123, 'ready', '2026-09-18');
INSERT INTO lecture_recap (lecture_id, idx, start_seconds, label, body)
VALUES ('lecture-one', 0, 0, 'Old derived recap', 'This can be regenerated');
INSERT INTO harness_threads (id, provider, title, subject_id, lecture_id)
VALUES (5, 'codex', 'Keep my conversation', 1, 'lecture-one');
INSERT INTO harness_items (thread_id, kind, content) VALUES (5, 'user', 'Keep my question');
INSERT INTO settings (key, value) VALUES
('job_models', '{"lectureRecap":{"provider":"codex","model":"my-selected-model"},"other":{"model":"unchanged"}}');
"#).await.unwrap();
    connection
}

#[tokio::test]
async fn legacy_windows_library_preserves_tasks_pages_and_settings_through_upgrade() {
    let mut connection = legacy_library().await;
    for (version, sql) in migrations().into_iter().filter(|(v, _)| *v > 33) {
        apply(&mut connection, version, sql).await;
    }

    let tasks = sqlx::query("SELECT id, project_id, parent_id, title, body, position FROM project_tasks ORDER BY id")
        .fetch_all(&mut connection).await.unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].get::<i64, _>("id"), 21);
    assert_eq!(tasks[0].get::<i64, _>("project_id"), 8);
    assert_eq!(tasks[0].get::<String, _>("body"), "Keep my own notes");
    assert_eq!(tasks[0].get::<f64, _>("position"), 2.5);
    assert_eq!(tasks[1].get::<Option<i64>, _>("parent_id"), Some(21));
    assert_eq!(tasks[1].get::<String, _>("title"), "Child task");
    let project = sqlx::query("SELECT tags, event_id FROM projects WHERE id = 8").fetch_one(&mut connection).await.unwrap();
    assert_eq!(project.get::<String, _>("tags"), "[\"revision\"]");
    assert_eq!(project.get::<String, _>("event_id"), "local_9");

    let page = sqlx::query("SELECT markdown, embedding, embed_model, embed_dim FROM pages WHERE id = 12")
        .fetch_one(&mut connection).await.unwrap();
    assert_eq!(page.get::<String, _>("markdown"), "Eigenvalues survive the upgrade");
    assert_eq!(page.get::<Vec<u8>, _>("embedding"), vec![0, 60, 0, 0]);
    assert_eq!(page.get::<String, _>("embed_model"), "legacy-embedding");
    assert_eq!(page.get::<i64, _>("embed_dim"), 2);
    let indexed: i64 = sqlx::query_scalar("SELECT rowid FROM pages_fts WHERE pages_fts MATCH 'eigenvalues'")
        .fetch_one(&mut connection).await.unwrap();
    assert_eq!(indexed, 12);

    let models: String = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'job_models'")
        .fetch_one(&mut connection).await.unwrap();
    let models: serde_json::Value = serde_json::from_str(&models).unwrap();
    assert_eq!(models["lectureReading"]["model"], "my-selected-model");
    assert_eq!(models["other"]["model"], "unchanged");
    assert!(models.get("lectureRecap").is_none());
    let lecture = sqlx::query("SELECT progress_seconds, last_watched_at, reading_status FROM lectures WHERE id='lecture-one'")
        .fetch_one(&mut connection).await.unwrap();
    assert_eq!(lecture.get::<i64, _>("progress_seconds"), 123);
    assert_eq!(lecture.get::<String, _>("last_watched_at"), "2026-09-18");
    assert_eq!(lecture.get::<Option<String>, _>("reading_status"), None);
    let question: String = sqlx::query_scalar("SELECT content FROM harness_items WHERE thread_id = 5")
        .fetch_one(&mut connection).await.unwrap();
    assert_eq!(question, "Keep my question");

    // New task and browser capabilities work after an existing library upgrades.
    let unfiled = connection.execute("INSERT INTO project_tasks (title,column_id,position) VALUES ('Unfiled','todo',1)")
        .await.unwrap().last_insert_rowid();
    assert!(unfiled > 22);
    connection.execute("INSERT INTO browser_history (url,host) VALUES ('https://example.test','example.test'); INSERT INTO browser_favicons (host,icon) VALUES ('example.test','data:image/png;base64,test');").await.unwrap();
    let fk_errors = sqlx::query("PRAGMA foreign_key_check").fetch_all(&mut connection).await.unwrap();
    assert!(fk_errors.is_empty());
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check").fetch_one(&mut connection).await.unwrap();
    assert_eq!(integrity, "ok");

    // The rebuilt self-reference still cascades, and the new FTS triggers track
    // edits/deletions without losing unrelated unfiled tasks.
    connection.execute("DELETE FROM project_tasks WHERE id=21; UPDATE pages SET markdown='Orthogonality' WHERE id=12;").await.unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks").fetch_one(&mut connection).await.unwrap();
    assert_eq!(remaining, 1);
    let indexed: i64 = sqlx::query_scalar("SELECT rowid FROM pages_fts WHERE pages_fts MATCH 'orthogonality'").fetch_one(&mut connection).await.unwrap();
    assert_eq!(indexed, 12);
    connection.execute("DELETE FROM files WHERE id=4").await.unwrap();
    let indexed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pages_fts WHERE pages_fts MATCH 'orthogonality'").fetch_one(&mut connection).await.unwrap();
    assert_eq!(indexed, 0);
}

#[tokio::test]
async fn legacy_malformed_optional_model_setting_does_not_block_upgrade() {
    let mut connection = legacy_library().await;
    connection.execute("UPDATE settings SET value = 'not json' WHERE key='job_models'").await.unwrap();
    for (version, sql) in migrations().into_iter().filter(|(v, _)| *v > 33) {
        apply(&mut connection, version, sql).await;
    }
    let value: String = sqlx::query_scalar("SELECT value FROM settings WHERE key='job_models'").fetch_one(&mut connection).await.unwrap();
    assert_eq!(value, "not json");
}
