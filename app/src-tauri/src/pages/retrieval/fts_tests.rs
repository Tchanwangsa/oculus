use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use super::{PAGES_FTS_CHANGED_SQL, PAGES_FTS_SQL};

/// A `pages` table and the FTS index over it, from the SQL migrations 35 and 38 run.
async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");
    sqlx::query(
        "CREATE TABLE pages (
                 id        INTEGER PRIMARY KEY AUTOINCREMENT,
                 file_id   INTEGER NOT NULL,
                 page_no   INTEGER NOT NULL,
                 markdown  TEXT NOT NULL DEFAULT '',
                 embedding BLOB,
                 UNIQUE(file_id, page_no)
             )",
    )
    .execute(&pool)
    .await
    .expect("pages");
    // `raw_sql`: several statements, as the migration runner executes it.
    sqlx::raw_sql(PAGES_FTS_SQL)
        .execute(&pool)
        .await
        .expect("pages_fts");
    sqlx::raw_sql(PAGES_FTS_CHANGED_SQL)
        .execute(&pool)
        .await
        .expect("changed-only trigger");
    pool
}

async fn hits(pool: &SqlitePool, q: &str) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT p.page_no FROM pages_fts
               JOIN pages p ON p.id = pages_fts.rowid
              WHERE pages_fts MATCH ?1
              ORDER BY bm25(pages_fts)",
    )
    .bind(q)
    .fetch_all(pool)
    .await
    .expect("match")
}

/// Without FTS5 in the linked SQLite, migration 35 fails and the app cannot
/// open its database; `libsqlite3-sys`' bundled build must keep it.
#[tokio::test]
async fn fts5_indexes_inserts_updates_and_deletes() {
    let pool = pool().await;
    sqlx::query("INSERT INTO pages (file_id, page_no, markdown) VALUES (1, 1, ?1)")
        .bind("A Nash equilibrium is a profile of strategies")
        .execute(&pool)
        .await
        .expect("insert");
    assert_eq!(hits(&pool, "nash").await, vec![1], "insert trigger");

    // An embed's blob write must not drop the row from the index.
    sqlx::query("UPDATE pages SET embedding = ?1 WHERE page_no = 1")
        .bind(vec![0u8; 8])
        .execute(&pool)
        .await
        .expect("embed");
    assert_eq!(
        hits(&pool, "nash").await,
        vec![1],
        "blob write left the index alone"
    );

    // A re-parse replaces the text: the old terms must stop matching.
    sqlx::query("UPDATE pages SET markdown = ?1 WHERE page_no = 1")
        .bind("A dominant strategy dominates every alternative")
        .execute(&pool)
        .await
        .expect("reparse");
    assert!(
        hits(&pool, "nash").await.is_empty(),
        "update trigger cleared the old terms"
    );
    assert_eq!(
        hits(&pool, "dominant").await,
        vec![1],
        "update trigger indexed the new ones"
    );

    sqlx::query("DELETE FROM pages WHERE page_no = 1")
        .execute(&pool)
        .await
        .expect("delete");
    assert!(hits(&pool, "dominant").await.is_empty(), "delete trigger");
}

#[tokio::test]
async fn repeated_upserts_leave_the_fts_index_untouched() {
    let pool = pool().await;
    let upsert = "INSERT INTO pages (file_id, page_no, markdown) VALUES (1, 1, ?1)
                      ON CONFLICT(file_id, page_no) DO UPDATE SET markdown = excluded.markdown";
    sqlx::query(upsert)
        .bind("Shannon entropy")
        .execute(&pool)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(upsert)
        .bind("Shannon entropy")
        .execute(&pool)
        .await
        .unwrap();
    let unchanged: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unchanged - before, 1, "only the page row should be written");
    assert_eq!(hits(&pool, "entropy").await, vec![1]);
    sqlx::query(upsert)
        .bind("Nash equilibrium")
        .execute(&pool)
        .await
        .unwrap();
    let changed: i64 = sqlx::query_scalar("SELECT total_changes()")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        changed - unchanged > 1,
        "changed text must update the FTS index"
    );
    assert!(hits(&pool, "entropy").await.is_empty());
    assert_eq!(hits(&pool, "nash").await, vec![1]);
}

/// `snippet()` marks matched prose; prefix terms answer mid-word.
#[tokio::test]
async fn snippet_marks_the_matched_words() {
    let pool = pool().await;
    sqlx::query("INSERT INTO pages (file_id, page_no, markdown) VALUES (1, 1, ?1)")
        .bind("Shannon entropy measures the uncertainty of a source")
        .execute(&pool)
        .await
        .expect("insert");
    let snippet: String = sqlx::query_scalar(
        "SELECT snippet(pages_fts, 0, '<', '>', '…', 8)
               FROM pages_fts WHERE pages_fts MATCH ?1",
    )
    .bind("\"entrop\"*")
    .fetch_one(&pool)
    .await
    .expect("snippet");
    assert!(snippet.contains("<entropy>"), "got {snippet}");
}
