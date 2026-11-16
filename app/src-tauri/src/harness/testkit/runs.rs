//! Stage 1 of the turn model: each send's run, attempt, binding and receipt.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use std::sync::{Arc, Mutex};

use crate::harness::app::consumer::{consume, ConsumerIo};
use crate::harness::app::{BusMsg, Envelope};
use crate::harness::core::effect::Effect;
use crate::harness::core::{Command, Kind, Outcome};
use crate::harness::{jobs, store, HarnessEvent, OwnTurns, Provider, SendOptions};

use super::guard::{assert_scratch, scratch};
use super::replay::run;
use super::scenario::Scenario;

#[test]
fn a_send_allocates_one_run_attempt_binding_and_receipt() {
    for name in ["claude-ls", "codex-ls", "opencode-v1-ls", "antigravity-ls"] {
        let replay = run(Scenario::recorded(name));
        let ledger = &replay.ledger;
        for table in ["runs", "attempts", "bindings", "receipts"] {
            assert_eq!(
                ledger[table].as_array().unwrap().len(),
                1,
                "{table}: {name}"
            );
        }
        let run = &ledger["runs"][0];
        assert_eq!(run["origin"], "student");
        assert_eq!(run["execution_status"], "completed", "{name}");
        assert_eq!(run["active_attempt_id"], ledger["attempts"][0]["id"]);
        assert_eq!(run["binding_id"], ledger["bindings"][0]["id"]);
        assert_eq!(ledger["receipts"][0]["run_id"], run["id"]);

        let question = &replay.items[0];
        assert_eq!(question["kind"], "user");
        assert_eq!(run["initiating_item_id"], question["id"]);
        assert_eq!(question["run_id"], run["id"]);
        // Every later row is the run's, with a distinct weak attempt-local id.
        let rest = &replay.items[1..];
        assert!(!rest.is_empty(), "{name}");
        let mut refs: Vec<&str> = rest
            .iter()
            .map(|i| {
                assert_eq!(i["run_id"], run["id"], "{name}: {i}");
                assert_eq!(i["attempt_id"], ledger["attempts"][0]["id"], "{name}");
                i["entity_ref"].as_str().unwrap()
            })
            .collect();
        assert!(refs.iter().all(|r| r.starts_with("weak:")));
        refs.dedup();
        assert_eq!(refs.len(), rest.len(), "{name}");
        // Envelopes name the run; the event payloads are unchanged.
        let finished = replay
            .envelopes
            .iter()
            .find(|e| e["event"]["type"] == "turn_finished")
            .unwrap();
        assert_eq!(finished["runId"], run["id"]);
        assert!(replay
            .envelopes
            .iter()
            .all(|e| e["event"].get("run").is_none()));
    }
}

#[test]
fn a_storage_failure_before_send_blocks_the_provider_write() {
    let mut scenario = Scenario::recorded("opencode-v1-text");
    scenario.fail_intent = true;
    let replay = run(scenario);
    assert!(replay.outbound.is_empty(), "no provider write");
    for table in ["runs", "attempts", "bindings", "receipts"] {
        assert!(
            replay.ledger[table].as_array().unwrap().is_empty(),
            "{table}"
        );
    }
    assert!(!replay.items.iter().any(|i| i["kind"] == "user"));
    // The replay still feeds the recorded inbound frames; the refusal is a row.
    assert!(replay.items.iter().any(|i| i["kind"] == "error"
        && i["content"]
            .as_str()
            .unwrap()
            .contains("replay intent failure")));
    assert!(!replay.busy, "the failed turn released the thread");
}

/// The scratch directory lives as long as the returned guard.
pub(super) async fn scratch_pool(name: &str) -> (crate::test_support::Scratch, SqlitePool) {
    let dir = scratch(name);
    let database = assert_scratch(&dir.join("oculus.db"), &dir);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(database)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    (dir, pool)
}

pub(super) fn intent(thread_id: i64, receipt: &str) -> store::SendIntent {
    store::SendIntent {
        thread_id,
        provider: Provider::Claude,
        model: None,
        effort: None,
        binding: None,
        receipt: receipt.into(),
        text: "question".into(),
        at: None,
        continuation: None,
        queued: None,
        options: None,
    }
}

