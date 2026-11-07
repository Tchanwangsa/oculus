use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::Row;

use crate::harness::store;

use super::guard::{assert_scratch, scratch};

#[tokio::test]
async fn legacy_upgrade_and_pruning_preserve_source_ids_and_frozen_sql() {
    let scratch = scratch("turn-upgrade");
    let database = assert_scratch(&scratch.join("oculus.db"), &scratch);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    let checksums: BTreeMap<String, String> = serde_json::from_str(include_str!(
        "../../../fixtures/harness/turn-model/schema-checksums.json"
    ))
    .unwrap();
    for migration in crate::db::migrations::all() {
        if let Some(expected) = checksums.get(&migration.version.to_string()) {
            assert_eq!(
                &format!("{:x}", Sha256::digest(migration.sql.as_bytes())),
                expected,
                "frozen SQL {}",
                migration.version
            );
        }
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
        if migration.version == 24 {
            sqlx::raw_sql("INSERT INTO harness_threads(id,provider,title) VALUES(7,'claude','legacy'); INSERT INTO harness_items(id,thread_id,kind,content) VALUES(40,7,'assistant','legacy widget source'),(41,7,'assistant','later source');").execute(&pool).await.unwrap();
        }
    }
    let rows = super::snapshot::items(&pool, 7).await;
    assert_eq!(
        rows.iter()
            .map(|r| (r["id"].as_i64().unwrap(), r["content"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        vec![(40, "legacy widget source"), (41, "later source")]
    );
    // Migration 54 adds ownership; rows from before it stay unowned.
    for row in &rows {
        for column in ["run_id", "attempt_id", "entity_ref"] {
            assert!(row[column].is_null(), "legacy {column}: {row}");
        }
    }
    sqlx::raw_sql("INSERT INTO chat_proposals(item_id,block,thread_id,kind,status,state,initial_state) VALUES(40,0,7,'NewTasks','pending','{}','{}'),(41,0,7,'NewTasks','created','{}','{}');").execute(&pool).await.unwrap();
    assert_eq!(store::truncate_from(&pool, 7, 41).await.unwrap(), 1);
    let retained = sqlx::query("SELECT item_id,block,status FROM chat_proposals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(retained.get::<i64, _>("item_id"), 40);
    assert_eq!(retained.get::<i64, _>("block"), 0);
    assert_eq!(retained.get::<String, _>("status"), "pending");
    store::delete_thread(&pool, 7).await.unwrap();
    assert!(super::snapshot::items(&pool, 7).await.is_empty());
    pool.close().await;
}
