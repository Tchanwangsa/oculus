//! The database access every module shares. In the app the frontend writes most
//! rows through tauri-plugin-sql; headless runs write the same rows with the
//! same SQL here. Schema ownership stays with the migrations: a missing
//! database is reported, never created.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use crate::sync::Course;

// ── The connection pool ──────────────────────────────────────────────────────

/// Our own pool over the plugin's file. WAL makes a second reader harmless and
/// our writes are rare, so a busy timeout keeps out of the plugin's way.
pub async fn pool(path: &Path) -> Result<SqlitePool, String> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(15));
    SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(opts)
        .await
        .map_err(|e| format!("open {}: {e}", path.display()))
}

/// The shared database file.
pub fn db_path() -> PathBuf {
    crate::paths::db_path(&crate::paths::data_dir())
}

/// Open the shared database, reporting (never creating) a missing one.
pub async fn open_pool() -> Result<SqlitePool, String> {
    let path = db_path();
    if !path.exists() {
        return Err(format!("no database at {}", path.display()));
    }
    pool(&path).await
}

/// Read a settings row through the caller's pool, connection or transaction.
pub async fn setting<'e, E>(executor: E, key: &str) -> Result<Option<String>, String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query_scalar("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(executor)
        .await
        .map_err(|e| e.to_string())
}

/// Upsert one settings row; callers own its format and validation.
pub async fn set_setting<'e, E>(executor: E, key: &str, value: &str) -> Result<(), String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(executor)
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Edit an object settings row, retaining unknown keys. Missing or malformed
/// objects use the same empty default as the parse and embed config readers.
pub async fn edit_setting(
    pool: &SqlitePool,
    key: &str,
    edit: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) -> Result<(), String> {
    let stored = setting(pool, key).await?;
    let mut object = stored
        .as_deref()
        .and_then(|raw| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(raw).ok())
        .unwrap_or_default();
    edit(&mut object);
    let value = serde_json::to_string(&object).map_err(|e| e.to_string())?;
    set_setting(pool, key, &value).await
}

/// Read one `settings` row from a **synchronous** caller, from any context.
///
/// The seams read their backend setting from plain threads and non-async
/// clients. Never use `tauri::async_runtime::block_on` here: it panics on a
/// runtime worker thread, which is where an `async` Tauri command runs. So the
/// read always happens on a thread of its own with a current-thread runtime —
/// one code path, whoever calls. `None` covers no database, no row, or a
/// failed read; every caller defaults the same way.
pub fn setting_blocking(key: &str) -> Option<String> {
    let database = db_path();
    if !database.is_file() {
        return None;
    }
    let key = key.to_string();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().ok()?;
        runtime.block_on(async move {
            use sqlx::Connection;
            let options = SqliteConnectOptions::new()
                .filename(&database)
                .create_if_missing(false)
                .busy_timeout(Duration::from_secs(15));
            let mut connection = sqlx::SqliteConnection::connect_with(&options).await.ok()?;
            let value = setting(&mut connection, &key).await.ok().flatten();
            connection.close().await.ok();
            value
        })
    })
    .join()
    .ok()
    .flatten()
}

pub async fn open(data_dir: &Path) -> Result<SqlitePool, String> {
    let path = crate::paths::db_path(data_dir);
    if !path.exists() {
        return Err(format!(
            "no database at {} — open the Oculus app once to create it",
            path.display()
        ));
    }
    pool(&path).await
}

// ── Subjects ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SubjectRow {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub term_name: Option<String>,
    pub is_current: bool,
    pub selected: bool,
    pub last_synced_at: Option<String>,
}

