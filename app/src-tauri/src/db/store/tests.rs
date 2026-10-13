use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

use super::*;

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    pool
}

#[tokio::test]
async fn setting_edits_keep_unknown_keys_and_transactions_can_roll_back() {
    let pool = migrated_pool().await;
    set_setting(
        &pool,
        "parse",
        r#"{"engineUrl":"http://old","unknown":{"enabled":true}}"#,
    )
    .await
    .unwrap();
    set_setting(&pool, "embed", r#"{"engine":"cloud"}"#)
        .await
        .unwrap();
    edit_setting(&pool, "parse", |object| {
        object.insert("engine".into(), "local".into());
        object.remove("engineUrl");
    })
    .await
    .unwrap();
    let raw = setting(&pool, "parse").await.unwrap().unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw).unwrap(),
        serde_json::json!({"engine":"local", "unknown":{"enabled":true}})
    );
    assert_eq!(
        setting(&pool, "embed").await.unwrap().as_deref(),
        Some(r#"{"engine":"cloud"}"#)
    );
    assert_eq!(setting(&pool, "missing").await.unwrap(), None);

    let mut tx = pool.begin().await.unwrap();
    set_setting(&mut *tx, "parse", "replacement").await.unwrap();
    assert_eq!(
        setting(&mut *tx, "parse").await.unwrap().as_deref(),
        Some("replacement")
    );
    tx.rollback().await.unwrap();
    assert_eq!(setting(&pool, "parse").await.unwrap(), Some(raw));

    for invalid in [
        None,
        Some("invalid json"),
        Some("{broken"),
        Some("null"),
        Some("[]"),
        Some("42"),
    ] {
        sqlx::query("DELETE FROM settings WHERE key = 'embed'")
            .execute(&pool)
            .await
            .unwrap();
        if let Some(raw) = invalid {
            set_setting(&pool, "embed", raw).await.unwrap();
        }
        edit_setting(&pool, "embed", |object| {
            object.insert("engine".into(), "cloud".into());
        })
        .await
        .unwrap();
        assert_eq!(
            setting(&pool, "embed").await.unwrap().as_deref(),
            Some(r#"{"engine":"cloud"}"#)
        );
    }
}

#[tokio::test]
async fn reconciling_parse_status_keeps_failures_and_skips_and_clears_stale_runs() {
    let pool = migrated_pool().await;
    let dir = crate::test_support::Scratch::new("reconcile-parse");
    sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
        .execute(&pool)
        .await
        .unwrap();
    let rows = [
        ("courses/SUBJ/files/failed.pdf", "error"),
        ("courses/SUBJ/files/skipped.pdf", "skipped"),
        ("courses/SUBJ/files/queued.pdf", "queued"),
        ("courses/SUBJ/files/running.pdf", "running"),
        ("courses/SUBJ/files/gone.pdf", "quality"),
        ("courses/SUBJ/files/done.pdf", "error"),
    ];
    for (rel, status) in rows {
        sqlx::query(
            "INSERT INTO files (subject_id, filename, relative_path, file_type, parse_status)
                 VALUES (1, ?1, ?2, 'pdf', ?3)",
        )
        .bind(rel.rsplit('/').next().unwrap())
        .bind(rel)
        .bind(status)
        .execute(&pool)
        .await
        .unwrap();
    }
    // Only `done.pdf` has a record on disk.
    let done = dir.join("courses/SUBJ/files/done.pdf");
    std::fs::create_dir_all(done.parent().unwrap()).unwrap();
    std::fs::write(crate::parse::pages_path(&done), r#"{"mode":"quality"}"#).unwrap();

    reconcile_parse_status(&pool, &dir).await.unwrap();

    let status = |rel: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT parse_status FROM files WHERE relative_path = ?1",
            )
            .bind(rel)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(
        status("courses/SUBJ/files/failed.pdf").await.as_deref(),
        Some("error")
    );
    assert_eq!(
        status("courses/SUBJ/files/skipped.pdf").await.as_deref(),
        Some("skipped")
    );
    assert_eq!(status("courses/SUBJ/files/queued.pdf").await, None);
    assert_eq!(status("courses/SUBJ/files/running.pdf").await, None);
    assert_eq!(status("courses/SUBJ/files/gone.pdf").await, None);
    assert_eq!(
        status("courses/SUBJ/files/done.pdf").await.as_deref(),
        Some("quality")
    );
}

#[tokio::test]
async fn file_upserts_preserve_content_time_unless_bytes_changed() {
    let pool = migrated_pool().await;
    sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
        .execute(&pool)
        .await
        .unwrap();
    let row = FileRow {
        subject_id: 1,
        relative_path: "courses/SUBJ/files/a.pdf",
        size_bytes: 4,
        category: "files",
        canvas_id: Some(7),
        source_url: None,
        changed: false,
    };
    upsert_file(&pool, &row).await.unwrap();
    let id = file_id(&pool, 1, "courses/SUBJ/files/a.pdf")
        .await
        .unwrap()
        .unwrap();
    let stamp: Option<String> =
        sqlx::query_scalar("SELECT content_changed_at FROM files WHERE id = ?1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        stamp, None,
        "an unchanged insert has no content-change stamp"
    );
    sqlx::query(
        "UPDATE files SET first_seen_at = 'first', content_changed_at = 'content' WHERE id = ?1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    for changed in [false, true] {
        let row = FileRow {
            size_bytes: 8,
            canvas_id: Some(9),
            source_url: Some("https://source"),
            changed,
            ..row
        };
        upsert_file(&pool, &row).await.unwrap();
        let row = sqlx::query("SELECT first_seen_at, content_changed_at, size_bytes, canvas_id, source_url FROM files WHERE id = ?1")
                .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(row.get::<String, _>("first_seen_at"), "first");
        assert_eq!(
            row.get::<String, _>("content_changed_at") == "content",
            !changed
        );
        assert_eq!(row.get::<i64, _>("size_bytes"), 8);
        assert_eq!(row.get::<i64, _>("canvas_id"), 9);
        assert_eq!(row.get::<String, _>("source_url"), "https://source");
    }
}

#[tokio::test]
async fn content_end_claims_saves_backfills_done_and_reconciles() {
    let pool = migrated_pool().await;
    sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
        .execute(&pool)
        .await
        .unwrap();
    for (id, progress) in [("watched", 1910), ("early", 1000)] {
        sqlx::query(
                "INSERT INTO lectures (id, lesson_id, subject_id, title, date, duration_seconds, progress_seconds)
                 VALUES (?1, ?1, 1, ?1, '2026-01-01', 2400, ?2)",
            )
            .bind(id).bind(progress).execute(&pool).await.unwrap();
    }
    let row = |id: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query(
                    "SELECT content_end_seconds, content_end_quote, content_end_status, content_end_error, completed
                       FROM lectures WHERE id = ?1",
                )
                .bind(id).fetch_one(&pool).await.unwrap()
        }
    };

    let claim = |id: &'static str, force: bool| {
        let pool = pool.clone();
        async move { claim_content_end(&pool, id, force).await.unwrap() }
    };
    assert_eq!(claim("watched", false).await, EndClaim::Claimed);
    assert_eq!(
        claim("watched", true).await,
        EndClaim::Running,
        "force never takes a running claim"
    );
    assert!(
        save_content_end(&pool, "watched", Some((1915, "see you tomorrow")))
            .await
            .unwrap(),
        "1910 s watched is within 10 s of a 1915 s end"
    );
    let saved = row("watched").await;
    assert_eq!(saved.get::<i64, _>("content_end_seconds"), 1915);
    assert_eq!(
        saved.get::<String, _>("content_end_quote"),
        "see you tomorrow"
    );
    assert_eq!(saved.get::<String, _>("content_end_status"), "ready");
    assert_eq!(saved.get::<i64, _>("completed"), 1);
    assert_eq!(claim("watched", false).await, EndClaim::Found);
    assert_eq!(claim("watched", true).await, EndClaim::Claimed);
    assert_eq!(reconcile_content_end_status(&pool).await.unwrap(), 1);
    let swept = row("watched").await;
    assert_eq!(swept.get::<Option<String>, _>("content_end_status"), None);
    assert_eq!(
        swept.get::<i64, _>("content_end_seconds"),
        1915,
        "the found end survives a sweep"
    );

    assert_eq!(claim("early", false).await, EndClaim::Claimed);
    assert!(!save_content_end(&pool, "early", Some((1915, "that's it")))
        .await
        .unwrap());
    assert_eq!(row("early").await.get::<i64, _>("completed"), 0);
    assert_eq!(claim("early", true).await, EndClaim::Claimed);
    set_content_end_error(&pool, "early", "bad reply")
        .await
        .unwrap();
    let failed = row("early").await;
    assert_eq!(failed.get::<String, _>("content_end_status"), "error");
    assert_eq!(failed.get::<String, _>("content_end_error"), "bad reply");
    assert_eq!(
        failed.get::<i64, _>("content_end_seconds"),
        1915,
        "a failed re-run keeps the end"
    );
    assert_eq!(
        claim("early", false).await,
        EndClaim::Claimed,
        "an error re-runs without force"
    );
    assert!(!save_content_end(&pool, "early", None).await.unwrap());
    let none = row("early").await;
    assert_eq!(none.get::<String, _>("content_end_status"), "none");
    assert_eq!(none.get::<Option<i64>, _>("content_end_seconds"), None);
    assert_eq!(claim("missing", false).await, EndClaim::NoLecture);
}

