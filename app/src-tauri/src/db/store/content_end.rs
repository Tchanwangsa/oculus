use sqlx::SqlitePool;

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
