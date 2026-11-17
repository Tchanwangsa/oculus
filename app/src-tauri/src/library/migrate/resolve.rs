//! Old citations after the move: a path written before it (a chat's tool
//! output, an undo snapshot, a saved tab) followed through `path_moves`.
//! `app/src/lib/files/pathMoves.ts` is the frontend's copy of the rule.

use sqlx::SqlitePool;

/// Where `rel` went: the longest `path_moves` prefix it starts with, at a
/// path boundary (`/`, `#page=`, `:L15`, `?`), with the rest carried over.
/// `None` when no rule covers it.
pub async fn resolve_moved(pool: &SqlitePool, rel: &str) -> Result<Option<String>, String> {
    sqlx::query_scalar(
        "SELECT new_prefix || substr(?1, length(old_prefix) + 1) FROM path_moves
          WHERE ?1 = old_prefix
             OR (substr(?1, 1, length(old_prefix)) = old_prefix
                 AND substr(?1, length(old_prefix) + 1, 1) IN ('/', '#', ':', '?'))
          ORDER BY length(old_prefix) DESC
          LIMIT 1",
    )
    .bind(rel)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("read path_moves: {e}"))
}