async fn pages_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    // The shape migration 12 created, embedding columns included.
    sqlx::query(
        "CREATE TABLE pages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id     INTEGER NOT NULL,
                page_no     INTEGER NOT NULL,
                markdown    TEXT    NOT NULL DEFAULT '',
                embedding   BLOB,
                embed_model TEXT,
                embed_dim   INTEGER,
                embedded_at TEXT,
                UNIQUE(file_id, page_no)
             )",
    )
    .execute(&pool)
    .await
    .expect("pages schema");
    pool
}

fn page(page_no: u32, markdown: &str) -> crate::parse::ParsePage {
    crate::parse::ParsePage {
        page_no,
        markdown: markdown.to_string(),
        blocks: Vec::new(),
    }
}

#[tokio::test]
async fn a_reparse_never_blanks_markdown_it_already_had() {
    let pool = pages_pool().await;

    let with_text = upsert_pages(&pool, 7, &[page(1, "one"), page(2, "two"), page(3, "")])
        .await
        .expect("first parse");
    assert_eq!(with_text, 2);

    // Pretend the embedder has been over it; re-parsing must not disturb that.
    sqlx::query("UPDATE pages SET embedding = X'00', embed_model = 'qwen' WHERE page_no = 1")
        .execute(&pool)
        .await
        .expect("fake embedding");

    // A thinner second parse: the empty page 2 must leave the good text standing.
    upsert_pages(&pool, 7, &[page(1, "one, better"), page(2, "")])
        .await
        .expect("second parse");

    let rows = sqlx::query("SELECT page_no, markdown, embed_model FROM pages ORDER BY page_no")
        .fetch_all(&pool)
        .await
        .expect("read back");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].get::<String, _>("markdown"), "one, better");
    assert_eq!(rows[1].get::<String, _>("markdown"), "two");
    assert_eq!(rows[2].get::<String, _>("markdown"), "");
    assert_eq!(
        rows[0].get::<Option<String>, _>("embed_model").as_deref(),
        Some("qwen")
    );
}

