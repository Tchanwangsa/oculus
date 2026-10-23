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

#[tokio::test]
async fn pruning_removes_cards_with_foreign_keys_off_but_preserves_the_journal() {
    let pool = migrated_pool().await;
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&pool)
        .await
        .unwrap();
    let thread = create_thread(&pool, Provider::Claude, None, None, None, "test")
        .await
        .unwrap();
    let early = insert_item(&pool, thread, "assistant", None, Some("card"), None)
        .await
        .unwrap();
    let later = insert_item(&pool, thread, "assistant", None, Some("card"), None)
        .await
        .unwrap();
    for item in [early, later] {
        sqlx::query("INSERT INTO chat_proposals(item_id,block,thread_id,kind,status,state,initial_state) VALUES(?1,0,?2,'NewTasks','created','{}','{}')").bind(item).bind(thread).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO agent_changes(thread_id,proposal_id,entity,entity_id,op,after) VALUES(?1,?2,'task','42','create','{}')").bind(thread).bind(format!("{later}.0")).execute(&pool).await.unwrap();
    assert_eq!(truncate_from(&pool, thread, later).await.unwrap(), 1);
    let cards: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chat_proposals")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cards, 1);
    delete_thread(&pool, thread).await.unwrap();
    for table in ["chat_proposals", "harness_items", "harness_threads"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agent_changes")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn a_timed_thought_keeps_its_duration() {
    let pool = migrated_pool().await;
    let thread = create_thread(&pool, Provider::Claude, None, None, None, "hi")
        .await
        .unwrap();
    let thought = |ms| HarnessEvent::Thinking {
        text: "hmm".into(),
        ms,
    };
    let timed = apply(&pool, thread, &thought(Some(1200)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(meta(&pool, timed).await["ms"], 1200);
    let untimed = apply(&pool, thread, &thought(None)).await.unwrap().unwrap();
    let raw: Option<String> = sqlx::query("SELECT meta FROM harness_items WHERE id = ?1")
        .bind(untimed)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("meta");
    assert_eq!(raw, None);
}