pub async fn upsert_subjects(pool: &SqlitePool, courses: &[Course]) -> Result<(), String> {
    for c in courses {
        sqlx::query(
            r#"INSERT INTO subjects (id, code, name, term_name, is_current, workflow_state, selected)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
               ON CONFLICT(id) DO UPDATE SET
                 name           = excluded.name,
                 term_name      = excluded.term_name,
                 is_current     = excluded.is_current,
                 workflow_state = excluded.workflow_state"#,
        )
        .bind(c.id)
        .bind(&c.code)
        .bind(&c.name)
        .bind(&c.term)
        .bind(i32::from(c.is_current))
        .bind(&c.workflow_state)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub async fn subjects(pool: &SqlitePool) -> Result<Vec<SubjectRow>, String> {
    // Derived, not stored: the latest completed run naming the subject. Mirrors
    // `getSubjects` in app/src/lib/db.ts — keep the two in step.
    let rows = sqlx::query(
        "SELECT s.id, s.code, s.name, s.term_name, s.is_current, s.selected,
                (SELECT MAX(r.finished_at)
                 FROM sync_runs r, json_each(r.subject_codes) j
                 WHERE r.status = 'completed' AND j.value = s.code) AS last_synced_at
         FROM subjects s ORDER BY s.is_current DESC, s.term_name DESC, s.name ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|r| SubjectRow {
            id: r.get("id"),
            code: r.get("code"),
            name: r.get("name"),
            term_name: r.get("term_name"),
            is_current: r.get::<i64, _>("is_current") != 0,
            selected: r.get::<i64, _>("selected") != 0,
            last_synced_at: r.get("last_synced_at"),
        })
        .collect())
}

// ── Files ────────────────────────────────────────────────────────────────────

pub async fn upsert_file(
    pool: &SqlitePool,
    subject_id: i64,
    relative_path: &str,
    size_bytes: u64,
    category: &str,
    canvas_id: Option<i64>,
    source_url: Option<&str>,
    changed: bool,
) -> Result<(), String> {
    let filename = relative_path.rsplit('/').next().unwrap_or(relative_path).to_string();
    let file_type = filename.rsplit_once('.').map(|(_, e)| e.to_string()).unwrap_or_else(|| "md".into());

    // `changed` is the engine's action; content_changed_at moves only on new bytes.
    sqlx::query(
        r#"INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category, canvas_id, source_url, first_seen_at, content_changed_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'),
                   CASE WHEN ?9 THEN datetime('now') ELSE NULL END)
           ON CONFLICT(subject_id, relative_path) DO UPDATE SET
             filename   = excluded.filename,
             file_type  = excluded.file_type,
             size_bytes = excluded.size_bytes,
             category   = excluded.category,
             canvas_id  = excluded.canvas_id,
             source_url = excluded.source_url,
             scraped_at = datetime('now'),
             content_changed_at = CASE WHEN ?9 THEN excluded.content_changed_at ELSE files.content_changed_at END"#,
    )
    .bind(subject_id)
    .bind(&filename)
    .bind(relative_path)
    .bind(&file_type)
    .bind(size_bytes as i64)
    .bind(category)
    .bind(canvas_id)
    .bind(source_url)
    .bind(changed)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// `files.id` for one artifact, needed to hang embedded pages off it.
pub async fn file_id(
    pool: &SqlitePool,
    subject_id: i64,
    relative_path: &str,
) -> Result<Option<i64>, String> {
    sqlx::query_scalar("SELECT id FROM files WHERE subject_id = ?1 AND relative_path = ?2")
        .bind(subject_id)
        .bind(relative_path)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Write one file's page records.
///
/// The conflict clause is a CASE, not a `COALESCE`: an empty incoming page (a
/// full-bleed image, or a thinner re-parse) must leave good text alone. The
/// embedding columns are the embedder's and are never touched here.
pub async fn upsert_pages(
    pool: &SqlitePool,
    file_id: i64,
    pages: &[crate::parse::ParsePage],
) -> Result<usize, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let with_text = write_pages(&mut tx, file_id, pages).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(with_text)
}

/// Replace one file's page records outright, vectors included: for text that
/// is never embedded (`crate::sheets`), where a shorter workbook must lose
/// its old sheets.
pub async fn replace_pages(
    pool: &SqlitePool,
    file_id: i64,
    pages: &[crate::parse::ParsePage],
) -> Result<usize, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM pages WHERE file_id = ?1")
        .bind(file_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let with_text = write_pages(&mut tx, file_id, pages).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(with_text)
}

async fn write_pages(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    file_id: i64,
    pages: &[crate::parse::ParsePage],
) -> Result<usize, String> {
    let mut with_text = 0usize;
    for page in pages {
        if !page.markdown.is_empty() {
            with_text += 1;
        }
        sqlx::query(
            r#"INSERT INTO pages (file_id, page_no, markdown)
               VALUES (?1, ?2, ?3)
               ON CONFLICT(file_id, page_no) DO UPDATE SET
                 markdown = CASE WHEN excluded.markdown != '' THEN excluded.markdown ELSE pages.markdown END"#,
        )
        .bind(file_id)
        .bind(i64::from(page.page_no))
        .bind(&page.markdown)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("upsert page {}: {e}", page.page_no))?;
    }
    Ok(with_text)
}

