use super::*;
use crate::harness::core::{commit, decide, state, Command, Kind};
use crate::harness::{Provider, SendOptions};

async fn pool() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for m in crate::db::migrations::all() {
        sqlx::raw_sql(m.sql).execute(&pool).await.unwrap();
    }
    pool
}

async fn command(pool: &SqlitePool, thread: i64, id: &str, kind: Kind) -> commit::Committed {
    let command = Command {
        id: id.into(),
        thread_id: Some(thread),
        kind,
    };
    let state = state::read(pool, &command, state::Live::default())
        .await
        .unwrap();
    let decide::Decision::Commit(plan) = decide::decide(&state, &command, None).unwrap() else {
        panic!("completion never asks a provider")
    };
    commit::run(
        pool,
        Some(commit::Receipt {
            id,
            command: command.kind.name(),
        }),
        Some(thread),
        plan,
    )
    .await
    .unwrap()
}

async fn completed(pool: &SqlitePool, thread: i64) -> Option<String> {
    sqlx::query_scalar("SELECT completed_at FROM harness_threads WHERE id = ?1")
        .bind(thread)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn complete_and_reopen_only_change_display_state_and_reopen_survives_sweep() {
    let pool = pool().await;
    sqlx::raw_sql("INSERT INTO harness_threads(id, provider, provider_session_id, updated_at) VALUES (1, 'claude', 'keep-me', '2000-01-01')")
        .execute(&pool).await.unwrap();
    let done = command(&pool, 1, "complete", Kind::CompleteThread).await;
    assert!(completed(&pool, 1).await.is_some());
    assert!(matches!(
        done.applied.as_slice(),
        [commit::Applied::Completion]
    ));
    let (session, updated): (String, String) =
        sqlx::query_as("SELECT provider_session_id, updated_at FROM harness_threads WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(session, "keep-me");
    assert_eq!(updated, "2000-01-01");
    command(&pool, 1, "reopen", Kind::ReopenThread).await;
    assert_eq!(completed(&pool, 1).await, None);
    assert!(sweep(&pool).await.unwrap().is_empty());
    let session: String =
        sqlx::query_scalar("SELECT provider_session_id FROM harness_threads WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(session, "keep-me");
}

#[tokio::test]
async fn a_send_reopens_in_the_same_transaction_for_start_and_queue_and_rolls_back_on_failure() {
    let pool = pool().await;
    for thread in 1..=3 {
        crate::harness::store::create_thread(&pool, Provider::Claude, None, None, None, "old")
            .await
            .unwrap();
        sqlx::query("UPDATE harness_threads SET completed_at = '2000-01-01' WHERE id = ?1")
            .bind(thread)
            .execute(&pool)
            .await
            .unwrap();
    }
    let send = || Kind::Send {
        provider: Provider::Claude,
        text: "new message".into(),
        options: SendOptions::default(),
        mode: Default::default(),
    };
    let started = command(&pool, 1, "start", send()).await;
    assert!(started.outcome.run.is_some());
    assert_eq!(completed(&pool, 1).await, None);
    sqlx::raw_sql("INSERT INTO harness_bindings(id, thread_id, provider) VALUES (20, 2, 'claude');
        INSERT INTO harness_runs(id, thread_id, ordinal, origin, binding_id, execution_status) VALUES (20, 2, 1, 'student', 20, 'running')")
        .execute(&pool).await.unwrap();
    let queued = command(&pool, 2, "queue", send()).await;
    assert!(queued.outcome.queued.is_some());
    assert_eq!(completed(&pool, 2).await, None);
    // The reopen must roll back if the rest of the send cannot commit.
    sqlx::raw_sql("CREATE TRIGGER refuse_run BEFORE INSERT ON harness_runs BEGIN SELECT RAISE(ABORT, 'disk refused'); END;")
        .execute(&pool).await.unwrap();
    let command = Command {
        id: "failed".into(),
        thread_id: Some(3),
        kind: send(),
    };
    let state = state::read(&pool, &command, state::Live::default())
        .await
        .unwrap();
    let decide::Decision::Commit(plan) = decide::decide(&state, &command, None).unwrap() else {
        panic!()
    };
    assert!(commit::run(&pool, None, Some(3), plan).await.is_err());
    assert!(completed(&pool, 3).await.is_some());
}

#[tokio::test]
async fn sweep_skips_running_starting_queued_pending_questions_failed_and_recent_threads() {
    let pool = pool().await;
    for id in 1..=12 {
        sqlx::query("INSERT INTO harness_threads(id, provider, status, updated_at) VALUES (?1, 'claude', 'idle', '2000-01-01')")
            .bind(id).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("UPDATE harness_threads SET status = 'running' WHERE id = 2;
        UPDATE harness_threads SET status = 'error' WHERE id = 3;
        UPDATE harness_threads SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now','-2 days') WHERE id = 4;
        UPDATE harness_threads SET completed_at = 'already' WHERE id = 5;
        INSERT INTO harness_bindings(id, thread_id, provider) SELECT id, id, 'claude' FROM harness_threads WHERE id >= 6;
        INSERT INTO harness_runs(id, thread_id, ordinal, origin, binding_id, execution_status)
          VALUES (6,6,1,'student',6,'starting'), (7,7,1,'student',7,'running'),
                 (8,8,1,'student',8,'queued'), (9,9,1,'student',9,'completed'),
                 (10,10,1,'student',10,'completed'), (11,11,1,'student',11,'failed');
        INSERT INTO harness_attempts(id,run_id,ordinal,reason,status) VALUES (9,9,1,'initial','completed'), (10,10,1,'initial','completed');
        INSERT INTO harness_items(id,thread_id,kind) VALUES (9,9,'request'), (10,10,'request');
        INSERT INTO harness_requests(thread_id,run_id,attempt_id,binding_id,item_id,provider,kind,action,payload,options,responder,generation,native_ref)
          VALUES (9,9,9,9,9,'claude','approval','command','{}','[]','live',1,'a'),
                 (10,10,10,10,10,'claude','question','question','{}','[]','message',1,'q');")
        .execute(&pool).await.unwrap();
    let mut ids = sweep(&pool).await.unwrap();
    ids.sort();
    assert_eq!(ids, vec![1, 12]);
    assert!(sweep(&pool).await.unwrap().is_empty());
    let updated: String = sqlx::query_scalar("SELECT updated_at FROM harness_threads WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(updated, "2000-01-01");
}

#[tokio::test]
async fn commit_rechecks_completion_if_a_sweep_lands_after_the_send_decision() {
    let pool = pool().await;
    crate::harness::store::create_thread(&pool, Provider::Claude, None, None, None, "old")
        .await
        .unwrap();
    for queued in [false, true] {
        // A plan made while open has no SetCompleted change.
        let change = if queued {
            commit::Change::Enqueue {
                text: "new".into(),
                options: SendOptions::default(),
                front: false,
            }
        } else {
            commit::Change::Begin {
                text: "new".into(),
                options: SendOptions::default(),
                receipt: "race".into(),
                queued: None,
            }
        };
        sqlx::query("UPDATE harness_threads SET completed_at = '2000-01-01', updated_at = '2000-01-01' WHERE id = 1").execute(&pool).await.unwrap();
        let plan = decide::Plan {
            changes: vec![change],
            text: None,
        };
        let committed = commit::run(&pool, None, Some(1), plan).await.unwrap();
        assert_eq!(completed(&pool, 1).await, None);
        assert!(committed
            .applied
            .iter()
            .any(|a| matches!(a, commit::Applied::Completion)));
    }
}
