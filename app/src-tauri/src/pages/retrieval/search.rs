use std::collections::HashMap;
use std::path::Path;

use sqlx::Row;

use crate::db::store::pool;
use crate::embed;

use super::SearchHit;

/// Rank pages against a natural-language query.
///
/// `subject_id` scopes the scan to one subject; omit it to search everything.
pub async fn search(
    db_file: &Path,
    query: String,
    limit: i64,
    subject_id: Option<i64>,
) -> Result<Vec<SearchHit>, String> {
    let ids: Vec<i64> = subject_id.into_iter().collect();
    search_in(db_file, query, limit, &ids).await
}

/// `search`, but over a set of subjects (empty means every subject), so the
/// CLI's prefix codes embed the query once.
pub async fn search_in(
    db_file: &Path,
    query: String,
    limit: i64,
    subject_ids: &[i64],
) -> Result<Vec<SearchHit>, String> {
    let (qvec, model, dim) = tauri::async_runtime::spawn_blocking(move || {
        let backend = embed::backend()?;
        // Before the query costs anything; also names the space to scan.
        let health = embed::preflight(backend.as_ref())?;
        let vector = backend.embed_query(&query)?;
        Ok::<_, embed::EmbedError>((vector, health.model, health.dim))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    if qvec.len() != dim {
        return Err(format!(
            "query vector is {} dims, the backend claims {dim}",
            qvec.len()
        ));
    }
    rank(db_file, &qvec, &model, dim as i64, limit, subject_ids).await
}

pub(super) fn score_blob(blob: &[u8], qvec: &[f32]) -> Option<f32> {
    if blob.len() / 2 != qvec.len() {
        return None;
    }
    Some(
        blob.chunks_exact(2)
            .zip(qvec)
            .map(|(bytes, query)| half::f16::from_le_bytes([bytes[0], bytes[1]]).to_f32() * query)
            .sum(),
    )
}

/// Score on a blocking worker, keep only the requested matches, and hydrate
/// their markdown after ranking. Both reads share one SQLite snapshot.
pub(super) async fn rank(
    db_file: &Path,
    qvec: &[f32],
    model: &str,
    dim: i64,
    limit: i64,
    subject_ids: &[i64],
) -> Result<Vec<SearchHit>, String> {
    let limit = limit.clamp(1, 50) as usize;
    let db = pool(db_file).await?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    // Inlined: sqlx has no list binding, and these are i64s.
    let filter = if subject_ids.is_empty() {
        String::new()
    } else {
        let list: Vec<String> = subject_ids.iter().map(|i| i.to_string()).collect();
        format!(" AND f.subject_id IN ({})", list.join(","))
    };
    // The space predicate, in SQL so retired vectors are never decoded.
    let sql = format!(
        r#"
        SELECT p.id, p.embedding
        FROM pages p
        JOIN files f ON f.id = p.file_id
        WHERE p.embedding IS NOT NULL
          AND p.embed_model = ?1
          AND p.embed_dim   = ?2{filter}
    "#
    );
    let rows = sqlx::query(&sql)
        .bind(model)
        .bind(dim)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    let qvec = qvec.to_vec();
    let best = tauri::async_runtime::spawn_blocking(move || {
        let mut scored: Vec<(i64, f32)> = Vec::with_capacity(rows.len());
        for row in rows {
            let blob: &[u8] = row.try_get("embedding").map_err(|e| e.to_string())?;
            let Some(score) = score_blob(blob, &qvec) else {
                continue;
            };
            scored.push((row.try_get("id").map_err(|e| e.to_string())?, score));
        }
        // Stable, so equal scores keep the scan's order.
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(limit);
        Ok::<_, String>(scored)
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut hits = Vec::with_capacity(best.len());
    if !best.is_empty() {
        let ids = best
            .iter()
            .map(|(id, _)| id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT p.id, p.file_id, p.page_no, p.markdown, f.filename, f.relative_path, f.subject_id
             FROM pages p JOIN files f ON f.id = p.file_id WHERE p.id IN ({ids})"
        );
        let rows = sqlx::query(&sql)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        let mut rows: HashMap<i64, _> = rows
            .into_iter()
            .map(|row| row.try_get("id").map(|id| (id, row)))
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())?;
        for (id, score) in best {
            let row = rows
                .remove(&id)
                .ok_or_else(|| format!("ranked page {id} vanished"))?;
            hits.push(SearchHit {
                file_id: row.try_get("file_id").map_err(|e| e.to_string())?,
                page_no: row.try_get("page_no").map_err(|e| e.to_string())?,
                score,
                filename: row.try_get("filename").map_err(|e| e.to_string())?,
                relative_path: row.try_get("relative_path").map_err(|e| e.to_string())?,
                subject_id: row.try_get("subject_id").map_err(|e| e.to_string())?,
                markdown: row.try_get("markdown").map_err(|e| e.to_string())?,
            });
        }
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    db.close().await;
    Ok(hits)
}
