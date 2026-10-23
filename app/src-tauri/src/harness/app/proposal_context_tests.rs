use super::proposal_context::{accepted, read};
use serde_json::Value;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

async fn pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../db/proposals/schema.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO chat_proposals(item_id,block,thread_id,kind,status,state,initial_state) VALUES(12,0,1,'NewTasks','pending','{\"project\":null,\"items\":[]}','{\"project\":null,\"items\":[]}')").execute(&pool).await.unwrap();
    pool
}
fn cards(section: &str) -> Value {
    serde_json::from_str(section.lines().find(|s| s.starts_with('[')).unwrap()).unwrap()
}

#[tokio::test]
async fn pending_repeats_decided_once_and_failed_send_retries() {
    let pool = pool().await;
    let first = read(&pool, 1).await.unwrap();
    assert_eq!(cards(&first.section)[0]["edited"], false);
    accepted(&pool, first).await.unwrap();
    assert!(read(&pool, 1).await.unwrap().section.contains("pending"));
    sqlx::query("UPDATE chat_proposals SET status='discarded'")
        .execute(&pool)
        .await
        .unwrap();
    let failed = read(&pool, 1).await.unwrap();
    assert!(failed.section.contains("discarded"));
    // A rejected dispatch never calls accepted.
    let retry = read(&pool, 1).await.unwrap();
    assert_eq!(retry.section, failed.section);
    accepted(&pool, retry).await.unwrap();
    assert!(read(&pool, 1).await.unwrap().section.is_empty());
}

#[tokio::test]
async fn dispatch_reads_the_state_after_queueing_and_told_does_not_race() {
    let pool = pool().await;
    let queued = read(&pool, 1).await.unwrap();
    sqlx::query("UPDATE chat_proposals SET status='created',state='{\"items\":[{\"title\":\"Student edit\"}]}'").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO agent_changes(id,thread_id,proposal_id,entity,entity_id,op,after) VALUES(1,1,'12.0','task','42','create','{\"row\":{\"title\":\"Student edit\"}}')").execute(&pool).await.unwrap();
    accepted(&pool, queued).await.unwrap();
    let dispatch = read(&pool, 1).await.unwrap();
    let context = cards(&dispatch.section);
    assert_eq!(context[0]["status"], "created");
    assert_eq!(context[0]["made"][0]["id"], 42);
    sqlx::query("UPDATE chat_proposals SET status='undone'")
        .execute(&pool)
        .await
        .unwrap();
    accepted(&pool, dispatch).await.unwrap();
    assert_eq!(
        cards(&read(&pool, 1).await.unwrap().section)[0]["status"],
        "undone"
    );
}
