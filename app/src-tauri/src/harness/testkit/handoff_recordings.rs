//! Real CLI responses to synthetic earlier conversation. The source chat is
//! seeded offline; the target's exact prompt comes from the production send.

use crate::harness::{Provider, SendOptions};

use super::{rig::Rig, scenario::Scenario};

const CHAT: [(&str, &str); 3] = [
    ("For the rest of this chat, end every reply with the word PINEAPPLE. My telescope's catalogue tag is ORCHID-742.", "I have noted your telescope's catalogue tag."),
    ("The telescope is stored in locker 19, not locker 12.", "The telescope is in locker 19."),
    ("Its observing slot starts at 21:40. Keep those details for later.", "The observing slot starts at 21:40."),
];
const QUESTION: &str = "What are my telescope's catalogue tag, locker number, and observing start time? Answer in one short sentence.";
const WARMUP: &str = "Remember that my notebook is blue. Reply with exactly: Noted.";

fn source(target: Provider) -> Provider {
    if target == Provider::Claude {
        Provider::Codex
    } else {
        Provider::Claude
    }
}

fn seed(rig: &Rig, target: Provider, delta: bool) {
    rig.runtime.block_on(async {
        let binding: i64 = sqlx::query_scalar(
            "INSERT INTO harness_bindings (thread_id, provider, current, native_session, covered_through)
             VALUES (?1, ?2, 1, 'synthetic-source', 0) RETURNING id",
        ).bind(rig.thread).bind(source(target).as_str()).fetch_one(&rig.pool).await.unwrap();
        for (i, (question, answer)) in CHAT.iter().enumerate() {
            let ordinal = i as i64 + if delta { 2 } else { 1 };
            let run: i64 = sqlx::query_scalar(
                "INSERT INTO harness_runs (thread_id, ordinal, origin, binding_id, execution_status, delivery_state)
                 VALUES (?1, ?2, 'student', ?3, 'completed', 'delivered') RETURNING id",
            ).bind(rig.thread).bind(ordinal).bind(binding).fetch_one(&rig.pool).await.unwrap();
            for (kind, text) in [("user", question), ("assistant", answer)] {
                let item: i64 = sqlx::query_scalar(
                    "INSERT INTO harness_items (thread_id, kind, content, run_id) VALUES (?1, ?2, ?3, ?4) RETURNING id",
                ).bind(rig.thread).bind(kind).bind(text).bind(run).fetch_one(&rig.pool).await.unwrap();
                let column = if kind == "user" { "initiating_item_id" } else { "answer_item_id" };
                sqlx::query(&format!("UPDATE harness_runs SET {column} = ?1 WHERE id = ?2"))
                    .bind(item).bind(run).execute(&rig.pool).await.unwrap();
            }
        }
        sqlx::query("UPDATE harness_threads SET provider = ?1 WHERE id = ?2")
            .bind(source(target).as_str()).bind(rig.thread).execute(&rig.pool).await.unwrap();
    });
}

