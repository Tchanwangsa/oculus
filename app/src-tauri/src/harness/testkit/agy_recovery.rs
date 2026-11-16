//! Derived setup traffic: an unloadable saved session, then a new session
//! carrying the same transcript handoff as a provider switch.
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::harness::antigravity::acp::{replay::opening_bridge, OpenError};
use crate::harness::{Provider, SendError, SendOptions};

use super::{outbound::Expected, rig::Rig, scenario::Scenario};

fn expected(id: i64, method: &str, params: Value, response: Value, prompt: bool) -> Expected {
    Expected {
        frame: json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        response,
        error: None,
        any_prompt: prompt,
    }
}

#[test]
fn a_failed_load_starts_new_with_one_full_handoff_and_the_next_send_uses_its_id() {
    let init = || {
        expected(
            1,
            "initialize",
            json!({
                "protocolVersion":1,
                "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false,"elicitation":{"form":{}}},
                "clientInfo":{"name":"oculus","title":"Oculus","version":env!("CARGO_PKG_VERSION")},
            }),
            json!({"agentInfo":{"version":"1.5.9"}}),
            false,
        )
    };
    let mut scenario = Scenario::recorded("antigravity-acp-ls");
    scenario.frames.clear();
    scenario.fixture.expected_outbound = vec![
        init(),
        expected(
            2,
            "session/load",
            json!({"cwd":"$SCRATCH/agents","mcpServers":[],"sessionId":"old-print-id"}),
            json!({"error":{"code":-32603,"message":"conversation not found"}}),
            false,
        ),
        init(),
        expected(
            2,
            "session/new",
            json!({"cwd":"$SCRATCH/agents","mcpServers":[]}),
            json!({"sessionId":"fresh-acp-id"}),
            false,
        ),
        expected(
            3,
            "session/set_mode",
            json!({"sessionId":"fresh-acp-id","modeId":"default"}),
            json!({}),
            false,
        ),
        expected(
            4,
            "session/prompt",
            json!({"sessionId":"fresh-acp-id","prompt":[{"type":"text","text":"Replay request"}]}),
            Value::Null,
            true,
        ),
        expected(
            5,
            "session/prompt",
            json!({"sessionId":"fresh-acp-id","prompt":[{"type":"text","text":"Replay request"}]}),
            Value::Null,
            true,
        ),
    ];
    let mut rig = Rig::start(scenario);
    rig.runtime.block_on(async {
        let binding: i64 = sqlx::query_scalar("INSERT INTO harness_bindings (thread_id,provider,current,native_session,covered_through) VALUES (?1,'antigravity',1,'old-print-id',1) RETURNING id")
            .bind(rig.thread).fetch_one(&rig.pool).await.unwrap();
        let old: i64 = sqlx::query_scalar("INSERT INTO harness_runs (thread_id,ordinal,origin,binding_id,execution_status,delivery_state) VALUES (?1,1,'student',?2,'completed','delivered') RETURNING id")
            .bind(rig.thread).bind(binding).fetch_one(&rig.pool).await.unwrap();
        for (kind, text, column) in [("user","Remember locker 19","initiating_item_id"),("assistant","Your locker is 19","answer_item_id")] {
            let item: i64 = sqlx::query_scalar("INSERT INTO harness_items (thread_id,kind,content,run_id) VALUES (?1,?2,?3,?4) RETURNING id")
                .bind(rig.thread).bind(kind).bind(text).bind(old).fetch_one(&rig.pool).await.unwrap();
            sqlx::query(&format!("UPDATE harness_runs SET {column} = ?2 WHERE id = ?1")).bind(old).bind(item).execute(&rig.pool).await.unwrap();
        }
    });
    let bridge: Arc<Mutex<Option<crate::harness::manager::Handle>>> = Arc::default();
    let (harness, thread, outbound, root, slot) = (
        Arc::downgrade(&rig.harness),
        rig.thread,
        rig.outbound.clone(),
        rig.scratch.to_path_buf(),
        bridge.clone(),
    );
    rig.harness
        .replay_spawn_result(thread, Provider::Antigravity, move |sink| {
            let error = match opening_bridge(outbound.clone(), sink, Some("old-print-id"), &root) {
                Err(OpenError::Load(e)) => e,
                _ => panic!("the real load must fail before any prompt"),
            };
            harness.upgrade().unwrap().replay_spawn_result(
                thread,
                Provider::Antigravity,
                move |sink| {
                    let next = opening_bridge(outbound, sink, None, &root)
                        .map_err(|e| SendError::Unsent(e.to_string()))?;
                    let handle = next.handle.clone();
                    *slot.lock().unwrap() = Some(handle.clone());
                    Ok(handle)
                },
            );
            Err(SendError::ResumeFailed(error))
        });
    rig.say(
        "recover",
        Provider::Antigravity,
        "Which locker?",
        &SendOptions::default(),
    )
    .unwrap();
    rig.drain();
    let prompt = rig.prompts().remove(0);
    assert!(prompt.contains("<earlier-conversation"));
    assert!(prompt.contains("Remember locker 19"));
    assert!(prompt.contains("Your locker is 19"));
    assert_eq!(prompt.matches("Which locker?").count(), 1);
    let reply = |id| json!({"id":id,"result":{"stopReason":"end_turn"}});
    let feed = |frame: Value| {
        let slot = bridge.lock().unwrap();
        let crate::harness::manager::Handle::Antigravity(session) = slot.as_ref().unwrap() else {
            panic!()
        };
        session.inbound(&frame);
    };
    feed(reply(4));
    rig.drain();
    rig.say(
        "next",
        Provider::Antigravity,
        "Thank you",
        &SendOptions::default(),
    )
    .unwrap();
    rig.drain();
    assert_eq!(rig.prompts(), [prompt, "Thank you".into()]);
    feed(reply(5));
    rig.drain();
    let current: String = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT native_session FROM harness_bindings WHERE current = 1")
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(current, "fresh-acp-id");
    let handoffs: i64 = rig
        .runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT count(*) FROM harness_handoffs WHERE kind='full' AND status='inline'",
            )
            .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(handoffs, 1);
    let notes: i64 = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT count(*) FROM harness_items WHERE kind='session_restarted'")
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(notes, 1);
    rig.drain();
    let replay = rig.finish();
    assert_eq!(replay.items, replay.reopened);
}

