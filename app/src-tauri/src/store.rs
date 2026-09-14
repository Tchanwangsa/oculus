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

use std::path::Path;

use sqlx::{Row, SqlitePool};

use crate::sync::Course;

pub async fn open(data_dir: &Path) -> Result<SqlitePool, String> {
    let path = crate::paths::db_path(data_dir);
    if !path.exists() {
        return Err(format!(
            "no database at {} — open the Oculus app once to create it",
            path.display()
        ));
    }
    crate::retrieval::pool(&path).await
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

/// Derive parse status from what the sidecar left on disk. Without the app's
/// event listener running, this is how a CLI run's parse results reach the
/// database.
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
        let Some(status) = crate::paths::parse_mode(&data_dir.join(&pdf_rel)) else {
            continue;
        };

        let res = sqlx::query(
            "UPDATE files SET parse_status = ?1, parsed_at = datetime('now')
             WHERE relative_path = ?2 AND (parse_status IS NULL OR parse_status != ?1)",
        )
        .bind(status)
        .bind(&rel)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        updated += res.rows_affected();
    }
    Ok(updated)
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