#[tokio::test]
async fn reconcile_follows_the_disk_in_both_directions() {
    let data_dir = crate::test_support::Scratch::new("reconcile");
    let course = data_dir.join("courses/SUBJ/files");
    std::fs::create_dir_all(&course).expect("scratch library");

    let parsed = course.join("done.pdf");
    std::fs::write(&parsed, b"%PDF").unwrap();
    std::fs::write(
        crate::parse::pages_path(&parsed),
        r#"{"mode":"quality","parser_version":2,"page_count":1,"pages":[]}"#,
    )
    .unwrap();
    std::fs::write(course.join("gone.pdf"), b"%PDF").unwrap();

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    sqlx::query(
        "CREATE TABLE files (
                relative_path TEXT PRIMARY KEY,
                parse_status  TEXT,
                parsed_at     TEXT
             )",
    )
    .execute(&pool)
    .await
    .expect("files schema");
    sqlx::query(
        "INSERT INTO files (relative_path, parse_status) VALUES
               ('courses/SUBJ/files/done.pdf', NULL),
               -- Left behind by a run that was killed mid-parse: only a live
               -- process could ever have cleared this.
               ('courses/SUBJ/files/gone.pdf', 'running')",
    )
    .execute(&pool)
    .await
    .expect("rows");

    let updated = reconcile_parse_status(&pool, &data_dir)
        .await
        .expect("reconcile");
    assert_eq!(updated, 2);

    let status = |rel: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT parse_status FROM files WHERE relative_path = ?1",
            )
            .bind(rel)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(
        status("courses/SUBJ/files/done.pdf").await.as_deref(),
        Some("quality")
    );
    assert_eq!(status("courses/SUBJ/files/gone.pdf").await, None);
}