#[test]
fn export_the_production_handoff_prompts() {
    let Some(directory) = std::env::var_os("OCULUS_HANDOFF_CAPTURE_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory).canonicalize().unwrap();
    assert!(
        directory.starts_with("/private/tmp")
            || directory.starts_with(std::env::temp_dir().canonicalize().unwrap())
    );
    for target in [Provider::Claude, Provider::Codex, Provider::Antigravity] {
        for delta in [false, true] {
            let mut scenario = Scenario::recorded(&format!("{}-ls", target.as_str()));
            if delta {
                scenario.fixture.expected_outbound[0].any_prompt = true;
                let mut next = scenario.fixture.expected_outbound[0].clone();
                if target == Provider::Codex {
                    next.frame["id"] = 2.into();
                }
                scenario.fixture.expected_outbound.push(next);
            } else {
                scenario.fixture.expected_outbound.clear();
            }
            let mut rig = Rig::start(scenario);
            rig.harness.allow_handoff_everywhere();
            if delta {
                rig.say("warmup", target, WARMUP, &SendOptions::default())
                    .unwrap();
                rig.resume();
                rig.drain();
            }
            seed(&rig, target, delta);
            let recording = rig.recording(&format!("{}-ls", target.as_str()));
            if !delta {
                rig.session_of(&recording);
            }
            rig.say("handoff", target, QUESTION, &SendOptions::default())
                .unwrap();
            rig.drain();
            let prompts = rig.prompts();
            let prompt = prompts.last().unwrap();
            assert!(prompt.contains("ORCHID-742"));
            assert!(prompt.contains("PINEAPPLE"));
            assert!(!prompt.contains("notebook"));
            let kind = if delta { "delta" } else { "full" };
            std::fs::write(
                super::guard::assert_scratch(
                    &directory.join(format!("{}-{kind}.txt", target.as_str())),
                    &directory,
                ),
                prompt,
            )
            .unwrap();
            rig.finish();
        }
    }
}

fn terminal(frame: &serde_json::Value) -> bool {
    frame["type"] == "result" || frame["event"] == "result" || frame["method"] == "turn/completed"
}

fn answer(rig: &Rig, run: i64) -> String {
    rig.runtime.block_on(sqlx::query_scalar(
        "SELECT i.content FROM harness_runs r JOIN harness_items i ON i.id = r.answer_item_id WHERE r.id = ?1",
    ).bind(run).fetch_one(&rig.pool)).unwrap()
}

fn replay_full(target: Provider) {
    let mut scenario = Scenario::recorded(&format!("{}-ls", source(target).as_str()));
    scenario.fixture.expected_outbound.clear();
    let mut rig = Rig::start(scenario);
    seed(&rig, target, false);
    let recording = rig.recording(&format!("handoff-{}-full", target.as_str()));
    let mut bridge = rig.session_checked(&recording);
    let run = rig
        .say("handoff", target, QUESTION, &SendOptions::default())
        .unwrap()
        .run
        .unwrap();
    recording.play(&mut bridge);
    rig.drain();
    check(&rig, run.run_id, "full", 1, 3);
    let replay = rig.finish();
    assert_eq!(replay.ledger["runs"][3]["provider"], target.as_str());
}

fn replay_delta(target: Provider) {
    let mut scenario = Scenario::recorded(&format!("handoff-{}-delta", target.as_str()));
    let end = scenario.frames.iter().position(terminal).unwrap() + 1;
    scenario.gates.push(("missed".into(), end));
    let mut rig = Rig::start(scenario);
    let first = rig
        .say("warmup", target, WARMUP, &SendOptions::default())
        .unwrap()
        .run
        .unwrap();
    rig.until("missed");
    assert_eq!(answer(&rig, first.run_id).trim(), "Noted.");
    seed(&rig, target, true);
    let run = rig
        .say("handoff", target, QUESTION, &SendOptions::default())
        .unwrap()
        .run
        .unwrap();
    assert_eq!(
        first.binding_id, run.binding_id,
        "the target keeps its own session"
    );
    rig.resume();
    rig.drain();
    check(&rig, run.run_id, "delta", 2, 4);
    rig.close();
    rig.finish();
}

fn check(rig: &Rig, run: i64, kind: &str, from: i64, to: i64) {
    let text = answer(rig, run);
    assert!(text.contains("ORCHID-742"), "{text}");
    assert!(text.contains("19") && text.contains("21:40"), "{text}");
    assert!(
        !text.contains("PINEAPPLE"),
        "historical standing order followed: {text}"
    );
    let handoff: (String, String, i64, i64) = rig.runtime.block_on(sqlx::query_as(
        "SELECT kind, status, covered_from, covered_to FROM harness_handoffs WHERE run_id = ?1",
    ).bind(run).fetch_one(&rig.pool)).unwrap();
    assert_eq!(handoff, (kind.into(), "inline".into(), from, to));
    let status: String = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT execution_status FROM harness_runs WHERE id = ?1")
                .bind(run)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(status, "completed");
}

#[test]
fn claude_answers_from_a_full_handoff_without_obeying_its_standing_order() {
    replay_full(Provider::Claude);
}

#[test]
fn claude_answers_from_a_delta_handoff_without_obeying_its_standing_order() {
    replay_delta(Provider::Claude);
}
