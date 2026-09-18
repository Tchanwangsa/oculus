//! Database writes for headless runs.
//!
//! In the app the frontend owns these tables: it listens for scrape events and
//! upserts through tauri-plugin-sql. The CLI has no frontend, so it writes the
//! same rows with the same SQL — same shape, same conflict handling — and the
//! app picks the run up as if it had done the work itself.
//!
//! Schema ownership stays with the plugin's migrations. If the database does
//! not exist yet, we do not invent one; the caller reports that and keeps
//! scraping to disk.

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use tauri::{AppHandle, Manager};

use crate::sync::Course;

// ── The connection pool ──────────────────────────────────────────────────────
//
// These three lived in `retrieval.rs` while retrieval was the only thing in
// Rust that touched the database on its own. It is not any more — the parse
// path writes page records too — so helpers every module needs do not belong
// inside one of them. This is the DB-access module and the one they all
// already depend on; the helpers belong here and every caller says `store::`.

/// Our own pool over the file tauri-plugin-sql already manages. WAL means a
/// second reader is harmless, and our writes are occasional (once per file
/// parsed), so a busy timeout is enough to stay out of the plugin's way.
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

/// Where the app's database is, asked of Tauri rather than recomputed. The
/// answer is the same one `paths::db_path(paths::data_dir())` gives; this is
/// for the command layer, which already has a handle.
pub fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("oculus.db"))
}

/// Open the shared database the same way the CLI does — no AppHandle, so this
/// works headless.
pub async fn open_pool() -> Result<SqlitePool, String> {
    let path = crate::paths::db_path(&crate::paths::data_dir());
    if !path.exists() {
        return Err(format!("no database at {}", path.display()));
    }
    pool(&path).await
}

/// Read one `settings` row from a **synchronous** caller, from any context.
///
/// Both seams need to know which backend is selected before they have an async
/// frame to await in: the parse queue and the CLI are plain threads, and
/// `parse_config` / `embed_config` are called from inside clients that are not
/// async at all.
///
/// The obvious spelling for that — `tauri::async_runtime::block_on` — is a
/// **trap**, and it cost a real bug. It is correct on a plain thread and on a
/// `spawn_blocking` worker, and it *panics* on a runtime worker thread:
/// "Cannot start a runtime from within a runtime". An `async` Tauri command
/// runs on exactly such a thread, so `embed_settings` aborted mid-task, its
/// promise never settled, and Settings → Library sat on its loading state
/// forever — every field a dash, no error to show, because an aborted task
/// rejects nothing. The indexing and search paths were fine the whole time,
/// which is what made it look like a data problem instead of a crash.
///
/// So this never touches the caller's runtime. The read happens on a thread of
/// its own with a current-thread runtime and the caller joins it: one `SELECT`
/// a few times per run makes the spawn free, and **one code path** means the
/// behaviour cannot depend on who is calling. Never reintroduce a `block_on`
/// here, and never make its safety a fact about the call site.
///
/// `None` covers every uninteresting case — no database yet, no such row,
/// a read that failed — because every caller's answer to all three is the same
/// default.
pub fn setting_blocking(key: &str) -> Option<String> {
    let database = crate::paths::db_path(&crate::paths::data_dir());
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
            let row = sqlx::query("SELECT value FROM settings WHERE key = ?1")
                .bind(&key)
                .fetch_optional(&mut connection)
                .await
                .ok()
                .flatten();
            connection.close().await.ok();
            row?.try_get::<String, _>("value").ok()
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
    // Last-synced is derived, not stored: the finish time of the latest
    // completed run whose subject_codes contain the subject. Mirrors
    // getSubjects in app/src/lib/db.ts — keep the two in step.
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

    // `changed` is the write action from the engine ('new'/'updated' vs
    // 'unchanged') — content_changed_at only moves when bytes actually did.
    let sql = if changed {
        r#"INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category, canvas_id, source_url, first_seen_at, content_changed_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'), datetime('now'))
           ON CONFLICT(subject_id, relative_path) DO UPDATE SET
             filename   = excluded.filename,
             file_type  = excluded.file_type,
             size_bytes = excluded.size_bytes,
             category   = excluded.category,
             canvas_id  = excluded.canvas_id,
             source_url = excluded.source_url,
             scraped_at = datetime('now'),
             content_changed_at = datetime('now')"#
    } else {
        r#"INSERT INTO files (subject_id, filename, relative_path, file_type, size_bytes, category, canvas_id, source_url, first_seen_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, datetime('now'))
           ON CONFLICT(subject_id, relative_path) DO UPDATE SET
             filename   = excluded.filename,
             file_type  = excluded.file_type,
             size_bytes = excluded.size_bytes,
             category   = excluded.category,
             canvas_id  = excluded.canvas_id,
             source_url = excluded.source_url,
             scraped_at = datetime('now')"#
    };
    sqlx::query(sql)
    .bind(subject_id)
    .bind(&filename)
    .bind(relative_path)
    .bind(&file_type)
    .bind(size_bytes as i64)
    .bind(category)
    .bind(canvas_id)
    .bind(source_url)
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
/// This used to happen on the *embed* path only (`retrieval::ingest`), which
/// meant `pages.markdown` — the table `oculus grep` reads — was a side effect
/// of building the vector index. With embeddings going away, finishing a parse
/// has to write its own page records or the tool the user actually reaches for
/// would quietly go blank.
///
/// The conflict clause is deliberately **not** a `COALESCE`: an empty incoming
/// markdown must leave good text alone. A page that yields nothing (a slide
/// that is one full-bleed image) normalises to `""` in `ParseOutput`, and a
/// re-parse that produced fewer pages than the last one would otherwise wipe
/// the text the last one found.
///
/// Nothing here touches `embedding` / `embed_model` / `embed_dim` /
/// `embedded_at`. Those stay the embedder's until it stops writing them, and
/// the columns stay in the schema either way.
pub async fn upsert_pages(
    pool: &SqlitePool,
    file_id: i64,
    pages: &[crate::parse::ParsePage],
) -> Result<usize, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
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
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("upsert page {}: {e}", page.page_no))?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(with_text)
}