#[tokio::test]
async fn an_echo_embed_refresh_keeps_its_file_and_poster_and_cascades_with_the_subject() {
    let pool = migrated_pool().await;
    sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
        .execute(&pool)
        .await
        .unwrap();
    let mut embed = EchoEmbed {
        link_id: "link-a".into(),
        subject_id: 1,
        media_id: "media-1".into(),
        secure_link_id: "sl-1".into(),
        name: "Week 1.mp4".into(),
        duration_s: Some(1358.393),
        restrict_download: false,
        has_captions: true,
        poster_rel: Some("courses/SUBJ/images/echo-media-1.jpg".into()),
        file_rel: None,
        extra_sources: 0,
    };
    upsert_echo_embed(&pool, &embed).await.unwrap();
    assert_eq!(
        echo_embed(&pool, "link-a").await.unwrap(),
        Some(embed.clone())
    );

    set_echo_embed_file(&pool, "link-a", Some("courses/SUBJ/files/Week_1.mp4"))
        .await
        .unwrap();
    embed.name = "Week 1 (renamed).mp4".into();
    embed.poster_rel = None;
    upsert_echo_embed(&pool, &embed).await.unwrap();
    let stored = echo_embed(&pool, "link-a").await.unwrap().unwrap();
    assert_eq!(stored.name, "Week 1 (renamed).mp4");
    assert_eq!(
        stored.file_rel.as_deref(),
        Some("courses/SUBJ/files/Week_1.mp4")
    );
    assert_eq!(
        stored.poster_rel.as_deref(),
        Some("courses/SUBJ/images/echo-media-1.jpg")
    );
    assert_eq!(
        echo_embed_file_of_media(&pool, 1, "media-1").await.unwrap(),
        vec!["courses/SUBJ/files/Week_1.mp4".to_string()]
    );
    assert!(echo_embed_file_of_media(&pool, 1, "media-2")
        .await
        .unwrap()
        .is_empty());

    sqlx::query("DELETE FROM subjects WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(echo_embed(&pool, "link-a").await.unwrap(), None);
}

#[tokio::test]
async fn a_studio_video_links_to_its_thread_and_survives_the_thread() {
    let pool = migrated_pool().await;
    let thread = |pool: SqlitePool| async move {
        sqlx::query("INSERT INTO harness_threads (provider, status) VALUES ('claude', 'idle')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid()
    };
    let (first, second) = (thread(pool.clone()).await, thread(pool.clone()).await);

    assert_eq!(studio_link_of_thread(&pool, first).await.unwrap(), None);
    link_studio_video(&pool, "gradient-descent", first, Some(60))
        .await
        .unwrap();
    let link = studio_link_of_thread(&pool, first).await.unwrap().unwrap();
    assert_eq!(link.slug, "gradient-descent");
    assert_eq!(link.length_secs, Some(60));

    // A deleted video's row is replaced when its slug is made again.
    link_studio_video(&pool, "gradient-descent", second, None)
        .await
        .unwrap();
    assert_eq!(studio_link_of_thread(&pool, first).await.unwrap(), None);
    let links = studio_links(&pool).await.unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links["gradient-descent"].thread_id, Some(second));
    assert_eq!(links["gradient-descent"].length_secs, None);

    // Deleting the thread keeps the video, unlinked.
    sqlx::query("DELETE FROM harness_threads WHERE id = ?1")
        .bind(second)
        .execute(&pool)
        .await
        .unwrap();
    let links = studio_links(&pool).await.unwrap();
    assert_eq!(links["gradient-descent"].thread_id, None);
}

#[tokio::test]
async fn term_sort_backfill_reads_names_and_keeps_stamped_keys() {
    let pool = migrated_pool().await;
    sqlx::raw_sql(
        "INSERT INTO subjects (id, code, name, term_name, term_sort) VALUES
           (1, 'A', 'Alpha', 'UGRD Semester 2 2026 (2650)', NULL),
           (2, 'B', 'Beta', 'UGRD Semester 1 2026 (2610)', NULL),
           (3, 'C', 'Gamma', 'STEM Program', NULL),
           (4, 'D', 'Delta', 'UGRD Semester 2 2026 (2650)', '2026-08'),
           (5, 'E', 'Empty', NULL, NULL);",
    )
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(backfill_term_sort(&pool, true).await.unwrap(), 2);
    let sorts: Vec<Option<String>> =
        sqlx::query_scalar("SELECT term_sort FROM subjects ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        sorts,
        [
            Some("2026-07"),
            Some("2026-03"),
            None,
            Some("2026-08"),
            None
        ]
        .map(|s| s.map(str::to_string))
    );
    assert_eq!(backfill_term_sort(&pool, true).await.unwrap(), 0);
}
