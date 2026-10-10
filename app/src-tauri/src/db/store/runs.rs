use sqlx::SqlitePool;

pub async fn start_run(pool: &SqlitePool, subject_codes: &[String]) -> Result<i64, String> {
    let codes_json = serde_json::to_string(subject_codes).unwrap_or_else(|_| "[]".to_string());
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

pub async fn add_log(
    pool: &SqlitePool,
    level: &str,
    message: &str,
    run_id: Option<i64>,
) -> Result<(), String> {
    sqlx::query("INSERT INTO sync_log (run_id, level, message) VALUES (?1, ?2, ?3)")
        .bind(run_id)
        .bind(level)
        .bind(message)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
