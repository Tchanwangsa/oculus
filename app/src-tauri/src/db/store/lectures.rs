use sqlx::{Row, SqlitePool};

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
    lectures: &[crate::sources::echo360::Lecture],
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
    chapters: &[crate::lectures::chapters::Chapter],
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
) -> Result<Vec<crate::lectures::chapters::Chapter>, String> {
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
        .map(|r| crate::lectures::chapters::Chapter {
            start_seconds: r.get::<i64, _>("start_seconds").max(0) as u32,
            title: r.get("title"),
            summary: r.get("summary"),
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