/// How many page rows this file already has.
pub async fn page_count(pool: &SqlitePool, file_id: i64) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE file_id = ?1")
        .bind(file_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Every PDF-backed file on record (PDFs and Office documents with a derived
/// sibling PDF), optionally narrowed to a set of subjects. A file the user
/// skipped stays out until they parse it.
pub async fn pdf_files(
    pool: &SqlitePool,
    subject_ids: &[i64],
) -> Result<Vec<(i64, String)>, String> {
    let rows = sqlx::query(
        "SELECT subject_id, relative_path FROM files
         WHERE parse_status IS NULL OR parse_status != 'skipped'
         ORDER BY relative_path",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|r| (r.get::<i64, _>("subject_id"), r.get::<String, _>("relative_path")))
        .filter(|(sid, rel)| {
            crate::paths::doc_pdf_rel(rel).is_some()
                && (subject_ids.is_empty() || subject_ids.contains(sid))
        })
        .collect())
}

/// Derive parse status from what the parser left on disk: a record is
/// `quality`. Without one, the transient `queued`/`running` a killed run
/// leaves behind (and a `quality` whose record is gone) are cleared, but
/// `error` stays — it is the only trace of a failure from an earlier run —
/// and so does `skipped`, the only memory of a skip across restarts.
pub async fn reconcile_parse_status(pool: &SqlitePool, data_dir: &Path) -> Result<u64, String> {
    let rows = sqlx::query("SELECT relative_path FROM files")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut updated = 0;
    for r in &rows {
        let rel: String = r.get("relative_path");
        let Some(pdf_rel) = crate::paths::doc_pdf_rel(&rel) else {
            continue;
        };

        let res = match crate::parse::parse_mode(&data_dir.join(&pdf_rel)) {
            Some(status) => {
                sqlx::query(
                    "UPDATE files SET parse_status = ?1, parsed_at = datetime('now')
                     WHERE relative_path = ?2 AND (parse_status IS NULL OR parse_status != ?1)",
                )
                .bind(status)
                .bind(&rel)
                .execute(pool)
                .await
            }
            None => {
                sqlx::query(
                    "UPDATE files SET parse_status = NULL, parsed_at = NULL
                     WHERE relative_path = ?1 AND parse_status IS NOT NULL
                       AND parse_status NOT IN ('error', 'skipped')",
                )
                .bind(&rel)
                .execute(pool)
                .await
            }
        }
        .map_err(|e| e.to_string())?;
        updated += res.rows_affected();
    }
    Ok(updated)
}

/// Chaptering runs killed mid-job (`running`, no `chaptered_at`), cleared at
/// startup: only a live process could clear the status otherwise.
pub async fn reconcile_chapter_status(pool: &SqlitePool) -> Result<u64, String> {
    sqlx::query(
        "UPDATE lectures SET chapter_status = NULL, chapter_error = NULL
          WHERE chapter_status = 'running'",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .map_err(|e| e.to_string())
}

/// Lecture-end runs killed mid-job. The found end, if any, stays.
pub async fn reconcile_content_end_status(pool: &SqlitePool) -> Result<u64, String> {
    sqlx::query(
        "UPDATE lectures SET content_end_status = NULL, content_end_error = NULL
          WHERE content_end_status = 'running'",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .map_err(|e| e.to_string())
}

// ── Calendar ─────────────────────────────────────────────────────────────────

/// Replace a subject's calendar rows with what Canvas just returned.
///
/// Delete-then-insert, not upsert, so a moved or cancelled class disappears.
/// The fetch is always the course's complete set.
pub async fn replace_calendar_events(
    pool: &SqlitePool,
    subject_id: i64,
    events: &[crate::calendar::CalendarEvent],
) -> Result<(), String> {
    sqlx::query("DELETE FROM calendar_events WHERE subject_id = ?1")
        .bind(subject_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    for e in events {
        sqlx::query(
            r#"INSERT INTO calendar_events
                 (id, subject_id, kind, title, start_at, end_at, all_day,
                  location, url, description, synced_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
               ON CONFLICT(id) DO UPDATE SET
                 subject_id  = excluded.subject_id,
                 kind        = excluded.kind,
                 title       = excluded.title,
                 start_at    = excluded.start_at,
                 end_at      = excluded.end_at,
                 all_day     = excluded.all_day,
                 location    = excluded.location,
                 url         = excluded.url,
                 description = excluded.description,
                 synced_at   = datetime('now')"#,
        )
        .bind(&e.id)
        .bind(subject_id)
        .bind(&e.kind)
        .bind(&e.title)
        .bind(&e.start_at)
        .bind(&e.end_at)
        .bind(e.all_day as i64)
        .bind(&e.location)
        .bind(&e.url)
        .bind(&e.description)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── Lectures ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct LectureRow {
    /// Echo360's media id and the folder under `lectures/`; the CLI's handle.
    pub id: String,
    pub title: String,
    pub date: String,
    pub duration_seconds: i64,
    pub has_video: bool,
    pub has_transcript: bool,
}

pub async fn upsert_lectures(
    pool: &SqlitePool,
    subject_id: i64,
    lectures: &[crate::echo360::Lecture],
) -> Result<(), String> {
    for l in lectures {
        sqlx::query(
            r#"INSERT INTO lectures
                 (id, lesson_id, subject_id, title, date, duration_seconds, has_source2, synced_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
               ON CONFLICT(id) DO UPDATE SET
                 title            = excluded.title,
                 date             = excluded.date,
                 duration_seconds = excluded.duration_seconds,
                 has_source2      = excluded.has_source2,
                 synced_at        = datetime('now')"#,
        )
        .bind(&l.id)
        .bind(&l.lesson_id)
        .bind(subject_id)
        .bind(&l.title)
        .bind(&l.date)
        .bind(l.duration_seconds)
        .bind(l.has_second_source as i64)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Record where a downloaded artifact landed.
pub async fn set_lecture_path(
    pool: &SqlitePool,
    id: &str,
    column: &str,
    path: &str,
) -> Result<(), String> {
    // `column` is never user input — it is one of two literals below.
    let sql = match column {
        "video_path" => "UPDATE lectures SET video_path = ?1 WHERE id = ?2",
        "video2_path" => "UPDATE lectures SET video2_path = ?1 WHERE id = ?2",
        "transcript_path" => "UPDATE lectures SET transcript_path = ?1 WHERE id = ?2",
        other => return Err(format!("unknown lecture column {other}")),
    };
    sqlx::query(sql)
        .bind(path)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The chaptering job's own three columns on `lectures`, written together.
///
/// Separate from [`set_lecture_path`] because these must clear to NULL.
/// `status` NULL means never chaptered; only a terminal status stamps
/// `chaptered_at`; `error` is cleared by any write that does not carry one.
pub async fn set_chapter_status(
    pool: &SqlitePool,
    lecture_id: &str,
    status: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let terminal = matches!(status, Some("ready") | Some("error"));
    sqlx::query(
        "UPDATE lectures
            SET chapter_status = ?1,
                chapter_error  = ?2,
                chaptered_at   = CASE WHEN ?3 THEN datetime('now') ELSE NULL END
          WHERE id = ?4",
    )
    .bind(status)
    .bind(error)
    .bind(i64::from(terminal))
    .bind(lecture_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Replace a lecture's chapters, and mark it chaptered.
///
/// One transaction, over a set `chapters::validate` already passed: a missing
/// chapter silently stretches the one before it, and a failed regenerate must
/// leave the old set standing.
pub async fn save_chapters(
    pool: &SqlitePool,
    lecture_id: &str,
    chapters: &[crate::chapters::Chapter],
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM lecture_chapters WHERE lecture_id = ?1")
        .bind(lecture_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for (idx, chapter) in chapters.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lecture_chapters (lecture_id, idx, start_seconds, title, summary)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(lecture_id)
        .bind(idx as i64)
        .bind(i64::from(chapter.start_seconds))
        .bind(&chapter.title)
        .bind(&chapter.summary)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    sqlx::query(
        "UPDATE lectures
            SET chapter_status = 'ready', chapter_error = NULL, chaptered_at = datetime('now')
          WHERE id = ?1",
    )
    .bind(lecture_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// A lecture's chapters in play order. Empty when it has never been chaptered.
pub async fn chapters(
    pool: &SqlitePool,
    lecture_id: &str,
) -> Result<Vec<crate::chapters::Chapter>, String> {
    let rows = sqlx::query(
        "SELECT start_seconds, title, summary FROM lecture_chapters
          WHERE lecture_id = ?1 ORDER BY idx",
    )
    .bind(lecture_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| crate::chapters::Chapter {
            start_seconds: r.get::<i64, _>("start_seconds").max(0) as u32,
            title: r.get("title"),
            summary: r.get("summary"),
        })
        .collect())
}

// ── Where a lecture's content ends (`lecture_end`) ───────────────────────────

/// What [`claim_content_end`] got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndClaim {
    Claimed,
    /// Another run holds it.
    Running,
    /// A `ready` or `none` result stands and `force` was not given.
    Found,
    NoLecture,
}

/// Atomically claim a lecture-end run: refused while one is `running`, and
/// over a `ready` or `none` result unless `force`; an `error` re-runs freely.
pub async fn claim_content_end(
    pool: &SqlitePool,
    lecture_id: &str,
    force: bool,
) -> Result<EndClaim, String> {
    let claimed = sqlx::query(
        "UPDATE lectures
            SET content_end_status = 'running', content_end_error = NULL
          WHERE id = ?1
            AND (content_end_status IS NULL
                 OR content_end_status = 'error'
                 OR (?2 AND content_end_status <> 'running'))",
    )
    .bind(lecture_id)
    .bind(force)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected()
        == 1;
    if claimed {
        return Ok(EndClaim::Claimed);
    }
    let status: Option<Option<String>> =
        sqlx::query_scalar("SELECT content_end_status FROM lectures WHERE id = ?1")
            .bind(lecture_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    Ok(match status {
        None => EndClaim::NoLecture,
        Some(Some(s)) if s == "running" => EndClaim::Running,
        Some(_) => EndClaim::Found,
    })
}

/// Write a found end (`None`: the recording has none, it was cut off) and, in
/// the same transaction, mark the lecture Done if it was already watched to
/// within 10 s of that end. Answers whether it did.
pub async fn save_content_end(
    pool: &SqlitePool,
    lecture_id: &str,
    end: Option<(u32, &str)>,
) -> Result<bool, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE lectures
            SET content_end_seconds = ?1,
                content_end_quote   = ?2,
                content_end_status  = ?3,
                content_end_error   = NULL
          WHERE id = ?4",
    )
    .bind(end.map(|(seconds, _)| i64::from(seconds)))
    .bind(end.map(|(_, quote)| quote))
    .bind(if end.is_some() { "ready" } else { "none" })
    .bind(lecture_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let mut completed = false;
    if let Some((seconds, _)) = end {
        completed = sqlx::query(
            "UPDATE lectures SET completed = 1
              WHERE id = ?1 AND completed = 0 AND progress_seconds >= ?2 - 10",
        )
        .bind(lecture_id)
        .bind(i64::from(seconds))
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected()
            == 1;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(completed)
}

/// A failed run. A previous end stays, as a failed chapter regenerate keeps
/// the old set.
pub async fn set_content_end_error(
    pool: &SqlitePool,
    lecture_id: &str,
    error: &str,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE lectures SET content_end_status = 'error', content_end_error = ?1 WHERE id = ?2",
    )
    .bind(error)
    .bind(lecture_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn lectures(pool: &SqlitePool, subject_id: i64) -> Result<Vec<LectureRow>, String> {
    let rows = sqlx::query(
        "SELECT id, title, date, duration_seconds, video_path, transcript_path
         FROM lectures WHERE subject_id = ?1 ORDER BY date ASC",
    )
    .bind(subject_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .map(|r| LectureRow {
            id: r.get("id"),
            title: r.get("title"),
            date: r.get("date"),
            duration_seconds: r.get("duration_seconds"),
            has_video: r.get::<Option<String>, _>("video_path").is_some(),
            has_transcript: r.get::<Option<String>, _>("transcript_path").is_some(),
        })
        .collect())
}

// ── Run bookkeeping ──────────────────────────────────────────────────────────

pub async fn start_run(pool: &SqlitePool, subject_codes: &[String]) -> Result<i64, String> {
    let codes_json =
        serde_json::to_string(subject_codes).unwrap_or_else(|_| "[]".to_string());
    sqlx::query("INSERT INTO sync_runs (status, subject_codes) VALUES ('running', ?1)")
        .bind(codes_json)
        .execute(pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

pub async fn finish_run(
    pool: &SqlitePool,
    id: i64,
    status: &str,
    subjects_synced: usize,
    error: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE sync_runs SET finished_at = datetime('now'), status = ?1,
             subjects_synced = ?2, pages_scraped = ?2, error = ?3
         WHERE id = ?4",
    )
    .bind(status)
    .bind(subjects_synced as i64)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn add_log(pool: &SqlitePool, level: &str, message: &str, run_id: Option<i64>) -> Result<(), String> {
    sqlx::query("INSERT INTO sync_log (run_id, level, message) VALUES (?1, ?2, ?3)")
        .bind(run_id)
        .bind(level)
        .bind(message)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn migrated_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new().max_connections(1)
            .connect("sqlite::memory:").await.unwrap();
        for migration in crate::migrations::all() {
            sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn setting_edits_keep_unknown_keys_and_transactions_can_roll_back() {
        let pool = migrated_pool().await;
        set_setting(&pool, "parse", r#"{"engineUrl":"http://old","unknown":{"enabled":true}}"#).await.unwrap();
        set_setting(&pool, "embed", r#"{"engine":"cloud"}"#).await.unwrap();
        edit_setting(&pool, "parse", |object| {
            object.insert("engine".into(), "local".into());
            object.remove("engineUrl");
        }).await.unwrap();
        let raw = setting(&pool, "parse").await.unwrap().unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(&raw).unwrap(),
            serde_json::json!({"engine":"local", "unknown":{"enabled":true}}));
        assert_eq!(setting(&pool, "embed").await.unwrap().as_deref(), Some(r#"{"engine":"cloud"}"#));
        assert_eq!(setting(&pool, "missing").await.unwrap(), None);

        let mut tx = pool.begin().await.unwrap();
        set_setting(&mut *tx, "parse", "replacement").await.unwrap();
        assert_eq!(setting(&mut *tx, "parse").await.unwrap().as_deref(), Some("replacement"));
        tx.rollback().await.unwrap();
        assert_eq!(setting(&pool, "parse").await.unwrap(), Some(raw));

        for invalid in [None, Some("invalid json"), Some("{broken"), Some("null"), Some("[]"), Some("42")] {
            sqlx::query("DELETE FROM settings WHERE key = 'embed'").execute(&pool).await.unwrap();
            if let Some(raw) = invalid {
                set_setting(&pool, "embed", raw).await.unwrap();
            }
            edit_setting(&pool, "embed", |object| {
                object.insert("engine".into(), "cloud".into());
            }).await.unwrap();
            assert_eq!(setting(&pool, "embed").await.unwrap().as_deref(),
                Some(r#"{"engine":"cloud"}"#));
        }
    }

    #[tokio::test]
    async fn reconciling_parse_status_keeps_failures_and_skips_and_clears_stale_runs() {
        let pool = migrated_pool().await;
        let dir = crate::test_support::Scratch::new("reconcile-parse");
        sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
            .execute(&pool).await.unwrap();
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
            .execute(&pool).await.unwrap();
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
                .fetch_one(&pool).await.unwrap()
            }
        };
        assert_eq!(status("courses/SUBJ/files/failed.pdf").await.as_deref(), Some("error"));
        assert_eq!(status("courses/SUBJ/files/skipped.pdf").await.as_deref(), Some("skipped"));
        assert_eq!(status("courses/SUBJ/files/queued.pdf").await, None);
        assert_eq!(status("courses/SUBJ/files/running.pdf").await, None);
        assert_eq!(status("courses/SUBJ/files/gone.pdf").await, None);
        assert_eq!(status("courses/SUBJ/files/done.pdf").await.as_deref(), Some("quality"));
    }

    #[tokio::test]
    async fn file_upserts_preserve_content_time_unless_bytes_changed() {
        let pool = migrated_pool().await;
        sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
            .execute(&pool).await.unwrap();
        upsert_file(&pool, 1, "courses/SUBJ/files/a.pdf", 4, "files", Some(7), None, false)
            .await.unwrap();
        let id = file_id(&pool, 1, "courses/SUBJ/files/a.pdf").await.unwrap().unwrap();
        let stamp: Option<String> = sqlx::query_scalar("SELECT content_changed_at FROM files WHERE id = ?1")
            .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(stamp, None, "an unchanged insert has no content-change stamp");
        sqlx::query("UPDATE files SET first_seen_at = 'first', content_changed_at = 'content' WHERE id = ?1")
            .bind(id).execute(&pool).await.unwrap();
        for changed in [false, true] {
            upsert_file(&pool, 1, "courses/SUBJ/files/a.pdf", 8, "files", Some(9), Some("https://source"), changed)
                .await.unwrap();
            let row = sqlx::query("SELECT first_seen_at, content_changed_at, size_bytes, canvas_id, source_url FROM files WHERE id = ?1")
                .bind(id).fetch_one(&pool).await.unwrap();
            assert_eq!(row.get::<String, _>("first_seen_at"), "first");
            assert_eq!(row.get::<String, _>("content_changed_at") == "content", !changed);
            assert_eq!(row.get::<i64, _>("size_bytes"), 8);
            assert_eq!(row.get::<i64, _>("canvas_id"), 9);
            assert_eq!(row.get::<String, _>("source_url"), "https://source");
        }
    }

    #[tokio::test]
    async fn content_end_claims_saves_backfills_done_and_reconciles() {
        let pool = migrated_pool().await;
        sqlx::query("INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ', 'Subject')")
            .execute(&pool).await.unwrap();
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
        assert_eq!(claim("watched", true).await, EndClaim::Running, "force never takes a running claim");
        assert!(save_content_end(&pool, "watched", Some((1915, "see you tomorrow"))).await.unwrap(),
            "1910 s watched is within 10 s of a 1915 s end");
        let saved = row("watched").await;
        assert_eq!(saved.get::<i64, _>("content_end_seconds"), 1915);
        assert_eq!(saved.get::<String, _>("content_end_quote"), "see you tomorrow");
        assert_eq!(saved.get::<String, _>("content_end_status"), "ready");
        assert_eq!(saved.get::<i64, _>("completed"), 1);
        assert_eq!(claim("watched", false).await, EndClaim::Found);
        assert_eq!(claim("watched", true).await, EndClaim::Claimed);
        assert_eq!(reconcile_content_end_status(&pool).await.unwrap(), 1);
        let swept = row("watched").await;
        assert_eq!(swept.get::<Option<String>, _>("content_end_status"), None);
        assert_eq!(swept.get::<i64, _>("content_end_seconds"), 1915, "the found end survives a sweep");

        assert_eq!(claim("early", false).await, EndClaim::Claimed);
        assert!(!save_content_end(&pool, "early", Some((1915, "that's it"))).await.unwrap());
        assert_eq!(row("early").await.get::<i64, _>("completed"), 0);
        assert_eq!(claim("early", true).await, EndClaim::Claimed);
        set_content_end_error(&pool, "early", "bad reply").await.unwrap();
        let failed = row("early").await;
        assert_eq!(failed.get::<String, _>("content_end_status"), "error");
        assert_eq!(failed.get::<String, _>("content_end_error"), "bad reply");
        assert_eq!(failed.get::<i64, _>("content_end_seconds"), 1915, "a failed re-run keeps the end");
        assert_eq!(claim("early", false).await, EndClaim::Claimed, "an error re-runs without force");
        assert!(!save_content_end(&pool, "early", None).await.unwrap());
        let none = row("early").await;
        assert_eq!(none.get::<String, _>("content_end_status"), "none");
        assert_eq!(none.get::<Option<i64>, _>("content_end_seconds"), None);
        assert_eq!(claim("missing", false).await, EndClaim::NoLecture);
    }

    // ── Page records ─────────────────────────────────────────────────────────

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
        crate::parse::ParsePage { page_no, markdown: markdown.to_string() }
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
        assert_eq!(rows[0].get::<Option<String>, _>("embed_model").as_deref(), Some("qwen"));
    }

    // ── Parse status reconciliation ──────────────────────────────────────────

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

        let updated = reconcile_parse_status(&pool, &data_dir).await.expect("reconcile");
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
        assert_eq!(status("courses/SUBJ/files/done.pdf").await.as_deref(), Some("quality"));
        assert_eq!(status("courses/SUBJ/files/gone.pdf").await, None);
    }
}