/// A student's send as `harness_send` puts it on the bus, and where its
/// result comes back.
pub(super) fn send(
    thread_id: i64,
    provider: Provider,
    id: &str,
) -> (
    BusMsg,
    tokio::sync::oneshot::Receiver<Result<Outcome, String>>,
) {
    let (reply, answer) = tokio::sync::oneshot::channel();
    let command = Command {
        id: id.into(),
        thread_id: Some(thread_id),
        kind: Kind::Send {
            provider,
            text: "question".into(),
            options: SendOptions::default(),
            mode: crate::harness::manager::policy::Mode::Queue,
        },
    };
    (BusMsg::Command { command, reply }, answer)
}

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn a_repeated_receipt_starts_no_second_run() {
    let (_dir, pool) = scratch_pool("turn-receipt").await;
    let thread = store::create_thread(&pool, Provider::Claude, None, None, None, "q")
        .await
        .unwrap();
    let first = store::begin_run(&pool, &intent(thread, "r1"))
        .await
        .unwrap();
    let repeat = store::begin_run(&pool, &intent(thread, "r1")).await;
    assert!(repeat.unwrap_err().contains("already started"));
    assert_eq!(
        store::receipt_run(&pool, "r1").await.unwrap(),
        Some(first.run_id)
    );
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM harness_runs").await, 1);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM harness_attempts").await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM harness_items WHERE kind = 'user'"
        )
        .await,
        1
    );
    pool.close().await;
}

#[tokio::test]
async fn bindings_follow_the_native_session_and_rewinds_drop_their_runs() {
    let (_dir, pool) = scratch_pool("turn-bindings").await;
    let thread = store::create_thread(&pool, Provider::Claude, None, None, None, "q")
        .await
        .unwrap();
    let first = store::begin_run(&pool, &intent(thread, "a")).await.unwrap();
    store::bind_session(&pool, first.run_id, "s1")
        .await
        .unwrap();
    let second = store::begin_run(&pool, &intent(thread, "b")).await.unwrap();
    assert_eq!(second.binding_id, first.binding_id, "a resume reuses it");
    let current = |pool: SqlitePool| async move {
        store::bindings::find(&pool, thread, Provider::Claude)
            .await
            .unwrap()
            .unwrap()
    };
    assert_eq!(current(pool.clone()).await.id, first.binding_id);
    sqlx::query("UPDATE harness_bindings SET covered_through = 1 WHERE id = ?1")
        .bind(first.binding_id)
        .execute(&pool)
        .await
        .unwrap();
    // A provider that answers with another session starts a new binding,
    // which takes the old one's place as current and what it had covered.
    store::bind_session(&pool, second.run_id, "s2")
        .await
        .unwrap();
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM harness_bindings").await,
        2
    );
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM harness_bindings WHERE current = 1"
        )
        .await,
        1,
        "one current binding per chat and provider"
    );
    let moved = current(pool.clone()).await;
    assert_ne!(moved.id, first.binding_id);
    assert_eq!(moved.native_session.as_deref(), Some("s2"));
    assert_eq!(moved.covered_through, 1);
    let of_run = store::bindings::of_run(&pool, second.run_id).await.unwrap();
    assert_eq!(of_run.map(|binding| binding.id), Some(moved.id));
    // Another provider's binding in the same chat is its own, and current too.
    let mut conn = pool.acquire().await.unwrap();
    let other = store::bindings::current(&mut conn, thread, Provider::Codex)
        .await
        .unwrap();
    drop(conn);
    assert_eq!((other.native_session, other.covered_through), (None, 0));
    assert_eq!(current(pool.clone()).await.id, moved.id);
    assert_eq!(
        count(
            &pool,
            "SELECT COUNT(*) FROM harness_bindings WHERE current = 1"
        )
        .await,
        2
    );
    assert!(
        store::settle_run(&pool, first.run_id, first.attempt_id, "completed")
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store::settle_run(&pool, first.run_id, first.attempt_id, "failed")
            .await
            .unwrap()
            .is_none(),
        "a repeated terminal settles nothing"
    );
    let status: String =
        sqlx::query_scalar("SELECT execution_status FROM harness_runs WHERE id = ?1")
            .bind(first.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "completed", "a repeated terminal keeps the first");

    store::truncate_from(&pool, thread, second.item_id)
        .await
        .unwrap();
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM harness_runs").await, 1);
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM harness_command_receipts").await,
        1
    );
    assert_eq!(store::receipt_run(&pool, "b").await.unwrap(), None);
    store::delete_thread(&pool, thread).await.unwrap();
    for table in [
        "harness_runs",
        "harness_attempts",
        "harness_bindings",
        "harness_command_receipts",
    ] {
        assert_eq!(
            count(&pool, &format!("SELECT COUNT(*) FROM {table}")).await,
            0,
            "{table}"
        );
    }
    pool.close().await;
}

