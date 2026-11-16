use serde_json::Value;

use super::replay::Replay;

pub(super) fn stable(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .map(|v| {
            let mut v = v.clone();
            let object = v.as_object_mut().unwrap();
            object.remove("created_at");
            object.remove("anchor");
            // The timeline folds by run; attempts and sources are Rust's.
            for column in [
                "attempt_id",
                "entity_ref",
                "source_ref",
                "native_phase",
                "lifecycle",
            ] {
                object.remove(column);
            }
            // The role is compared: live settlement must match the stored one.
            if object.get("display_role").is_some_and(Value::is_null) {
                object.remove("display_role");
            }
            if v["content"].is_null() {
                v["content"] = "".into();
            }
            if let Some(meta) = v["meta"].as_str() {
                v["meta"] = serde_json::from_str(meta).unwrap();
            }
            v
        })
        .collect()
}

pub(super) fn check(name: &str, replay: &Replay, projection: &Value) -> bool {
    let items = &replay.items;
    // After a restart the app's only view is a reload.
    let view = if replay.restarted {
        &projection["reopened"]
    } else {
        &projection["rows"]
    };
    let visible = |kind: &str| {
        view.as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "item" && r["item"]["kind"] == kind)
            .count()
    };
    let messages = || items.iter().filter(|i| i["kind"] == "assistant").count();
    match name {
        "snapshot_race" => {
            stable(&replay.reopened) == stable(projection["racedItems"].as_array().unwrap())
        }
        "artifact_state_stable" => projection["artifactStateStable"] == true,
        "committed_cursor" => {
            let cursors: Vec<_> = replay
                .envelopes
                .iter()
                .filter_map(|e| e["sequence"].as_i64())
                .collect();
            cursors.len() == replay.envelopes.len() && cursors.windows(2).all(|w| w[0] < w[1])
        }
        "autonomous_origin" => {
            items.iter().filter(|i| i["kind"] == "user").count() == 1
                && items.iter().filter(|i| i["kind"] == "unprompted").count() == 1
        }
        "no_send_after_failed_intent" => replay.outbound.is_empty(),
        "durable_receipt" => {
            let receipts = replay.ledger["receipts"].as_array().unwrap();
            receipts.len() == 1 && receipts[0]["run_id"] == replay.ledger["runs"][0]["id"]
        }
        "one_answer" => visible("assistant") == 1,
        "two_messages" => messages() == 2,
        "one_message" => messages() == 1,
        // A gap is stored for its run (stage 2); stage 3 shows it.
        "gap_survives_reload" => {
            replay.ledger["gaps"].as_array().unwrap().iter().any(|g| {
                g["run_id"] == replay.ledger["runs"][0]["id"] && g["state"] == "unresolved"
            })
        }
        "honest_outcome" => {
            ["interrupted", "error", "delivery_notice", "compaction"]
                .iter()
                .map(|k| visible(k))
                .sum::<usize>()
                == 1
        }
        "run_before_send" => {
            replay.schema.iter().any(|t| t == "harness_runs")
                && items.iter().any(|i| i["run_id"].is_number())
        }
        "scoped_identity" => replay
            .envelopes
            .iter()
            .filter(|e| e["event"]["type"] == "assistant_message")
            .all(|e| {
                !e["runId"].is_null() && !e["attemptId"].is_null() && !e["entityId"].is_null()
            }),
        "answer_ref" => {
            items
                .iter()
                .filter(|i| i["display_role"] == "answer")
                .count()
                == 1
        }
        // A plan is a work row, never an assistant message.
        "phase_and_plan" => {
            let plan = |i: &&Value| {
                i["kind"] == "tool"
                    && i["meta"]
                        .as_str()
                        .and_then(|m| serde_json::from_str::<Value>(m).ok())
                        .is_some_and(|m| m["kind"] == "plan")
            };
            items.iter().any(|i| i["native_phase"] == "final_answer")
                && items.iter().any(|i| i["native_phase"] == "commentary")
                && items.iter().filter(plan).count() == 1
                && !items
                    .iter()
                    .any(|i| i["kind"] == "assistant" && i["content"] == "Plan mutation")
        }
        "no_reasoning_as_answer" => !replay.envelopes.iter().any(|e| {
            e["event"]["type"] == "assistant_message"
                && e["event"]["text"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("The user is asking"))
        }),
        "live_reload" => stable(items) == stable(projection["items"].as_array().unwrap()),
        "settled" => !replay.busy,
        "one_settlement" => {
            replay
                .envelopes
                .iter()
                .filter(|e| e["event"]["type"] == "turn_finished")
                .count()
                == 1
        }
        "public_text_durable" => replay
            .envelopes
            .iter()
            .filter(|e| e["event"]["type"] == "assistant_delta")
            .all(|e| {
                let text = e["event"]["text"].as_str().unwrap();
                items
                    .iter()
                    .any(|i| i["content"].as_str().is_some_and(|s| s.contains(text)))
            }),
        "artifact_source_visible" => projection["rows"].as_array().unwrap().iter().any(|r| {
            (r["kind"] == "item" || r["kind"] == "artifacts")
                && r["item"]["content"]
                    .as_str()
                    .is_some_and(|s| s.contains("```widget") && s.contains("```openui"))
        }),
        // A reply with fences folds as work, and its fences draw once, outside.
        "card_outside_fold" => {
            let rows = view.as_array().unwrap();
            let carded = |r: &&Value| {
                r["item"]["content"]
                    .as_str()
                    .is_some_and(|s| s.contains("```openui"))
            };
            let folded = rows.iter().any(|r| {
                r["kind"] == "work"
                    && r["items"].as_array().unwrap().iter().any(|i| {
                        i["content"]
                            .as_str()
                            .is_some_and(|s| s.contains("```openui"))
                    })
            });
            folded
                && rows
                    .iter()
                    .filter(|r| r["kind"] == "artifacts")
                    .filter(carded)
                    .count()
                    == 1
                && !rows
                    .iter()
                    .filter(|r| r["kind"] == "item")
                    .any(|r| carded(&r))
        }
        // The run's one answer is the one reply outside its fold; every other
        // reply is in the fold.
        "answer_outside_fold" => {
            let rows = view.as_array().unwrap();
            let answer = &replay.ledger["runs"][0]["answer_item_id"];
            let outside: Vec<_> = rows
                .iter()
                .filter(|r| r["kind"] == "item" && r["item"]["kind"] == "assistant")
                .collect();
            let folded = rows
                .iter()
                .filter(|r| r["kind"] == "work")
                .flat_map(|r| r["items"].as_array().unwrap())
                .filter(|i| i["kind"] == "assistant")
                .count();
            outside.len() == 1
                && outside[0]["item"]["id"] == *answer
                && outside[0]["role"] != "provisional"
                && folded + 1 == messages()
        }
        "one_compaction" => visible("compaction") == 1 && visible("assistant") == 0,
        "narration_is_work" => items
            .iter()
            .any(|i| i["kind"] == "thinking" && i["content"] == "Public summary mutation"),
        _ => panic!("unregistered release assertion: {name}"),
    }
}