/// How many page rows this file already has. Cheap enough to ask before
/// deciding whether an already-parsed file needs its record folding in.
pub async fn page_count(pool: &SqlitePool, file_id: i64) -> Result<i64, String> {
    sqlx::query_scalar("SELECT COUNT(*) FROM pages WHERE file_id = ?1")
        .bind(file_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())
}

/// Every PDF-backed file on record (PDFs and Office documents with a derived
/// sibling PDF), optionally narrowed to a set of subjects.
pub async fn pdf_files(
    pool: &SqlitePool,
    subject_ids: &[i64],
) -> Result<Vec<(i64, String)>, String> {
    let rows = sqlx::query(
        "SELECT subject_id, relative_path FROM files ORDER BY relative_path",
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

/// Derive parse status from what the parser left on disk. Without the app's
/// event listener running, this is how a CLI run's parse results reach the
/// database.
///
/// `parse::parse_mode` answers a binary question now — `Some("quality")` or
/// `None` — where it used to have a third value. `None` **clears** the row
/// rather than skipping it, and that is the whole point of the sweep in the
/// other direction: `files.parse_status` also holds the transient states the
/// app writes from `parse-status` events (`queued`, `running`, `error`), and a
/// run that is killed mid-parse leaves one of those behind with nothing left
/// alive to finish it. The artifacts on disk are the only durable truth, so a
/// row claiming anything the disk does not back is stale by definition. It is
/// the same reconciliation `reconcile_chapter_status` performs.
///
/// Nothing in the live library actually changes value today: 166 rows say
/// `quality` and have the record to prove it, 540 are already NULL, and the
/// `fast` the old three-value reader could invent never made it to disk.
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
                     WHERE relative_path = ?1 AND parse_status IS NOT NULL",
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

/// Runs that were killed mid-job: `running` with no `chaptered_at`, and
/// nothing left to finish them. Called at startup, the sibling of
/// `reconcile_parse_status` above and of `harness::store::reconcile` — a
/// status column that only a live process can clear needs a sweep behind it,
/// or one crash leaves the button saying "chaptering" forever.
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

/// Reading-copy runs that were killed mid-job. The windows already written remain
/// visible, but `running` cannot survive the process that owned it or the
/// player would wait forever for progress that can no longer arrive.
pub async fn reconcile_reading_status(pool: &SqlitePool) -> Result<u64, String> {
    sqlx::query(
        "UPDATE lectures SET reading_status = NULL, reading_error = NULL
          WHERE reading_status = 'running'",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .map_err(|e| e.to_string())
}

// ── Calendar ─────────────────────────────────────────────────────────────────

/// Replace a subject's calendar rows with what Canvas just returned.
///
/// Delete-then-insert, not upsert: a class moved or cancelled in Canvas has to
/// vanish from the calendar, and an upsert over a growing set would leave the
/// old occurrence sitting there forever. The fetch is always the complete set
/// for the course, so the replacement is safe.
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
    /// Echo360's media id, and the name of the folder under `lectures/`. The
    /// CLI's only handle on one lecture, so `list -l` has to print it.
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

/// Record where a downloaded artifact landed, so the app can play it without
/// re-deriving the path.
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
/// A sibling of [`set_lecture_path`] rather than another arm of it: that
/// function's allow-list takes a `&str` value and so cannot clear a column
/// back to NULL, which is exactly what starting a run and succeeding at one
/// both have to do. `status` NULL means "never chaptered"; only a terminal
/// status stamps `chaptered_at`, and `error` is cleared by every write that
/// does not carry one.
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
/// One transaction for the whole set, and the caller has already run
/// `chapters::validate` over it — half a chapter list is worse than none,
/// because a missing chapter is not a gap on the scrub bar but twenty extra
/// minutes silently attributed to the chapter before it. The delete is in the
/// same transaction as the inserts for the same reason: a regenerate that
/// fails partway must leave the chapters that were already there.
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

/// Atomically claim a reading-copy run and clear the previous derived set.
///
/// The conditional update is the one shared gate for the app and CLI. Two
/// callers may race to this transaction, but only the first can change a row
/// that is not already `running`; the loser spends no model turn. Clearing the
/// old lines is in the same transaction, so a failed delete cannot strand the
/// lecture in `running`.
pub async fn claim_reading(pool: &SqlitePool, lecture_id: &str) -> Result<bool, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let claimed = sqlx::query(
        "UPDATE lectures
            SET reading_status = 'running', reading_error = NULL, reading_written_at = NULL
          WHERE id = ?1 AND (reading_status IS NULL OR reading_status <> 'running')",
    )
        .bind(lecture_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected()
        == 1;
    if !claimed {
        tx.rollback().await.map_err(|e| e.to_string())?;
        return Ok(false);
    }
    sqlx::query("DELETE FROM lecture_reading WHERE lecture_id = ?1")
        .bind(lecture_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

/// The reading-copy job's status, terminal timestamp and failure message.
///
/// `reading_written_at` records the most recent terminal transition, including an
/// error after some windows were saved. Starting or reconciling a run clears
/// it; every non-error transition clears the previous failure message.
pub async fn set_reading_status(
    pool: &SqlitePool,
    lecture_id: &str,
    status: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let terminal = matches!(status, Some("ready") | Some("error"));
    sqlx::query(
        "UPDATE lectures
            SET reading_status = ?1,
                reading_error  = ?2,
                reading_written_at   = CASE WHEN ?3 THEN datetime('now') ELSE NULL END
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

/// Append one validated reading window atomically.
///
/// Windows commit independently by design. `idx` continues from the rows
/// already present, which keeps play order stable while allowing the panel to
/// show completed windows during a long run. The caller marks the lecture
/// `ready` only after every window has landed.
pub async fn save_reading_window(
    pool: &SqlitePool,
    lecture_id: &str,
    lines: &[crate::reading::ReadingLine],
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let first_idx: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(idx) + 1, 0) FROM lecture_reading WHERE lecture_id = ?1",
    )
    .bind(lecture_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    for (offset, line) in lines.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lecture_reading (lecture_id, idx, start_seconds, para, text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(lecture_id)
        .bind(first_idx + offset as i64)
        .bind(i64::from(line.start_seconds))
        .bind(i64::from(line.para))
        .bind(&line.text)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// A lecture's reading copy in play order, including complete windows from a
/// run that is still in progress or ended with an error.
pub async fn reading(
    pool: &SqlitePool,
    lecture_id: &str,
) -> Result<Vec<crate::reading::ReadingLine>, String> {
    let rows = sqlx::query(
        "SELECT start_seconds, para, text FROM lecture_reading
          WHERE lecture_id = ?1 ORDER BY idx",
    )
    .bind(lecture_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| crate::reading::ReadingLine {
            start_seconds: r.get::<i64, _>("start_seconds").max(0) as u32,
            para: r.get::<i64, _>("para") != 0,
            text: r.get("text"),
        })
        .collect())
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

    async fn reading_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE lectures (
                id TEXT PRIMARY KEY,
                reading_status TEXT,
                reading_written_at TEXT,
                reading_error TEXT
             )",
        )
        .execute(&pool)
        .await
        .expect("lecture schema");
        sqlx::query(
            "CREATE TABLE lecture_reading (
                lecture_id TEXT NOT NULL REFERENCES lectures(id) ON DELETE CASCADE,
                idx INTEGER NOT NULL,
                start_seconds INTEGER NOT NULL,
                para INTEGER NOT NULL DEFAULT 0,
                text TEXT NOT NULL,
                PRIMARY KEY (lecture_id, idx)
             )",
        )
        .execute(&pool)
        .await
        .expect("reading schema");
        sqlx::query("INSERT INTO lectures (id) VALUES ('lecture-1')")
            .execute(&pool)
            .await
            .expect("lecture row");
        pool
    }

    #[tokio::test]
    async fn reading_windows_append_in_play_order_and_claim_clears_for_a_fresh_run() {
        let pool = reading_pool().await;
        let first = vec![crate::reading::ReadingLine {
            start_seconds: 0,
            para: true,
            text: "We begin with the definition.".into(),
        }];
        let second = vec![
            crate::reading::ReadingLine {
                start_seconds: 40,
                para: true,
                text: "A state is $a_0|0\\rangle + a_1|1\\rangle$.".into(),
            },
            crate::reading::ReadingLine {
                start_seconds: 75,
                para: false,
                text: "The example continues.".into(),
            },
        ];

        save_reading_window(&pool, "lecture-1", &first).await.unwrap();
        save_reading_window(&pool, "lecture-1", &second).await.unwrap();
        let saved = reading(&pool, "lecture-1").await.unwrap();
        assert_eq!(
            saved.iter().map(|n| n.start_seconds).collect::<Vec<_>>(),
            vec![0, 40, 75]
        );
        assert_eq!(
            saved.iter().map(|n| n.para).collect::<Vec<_>>(),
            vec![true, true, false],
            "para round-trips through the integer column"
        );
        assert_eq!(saved[1].text, second[0].text);

        assert!(claim_reading(&pool, "lecture-1").await.unwrap());
        assert!(reading(&pool, "lecture-1").await.unwrap().is_empty());
        assert!(!claim_reading(&pool, "lecture-1").await.unwrap());
    }

    #[tokio::test]
    async fn reading_status_stamps_only_terminal_runs_and_reconciles_running() {
        let pool = reading_pool().await;
        set_reading_status(&pool, "lecture-1", Some("running"), None)
            .await
            .unwrap();
        assert_eq!(reconcile_reading_status(&pool).await.unwrap(), 1);

        set_reading_status(&pool, "lecture-1", Some("error"), Some("bad window"))
            .await
            .unwrap();
        let row = sqlx::query(
            "SELECT reading_status, reading_written_at, reading_error FROM lectures WHERE id = 'lecture-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("reading_status"), "error");
        assert!(row.get::<Option<String>, _>("reading_written_at").is_some());
        assert_eq!(row.get::<String, _>("reading_error"), "bad window");
    }

    // ── Page records ─────────────────────────────────────────────────────────

    async fn pages_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        // The shape migration 12 created, embedding columns included: the
        // parse path must leave them alone, not drop them.
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

        // Pretend the embedder has been over it. Re-parsing must not disturb
        // the vector columns — they stay the embedder's until it stops
        // writing them.
        sqlx::query("UPDATE pages SET embedding = X'00', embed_model = 'qwen' WHERE page_no = 1")
            .execute(&pool)
            .await
            .expect("fake embedding");

        // A second parse that came back thinner: page 2 now empty, page 1
        // rewritten. The empty one must leave the good text standing — this is
        // why the conflict clause is a CASE and not a COALESCE.
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
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let data_dir = std::env::temp_dir().join(format!("oculus-reconcile-{stamp}"));
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

        std::fs::remove_dir_all(&data_dir).ok();
    }
}