#[tokio::test]
async fn a_restart_interrupts_runs_left_open() {
    let (_dir, pool) = scratch_pool("turn-reconcile").await;
    let thread = store::create_thread(&pool, Provider::Claude, None, None, None, "q")
        .await
        .unwrap();
    let open = store::begin_run(&pool, &intent(thread, "a")).await.unwrap();
    // As the start step leaves it once the provider has the message.
    assert!(store::claim_start(&pool, &open).await.unwrap());
    store::mark_sent(&pool, &open).await.unwrap();
    crate::harness::app::startup::Startup::default()
        .ready(|| async { Ok(pool.clone()) })
        .await
        .unwrap();
    let status: String =
        sqlx::query_scalar("SELECT execution_status FROM harness_runs WHERE id = ?1")
            .bind(open.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "interrupted");
    pool.close().await;
}

/// Records envelopes; the ordering test needs no naming or dispatch.
struct Recorder {
    pool: SqlitePool,
    rows: Vec<Option<i64>>,
}

impl ConsumerIo for Recorder {
    fn open_pool(&mut self, _: &tokio::runtime::Runtime) -> Option<SqlitePool> {
        Some(self.pool.clone())
    }
    fn name(&mut self, _: i64, _: Provider, _: Vec<jobs::JobSelection>, _: store::NamingSeed) {
        panic!("no naming in this test");
    }
    fn perform(&mut self, _: Effect) {}
    fn emit(&mut self, envelope: Envelope) {
        self.rows.push(envelope.item_id);
    }
}

/// A row from the previous turn still on the bus is written before the next
/// question, so reload order (by id) is the order the timeline saw live.
#[test]
fn a_stray_row_on_the_bus_is_written_before_the_next_question() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (_dir, pool) = runtime.block_on(scratch_pool("turn-stray"));
    let thread = runtime
        .block_on(store::create_thread(
            &pool,
            Provider::Claude,
            None,
            None,
            None,
            "q",
        ))
        .unwrap();
    let (next, answer) = send(thread, Provider::Claude, "next");
    let bus = vec![
        BusMsg::app(
            thread,
            Provider::Claude,
            HarnessEvent::error("late from the previous turn"),
        ),
        next,
    ];
    let mut io = Recorder {
        pool: pool.clone(),
        rows: Vec::new(),
    };
    let queue = Arc::new(Mutex::new(OwnTurns::default()));
    std::thread::scope(|s| {
        s.spawn(|| consume(bus, queue, &mut io));
    });
    let run = runtime.block_on(answer).unwrap().unwrap().run.unwrap();
    let (stray, question) = (io.rows[0].unwrap(), io.rows[1].unwrap());
    assert_eq!(question, run.item_id);
    assert!(stray < question, "stray {stray}, question {question}");
    runtime.block_on(pool.close());
}

#[test]
fn bridges_name_their_messages_and_report_no_gap_on_clean_turns() {
    for name in [
        "claude-ls",
        "codex-ls",
        "opencode-v1-ls",
        "antigravity-ls",
        "claude-interrupt",
        "codex-interrupt",
        "opencode-v1-interrupt",
    ] {
        let replay = run(Scenario::recorded(name));
        assert!(
            replay.ledger["gaps"].as_array().unwrap().is_empty(),
            "{name}: {}",
            replay.ledger["gaps"]
        );
        for item in replay
            .items
            .iter()
            .filter(|i| i["kind"] == "assistant" || i["kind"] == "thinking")
        {
            assert!(item["source_ref"].is_string(), "{name}: {item}");
            assert!(item["lifecycle"].is_string(), "{name}: {item}");
            if item["kind"] == "assistant" {
                assert!(item["native_phase"].is_string(), "{name}: {item}");
            }
        }
    }
}

#[test]
fn a_stream_cut_off_without_a_stop_keeps_its_text_as_partial_with_a_gap() {
    // The turn completes, but the last message's `item/completed` is dropped.
    let mut scenario = Scenario::recorded("codex-ls");
    let close = scenario
        .frames
        .iter()
        .rposition(|v| {
            v["method"] == "item/completed" && v["params"]["item"]["type"] == "agentMessage"
        })
        .unwrap();
    scenario.frames.remove(close);
    let replay = run(scenario);
    let partial: Vec<_> = replay
        .items
        .iter()
        .filter(|i| i["lifecycle"] == "partial")
        .collect();
    assert_eq!(partial.len(), 1, "{:?}", replay.items);
    let gaps = replay.ledger["gaps"].as_array().unwrap();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0]["cause"], "missing_snapshot");
    assert_eq!(gaps[0]["source_ref"], partial[0]["source_ref"]);
}

