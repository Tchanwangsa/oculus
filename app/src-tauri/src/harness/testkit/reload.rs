//! A reload mid-answer: one read returns the rows, the text streamed so
//! far and the revision that covers both, and the envelopes say on which
//! side of that revision they fall. Checked at every pause of a real
//! recording, as a webview that reloaded there would compute it.

use serde_json::Value;

use crate::harness::core::load;

use super::rig::Rig;
use super::scenario::Scenario;

/// What a webview shows as the streaming answer after loading `snapshot`
/// and folding `envelopes`: the draft, then every delta told at or above
/// the snapshot's revision, cleared by a later row that takes the text.
fn streaming_after_reload(snapshot: &load::Snapshot, envelopes: &[Value]) -> String {
    let mut text = snapshot
        .draft
        .as_ref()
        .map(|d| d.text.clone())
        .unwrap_or_default();
    for envelope in envelopes {
        let revision = envelope["revision"].as_i64().unwrap();
        match envelope["event"]["type"].as_str().unwrap() {
            "assistant_delta" if revision >= snapshot.revision => {
                text.push_str(envelope["event"]["text"].as_str().unwrap());
            }
            "assistant_message" | "tool_started" | "turn_finished"
                if revision > snapshot.revision =>
            {
                text.clear();
            }
            _ => {}
        }
    }
    text
}

/// What a webview that was there from the start shows as the streaming
/// answer: the deltas since the last row that took the text.
fn streaming_live(envelopes: &[Value]) -> String {
    let mut text = String::new();
    for envelope in envelopes {
        match envelope["event"]["type"].as_str().unwrap() {
            "assistant_delta" => text.push_str(envelope["event"]["text"].as_str().unwrap()),
            "assistant_message" | "tool_started" | "turn_finished" => text.clear(),
            _ => {}
        }
    }
    text
}

/// The rows a webview holds after loading `snapshot` and folding
/// `envelopes`: the snapshot's, then each row told above its revision,
/// once.
fn rows_after_reload(snapshot: &load::Snapshot, envelopes: &[Value]) -> Vec<i64> {
    let mut rows: Vec<i64> = snapshot.items.iter().map(|i| i.id).collect();
    for envelope in envelopes {
        let revision = envelope["revision"].as_i64().unwrap();
        if let Some(item) = envelope["itemId"].as_i64() {
            if revision > snapshot.revision && !rows.contains(&item) {
                rows.push(item);
            }
        }
    }
    rows
}

/// At every frame of a recording: reload there, then let the rest of the
/// turn arrive. The streamed text is what a webview that never reloaded
/// shows, at the reload and at the end, and every row is there once.
#[test]
fn a_reload_at_any_point_of_an_answer_shows_its_text_and_each_row_once() {
    for name in [
        "claude-ls",
        "codex-ls",
        "opencode-v1-text",
        "antigravity-acp-ls",
    ] {
        let scenario = Scenario::recorded(name);
        let frames = scenario.frames.clone();
        let mut rig = Rig::start(scenario);
        rig.open();
        // (how many envelopes had been told, the snapshot read then)
        let mut reloads = Vec::new();
        let mut streamed_mid_answer = 0;
        for frame in &frames {
            rig.feed(frame);
            rig.drain();
            let told = rig.envelopes();
            let snapshot = rig
                .runtime
                .block_on(load::read(&rig.pool, rig.thread))
                .unwrap();
            let live = streaming_live(&told);
            assert_eq!(
                streaming_after_reload(&snapshot, &[]),
                live,
                "{name}: a reload after {} envelopes shows the text streamed so far",
                told.len()
            );
            if !live.is_empty() {
                streamed_mid_answer += 1;
            }
            reloads.push((told.len(), snapshot));
        }
        assert!(
            streamed_mid_answer > 0,
            "{name}: the recording streams text"
        );
        rig.close();
        let replay = rig.finish();
        let stored: Vec<i64> = replay
            .items
            .iter()
            .map(|i| i["id"].as_i64().unwrap())
            .collect();
        for (told, snapshot) in &reloads {
            let after = &replay.envelopes[*told..];
            let mut rows = rows_after_reload(snapshot, after);
            rows.sort_unstable();
            assert_eq!(rows, stored, "{name}: reloaded after {told} envelopes");
            assert_eq!(
                streaming_after_reload(snapshot, after),
                "",
                "{name}: nothing is left streaming once the turn is over"
            );
            // Nothing the snapshot holds is told again above its revision.
            for envelope in after {
                let (Some(item), Some(revision)) =
                    (envelope["itemId"].as_i64(), envelope["revision"].as_i64())
                else {
                    continue;
                };
                let inserted = !snapshot.items.iter().any(|i| i.id == item);
                assert!(
                    inserted || revision > snapshot.revision || is_update(envelope),
                    "{name}: row {item} was in the snapshot and told again at {revision}"
                );
            }
        }
    }
}

/// An envelope that names a row it changed in place (the same provider
/// message again), which a webview replaces by id.
fn is_update(envelope: &Value) -> bool {
    matches!(
        envelope["event"]["type"].as_str(),
        Some("assistant_message" | "thinking" | "user_message" | "continued")
    )
}
