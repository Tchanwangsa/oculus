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

/// Recap runs that were killed mid-job. The windows already written remain
/// visible, but `running` cannot survive the process that owned it or the
/// player would wait forever for progress that can no longer arrive.
pub async fn reconcile_recap_status(pool: &SqlitePool) -> Result<u64, String> {
    sqlx::query(
        "UPDATE lectures SET recap_status = NULL, recap_error = NULL
          WHERE recap_status = 'running'",
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

/// Atomically claim a recap run and clear the previous derived set.
///
/// The conditional update is the one shared gate for the app and CLI. Two
/// callers may race to this transaction, but only the first can change a row
/// that is not already `running`; the loser spends no model turn. Clearing the
/// old notes is in the same transaction, so a failed delete cannot strand the
/// lecture in `running`.
pub async fn claim_recap(pool: &SqlitePool, lecture_id: &str) -> Result<bool, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let claimed = sqlx::query(
        "UPDATE lectures
            SET recap_status = 'running', recap_error = NULL, recapped_at = NULL
          WHERE id = ?1 AND (recap_status IS NULL OR recap_status <> 'running')",
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
    sqlx::query("DELETE FROM lecture_recap WHERE lecture_id = ?1")
        .bind(lecture_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(true)
}

/// The recap job's status, terminal timestamp and failure message.
///
/// `recapped_at` records the most recent terminal transition, including an
/// error after some windows were saved. Starting or reconciling a run clears
/// it; every non-error transition clears the previous failure message.
pub async fn set_recap_status(
    pool: &SqlitePool,
    lecture_id: &str,
    status: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let terminal = matches!(status, Some("ready") | Some("error"));
    sqlx::query(
        "UPDATE lectures
            SET recap_status = ?1,
                recap_error  = ?2,
                recapped_at   = CASE WHEN ?3 THEN datetime('now') ELSE NULL END
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

/// Append one validated recap window atomically.
///
/// Windows commit independently by design. `idx` continues from the rows
/// already present, which keeps play order stable while allowing the panel to
/// show completed windows during a long run. The caller marks the lecture
/// `ready` only after every window has landed.
pub async fn save_recap_window(
    pool: &SqlitePool,
    lecture_id: &str,
    notes: &[crate::recap::RecapNote],
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let first_idx: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(idx) + 1, 0) FROM lecture_recap WHERE lecture_id = ?1",
    )
    .bind(lecture_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    for (offset, note) in notes.iter().enumerate() {
        sqlx::query(
            "INSERT INTO lecture_recap (lecture_id, idx, start_seconds, label, body)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(lecture_id)
        .bind(first_idx + offset as i64)
        .bind(i64::from(note.start_seconds))
        .bind(&note.label)
        .bind(&note.body)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}

/// A lecture's recap notes in play order, including complete windows from a
/// run that is still in progress or ended with an error.
pub async fn recap(
    pool: &SqlitePool,
    lecture_id: &str,
) -> Result<Vec<crate::recap::RecapNote>, String> {
    let rows = sqlx::query(
        "SELECT start_seconds, label, body FROM lecture_recap
          WHERE lecture_id = ?1 ORDER BY idx",
    )
    .bind(lecture_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| crate::recap::RecapNote {
            start_seconds: r.get::<i64, _>("start_seconds").max(0) as u32,
            label: r.get("label"),
            body: r.get("body"),
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

    async fn recap_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE lectures (
                id TEXT PRIMARY KEY,
                recap_status TEXT,
                recapped_at TEXT,
                recap_error TEXT
             )",
        )
        .execute(&pool)
        .await
        .expect("lecture schema");
        sqlx::query(
            "CREATE TABLE lecture_recap (
                lecture_id TEXT NOT NULL REFERENCES lectures(id) ON DELETE CASCADE,
                idx INTEGER NOT NULL,
                start_seconds INTEGER NOT NULL,
                label TEXT NOT NULL,
                body TEXT NOT NULL,
                PRIMARY KEY (lecture_id, idx)
             )",
        )
        .execute(&pool)
        .await
        .expect("recap schema");
        sqlx::query("INSERT INTO lectures (id) VALUES ('lecture-1')")
            .execute(&pool)
            .await
            .expect("lecture row");
        pool
    }

    #[tokio::test]
    async fn recap_windows_append_in_play_order_and_claim_clears_for_a_fresh_run() {
        let pool = recap_pool().await;
        let first = vec![crate::recap::RecapNote {
            start_seconds: 0,
            label: "Opening".into(),
            body: "The lecture begins.".into(),
        }];
        let second = vec![
            crate::recap::RecapNote {
                start_seconds: 40,
                label: "Definition".into(),
                body: "A definition appears.".into(),
            },
            crate::recap::RecapNote {
                start_seconds: 75,
                label: String::new(),
                body: "The example continues.".into(),
            },
        ];

        save_recap_window(&pool, "lecture-1", &first).await.unwrap();
        save_recap_window(&pool, "lecture-1", &second).await.unwrap();
        let saved = recap(&pool, "lecture-1").await.unwrap();
        assert_eq!(
            saved.iter().map(|n| n.start_seconds).collect::<Vec<_>>(),
            vec![0, 40, 75]
        );

        assert!(claim_recap(&pool, "lecture-1").await.unwrap());
        assert!(recap(&pool, "lecture-1").await.unwrap().is_empty());
        assert!(!claim_recap(&pool, "lecture-1").await.unwrap());
    }

    #[tokio::test]
    async fn recap_status_stamps_only_terminal_runs_and_reconciles_running() {
        let pool = recap_pool().await;
        set_recap_status(&pool, "lecture-1", Some("running"), None)
            .await
            .unwrap();
        assert_eq!(reconcile_recap_status(&pool).await.unwrap(), 1);

        set_recap_status(&pool, "lecture-1", Some("error"), Some("bad window"))
            .await
            .unwrap();
        let row = sqlx::query(
            "SELECT recap_status, recapped_at, recap_error FROM lectures WHERE id = 'lecture-1'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("recap_status"), "error");
        assert!(row.get::<Option<String>, _>("recapped_at").is_some());
        assert_eq!(row.get::<String, _>("recap_error"), "bad window");
    }
}
