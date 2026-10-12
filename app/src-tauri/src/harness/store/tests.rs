use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Row, SqlitePool};

use super::items::insert_item;
use super::*;
use crate::harness::event::{HarnessEvent, Provider, ToolKind};

async fn migrated_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    pool
}

fn started(id: &str) -> HarnessEvent {
    HarnessEvent::ToolStarted {
        id: id.into(),
        kind: ToolKind::Bash,
        name: "Bash".into(),
        title: "ls".into(),
        input: serde_json::json!({ "command": "ls" }),
    }
}

async fn meta(pool: &SqlitePool, item_id: i64) -> serde_json::Value {
    let raw: String = sqlx::query("SELECT meta FROM harness_items WHERE id = ?1")
        .bind(item_id)
        .fetch_one(pool)
        .await
        .unwrap()
        .get("meta");
    serde_json::from_str(&raw).unwrap()
}

#[tokio::test]
async fn reconcile_idles_running_threads_and_closes_open_tool_rows() {
    let pool = migrated_pool().await;
    let thread = create_thread(&pool, Provider::Claude, None, None, None, "hi")
        .await
        .unwrap();
    sqlx::query("UPDATE harness_threads SET status = 'running' WHERE id = ?1")
        .bind(thread)
        .execute(&pool)
        .await
        .unwrap();
    let open = apply(&pool, thread, &started("a")).await.unwrap().unwrap();
    let done = apply(&pool, thread, &started("b")).await.unwrap().unwrap();
    apply(
        &pool,
        thread,
        &HarnessEvent::ToolFinished {
            id: "b".into(),
            ok: true,
            output: "file.txt".into(),
            title: None,
        },
    )
    .await
    .unwrap();
    let garbled = insert_item(&pool, thread, "tool", Some("c"), None, Some("{oops".into()))
        .await
        .unwrap();

    let r = reconcile(&pool).await.unwrap();
    assert_eq!((r.threads, r.tools), (1, 1));

    let status: String = sqlx::query("SELECT status FROM harness_threads WHERE id = ?1")
        .bind(thread)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("status");
    assert_eq!(status, "idle");

    let closed = meta(&pool, open).await;
    assert_eq!(closed["ok"], serde_json::json!(false));
    assert_eq!(closed["output"], "the session ended before this finished");
    assert_eq!(closed["name"], "Bash");
    assert_eq!(closed["input"]["command"], "ls");

    let untouched = meta(&pool, done).await;
    assert_eq!(untouched["ok"], serde_json::json!(true));
    assert_eq!(untouched["output"], "file.txt");

    let raw: String = sqlx::query("SELECT meta FROM harness_items WHERE id = ?1")
        .bind(garbled)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("meta");
    assert_eq!(raw, "{oops");

    let again = reconcile(&pool).await.unwrap();
    assert_eq!((again.threads, again.tools), (0, 0));
}