/// A run with these assistant replies (text, native phase), then a gap if asked.
async fn settled(
    replies: &[(&str, Option<&str>)],
    gap: bool,
    status: &str,
) -> (
    Vec<i64>,
    Option<store::Settlement>,
    SqlitePool,
    crate::test_support::Scratch,
) {
    let (dir, pool) = scratch_pool("turn-settle").await;
    let thread = store::create_thread(&pool, Provider::Codex, None, None, None, "q")
        .await
        .unwrap();
    let run = store::begin_run(&pool, &intent(thread, "s")).await.unwrap();
    let mut ids = Vec::new();
    for (i, (text, phase)) in replies.iter().enumerate() {
        let id = sqlx::query(
            "INSERT INTO harness_items (thread_id, kind, content, run_id, attempt_id, source_ref, native_phase)
             VALUES (?1, 'assistant', ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(thread)
        .bind(text)
        .bind(run.run_id)
        .bind(run.attempt_id)
        .bind(format!("m{i}"))
        .bind(phase)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();
        ids.push(id);
    }
    if gap {
        sqlx::query("INSERT INTO harness_delivery_gaps (thread_id, run_id, attempt_id, cause) VALUES (?1, ?2, ?3, 'empty_narration')")
            .bind(thread)
            .bind(run.run_id)
            .bind(run.attempt_id)
            .execute(&pool)
            .await
            .unwrap();
    }
    let settlement = store::settle_run(&pool, run.run_id, run.attempt_id, status)
        .await
        .unwrap();
    (ids, settlement, pool, dir)
}

#[tokio::test]
async fn the_answer_is_the_native_final_else_the_last_reply_not_marked_commentary() {
    let (ids, s, ..) = settled(
        &[("answer", Some("final_answer")), ("ack", Some("unknown"))],
        false,
        "completed",
    )
    .await;
    let s = s.unwrap();
    assert_eq!(s.answer_item_id, Some(ids[0]), "a native final wins");
    assert_eq!(s.work_item_ids, vec![ids[1]]);
    assert_eq!(s.delivery, "delivered");

    // Length never decides: a short last reply is still the answer.
    let (ids, s, ..) = settled(
        &[("a long answer before a tool", None), ("Saved.", None)],
        false,
        "completed",
    )
    .await;
    assert_eq!(s.unwrap().answer_item_id, Some(ids[1]));

    let (ids, s, ..) = settled(
        &[("answer", None), ("narration", Some("commentary"))],
        false,
        "completed",
    )
    .await;
    assert_eq!(
        s.unwrap().answer_item_id,
        Some(ids[0]),
        "commentary is never the answer"
    );
}

#[tokio::test]
async fn missing_text_is_a_notice_never_a_pretend_answer() {
    let (_, s, ..) = settled(&[("planning", Some("commentary"))], false, "completed").await;
    let s = s.unwrap();
    assert_eq!(s.answer_item_id, None);
    assert_eq!(s.delivery, "incomplete");
    assert_eq!(s.notice.unwrap().1, store::NO_ANSWER);

    let (_, s, ..) = settled(&[("answer", None)], true, "completed").await;
    let s = s.unwrap();
    assert!(s.answer_item_id.is_some());
    assert_eq!(
        s.notice.unwrap().1,
        store::PART_MISSING,
        "a gap stays visible"
    );

    // A stop already says what happened: no notice on top of it.
    let (_, s, ..) = settled(&[("partial", None)], true, "interrupted").await;
    assert!(s.unwrap().notice.is_none());
}

#[tokio::test]
async fn naming_and_thread_status_follow_the_settled_run() {
    let (_, _, pool, _dir) = settled(
        &[("answer", Some("final_answer")), ("ack", Some("unknown"))],
        false,
        "completed",
    )
    .await;
    let status: String = sqlx::query_scalar("SELECT status FROM harness_threads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "idle", "settlement ends the thread's turn");
    let thread: i64 = sqlx::query_scalar("SELECT id FROM harness_threads")
        .fetch_one(&pool)
        .await
        .unwrap();
    let seed = store::claim_naming(&pool, thread).await.unwrap().unwrap();
    assert_eq!(
        seed.reply, "answer",
        "the selected answer, not the last reply"
    );

    let (_, _, pool, _dir) = settled(&[("partial", None)], false, "failed").await;
    let status: String = sqlx::query_scalar("SELECT status FROM harness_threads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "error");
}