#[test]
fn a_stop_or_delete_before_recovery_does_not_replace_the_binding() {
    for delete in [false, true] {
        let mut scenario = Scenario::recorded("antigravity-acp-ls");
        scenario.frames.clear();
        scenario.fixture.expected_outbound.clear();
        let mut rig = Rig::start(scenario);
        rig.hold_effects();
        rig.open();
        let run = rig.runtime.block_on(async {
            let (run_id, attempt_id, binding_id, item_id): (i64, i64, i64, i64) = sqlx::query_as(
                "SELECT id,active_attempt_id,binding_id,initiating_item_id FROM harness_runs",
            )
            .fetch_one(&rig.pool)
            .await
            .unwrap();
            let run = crate::harness::store::RunStart {
                run_id,
                attempt_id,
                binding_id,
                item_id,
            };
            assert!(crate::harness::store::claim_start(&rig.pool, &run)
                .await
                .unwrap());
            sqlx::query(if delete {
                "DELETE FROM harness_runs WHERE id = ?1"
            } else {
                "UPDATE harness_runs SET stop_requested_at = datetime('now') WHERE id = ?1"
            })
            .bind(run_id)
            .execute(&rig.pool)
            .await
            .unwrap();
            run
        });
        assert!(!rig
            .runtime
            .block_on(crate::harness::handoff::store::restart(
                &rig.pool, &run, "next"
            ))
            .unwrap());
        let count: i64 = rig
            .runtime
            .block_on(
                sqlx::query_scalar("SELECT count(*) FROM harness_bindings").fetch_one(&rig.pool),
            )
            .unwrap();
        assert_eq!(count, 1);
        rig.finish();
    }
}
