use std::path::Path;

use sqlx::Row;

use crate::db::store::pool;

use super::plumbing::current_space;
use super::IndexStats;

/// What the index actually contains — "nothing indexed", "indexed, no
/// matches" and "indexed by a retired model" are different answers.
pub async fn stats(db_file: &Path) -> Result<IndexStats, String> {
    let (model, dim) = current_space();
    let db = pool(db_file).await?;

    // One pass, so the current-space and whole-table counts are consistent.
    let row = sqlx::query(
        r#"SELECT COUNT(DISTINCT CASE WHEN embed_model = ?1 AND embed_dim = ?2
                                      THEN file_id END)                    AS files_embedded,
                  SUM(CASE WHEN embed_model = ?1 AND embed_dim = ?2
                           THEN 1 ELSE 0 END)                              AS pages_embedded,
                  SUM(CASE WHEN embed_model = ?1 AND embed_dim = ?2
                            AND markdown != '' THEN 1 ELSE 0 END)          AS pages_with_markdown,
                  COUNT(DISTINCT file_id)                                  AS files_stored,
                  COUNT(*)                                                 AS pages_stored
           FROM pages WHERE embedding IS NOT NULL"#,
    )
    .bind(model)
    .bind(dim)
    .fetch_one(&db)
    .await
    .map_err(|e| e.to_string())?;

    // Named, not just counted, so a message can say which model.
    let stale_models: Vec<String> = sqlx::query(
        r#"SELECT DISTINCT embed_model FROM pages
           WHERE embedding IS NOT NULL
             AND NOT (embed_model IS ?1 AND embed_dim IS ?2)
             AND embed_model IS NOT NULL
           ORDER BY embed_model"#,
    )
    .bind(model)
    .bind(dim)
    .fetch_all(&db)
    .await
    .map_err(|e| e.to_string())?
    .iter()
    .filter_map(|row| row.try_get::<String, _>("embed_model").ok())
    .collect();
    db.close().await;

    let pages_embedded: i64 = row
        .try_get::<Option<i64>, _>("pages_embedded")
        .ok()
        .flatten()
        .unwrap_or(0);
    let pages_stored: i64 = row.try_get("pages_stored").unwrap_or(0);

    Ok(IndexStats {
        files_embedded: row.try_get("files_embedded").unwrap_or(0),
        pages_embedded,
        pages_with_markdown: row
            .try_get::<Option<i64>, _>("pages_with_markdown")
            .ok()
            .flatten()
            .unwrap_or(0),
        model: Some(model.to_string()),
        dim: Some(dim),
        files_stored: row.try_get("files_stored").unwrap_or(0),
        pages_stored,
        pages_stale: (pages_stored - pages_embedded).max(0),
        stale_models,
    })
}
