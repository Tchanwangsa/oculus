use sqlx::SqlitePool;

async fn pool_before_search() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all()
        .into_iter()
        .filter(|m| m.version < 61)
    {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("PRAGMA foreign_keys = ON; PRAGMA recursive_triggers = OFF;
        INSERT INTO harness_threads(id,provider) VALUES (1,'claude'), (2,'claude');
        INSERT INTO harness_items(id,thread_id,kind,content) VALUES (1,1,'user','eigenvalues café'),(2,2,'assistant','eigenvectors'),(3,1,'tool','eigenignored');")
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("search.sql"))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

async fn hits(pool: &SqlitePool, query: &str) -> Vec<i64> {
    sqlx::query_scalar(
        "SELECT rowid FROM harness_items_fts WHERE harness_items_fts MATCH ?1 ORDER BY rowid",
    )
    .bind(query)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn backfill_and_prefix_search_strip_diacritics() {
    let pool = pool_before_search().await;
    assert_eq!(hits(&pool, "\"eigen\"*").await, vec![1, 2]);
    assert_eq!(hits(&pool, "cafe").await, vec![1]);
}

#[tokio::test]
async fn triggers_track_inserts_edits_kind_changes_item_deletes_and_thread_cascades() {
    let pool = pool_before_search().await;
    sqlx::raw_sql("INSERT INTO harness_items(id,thread_id,kind,content) VALUES (4,1,'assistant','eigenbasis');
        UPDATE harness_items SET content = 'orthogonal' WHERE id = 1;
        UPDATE harness_items SET kind = 'tool' WHERE id = 2;
        UPDATE harness_items SET kind = 'assistant' WHERE id = 3;")
        .execute(&pool).await.unwrap();
    assert_eq!(hits(&pool, "eigen*").await, vec![3, 4]);
    assert_eq!(hits(&pool, "orthogonal").await, vec![1]);
    sqlx::query("DELETE FROM harness_items WHERE id = 4")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(hits(&pool, "eigen*").await, vec![3]);
    sqlx::query("DELETE FROM harness_threads WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        hits(&pool, "eigen* OR orthogonal").await.is_empty(),
        "cascade removes postings even with recursive_triggers off"
    );
}
