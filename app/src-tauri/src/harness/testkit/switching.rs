//! Changing provider mid-chat: what each run records of its selection, the
//! one current binding per provider, the transition a send makes and the
//! context handoff its first prompt carries.

use sqlx::Row;

use crate::harness::core::load;
use crate::harness::{store, Provider, SendOptions};

use super::rig::Rig;
use super::scenario::Scenario;

fn options(model: &str, effort: Option<&str>) -> SendOptions {
    SendOptions {
        model: Some(model.into()),
        reasoning_effort: effort.map(String::from),
        ..SendOptions::default()
    }
}

/// A run keeps the model and reasoning level it was sent with and the
/// model its CLI reported; the thread keeps the student's pick.
#[test]
fn a_run_records_what_it_was_sent_with_and_what_its_cli_reported() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    let run = rig
        .send_with("replay-send", &options("sonnet", Some("high")))
        .unwrap();
    rig.resume();
    rig.close();
    rig.drain();
    let row = rig
        .runtime
        .block_on(
            sqlx::query("SELECT model, effort, resolved_model FROM harness_runs WHERE id = ?1")
                .bind(run.run_id)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(
        row.get::<Option<String>, _>("model").as_deref(),
        Some("sonnet")
    );
    assert_eq!(
        row.get::<Option<String>, _>("effort").as_deref(),
        Some("high")
    );
    let reported: Option<String> = row.get("resolved_model");
    let reported = reported.expect("the recording's init line names its model");
    assert_ne!(reported, "sonnet");
    let thread_model: Option<String> = rig
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT model FROM harness_threads WHERE id = ?1")
                .bind(rig.thread)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(
        thread_model.as_deref(),
        Some("sonnet"),
        "the student's pick"
    );

    // A reload reads the same off the run.
    let snapshot = rig
        .runtime
        .block_on(load::read(&rig.pool, rig.thread))
        .unwrap();
    let loaded = serde_json::to_value(&snapshot.runs[0]).unwrap();
    assert_eq!(loaded["provider"], "claude");
    assert_eq!(loaded["model"], "sonnet");
    assert_eq!(loaded["effort"], "high");
    assert_eq!(loaded["resolved_model"], reported.as_str());

    // And a live view is told once, under the run.
    let told = rig.envelopes();
    let selected: Vec<_> = told
        .iter()
        .filter(|e| e["event"]["type"] == "run_selected")
        .collect();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0]["runId"], run.run_id);
    assert_eq!(selected[0]["event"]["provider"], "claude");
    assert_eq!(selected[0]["event"]["model"], "sonnet");
    assert_eq!(selected[0]["event"]["effort"], "high");
    rig.finish();
}

fn status_of(rig: &Rig, run_id: i64) -> String {
    let status = sqlx::query_scalar("SELECT execution_status FROM harness_runs WHERE id = ?1")
        .bind(run_id)
        .fetch_one(&rig.pool);
    rig.runtime.block_on(status).unwrap()
}

/// The chat's selection, as its row holds it.
fn selection(rig: &Rig) -> (String, Option<String>, Option<String>) {
    let row = sqlx::query_as("SELECT provider, model, effort FROM harness_threads WHERE id = ?1")
        .bind(rig.thread)
        .fetch_one(&rig.pool);
    rig.runtime.block_on(row).unwrap()
}

fn count(rig: &Rig, sql: &str) -> i64 {
    rig.runtime
        .block_on(sqlx::query_scalar(sql).fetch_one(&rig.pool))
        .unwrap()
}

/// Claude takes its model at spawn: another model ends the process and a
/// new one resumes the same conversation. The provider did not change, so
/// the prompt is the message alone (the recorded frame, matched exactly).
#[test]
fn a_model_only_change_on_claude_respawns_and_resumes_without_a_handoff() {
    let mut scenario = Scenario::recorded("claude-ls");
    let again = scenario.fixture.expected_outbound[0].clone();
    scenario.fixture.expected_outbound.push(again);
    let mut rig = Rig::start(scenario);
    let first = rig.send_with("replay-send", &options("one", None)).unwrap();
    rig.resume();
    rig.drain();
    assert_eq!(status_of(&rig, first.run_id), "completed");

    let replaced = rig.next_session();
    assert!(replaced.handle.is_alive());
    let second = rig.send_with("second", &options("two", None)).unwrap();
    assert!(!replaced.handle.is_alive(), "the old process was killed");
    assert_eq!(rig.harness.live_threads(rig.provider), vec![rig.thread]);
    assert!(rig.alive(), "its successor took the message");
    // The same binding, so the same native session is what it resumed.
    assert_eq!(second.binding_id, first.binding_id);
    let binding = rig
        .runtime
        .block_on(store::bindings::of_run(&rig.pool, second.run_id))
        .unwrap()
        .unwrap();
    assert!(binding.native_session.is_some(), "the session to resume");
    assert_eq!(count(&rig, "SELECT COUNT(*) FROM harness_handoffs"), 0);
    assert_eq!(selection(&rig).1.as_deref(), Some("two"));
    rig.finish();
}

/// Codex names the model with each turn: another model goes out in the
/// next `turn/start` of the same thread. No session is registered for a
/// respawn, so one would fail the replay.
#[test]
fn a_model_change_on_codex_is_told_with_the_next_turn_of_the_same_session() {
    let mut scenario = Scenario::recorded("codex-ls");
    let mut next = scenario.fixture.expected_outbound[0].clone();
    next.frame["id"] = 2.into();
    next.frame["params"]["model"] = "two".into();
    scenario.fixture.expected_outbound.push(next);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.resume();
    rig.drain();
    assert!(!rig.busy());

    let second = rig.send_with("second", &options("two", None)).unwrap();
    assert_eq!(status_of(&rig, second.run_id), "running");
    assert!(rig.alive(), "the same session");
    assert_eq!(rig.harness.live_threads(rig.provider), vec![rig.thread]);
    assert_eq!(count(&rig, "SELECT COUNT(*) FROM harness_bindings"), 1);
    rig.finish();
}

/// A pick is saved at once, mid-turn too, and told to every view; the run
/// being answered keeps what it was sent with.
#[test]
fn a_pick_is_stored_mid_turn_and_the_running_run_keeps_its_own() {
    let scenario = Scenario::recorded("claude-ls").gate("answer", |f| f["type"] == "assistant");
    let mut rig = Rig::start(scenario);
    let run = rig
        .send_with("replay-send", &options("one", Some("low")))
        .unwrap();
    rig.until("answer");
    assert!(rig.busy());

    rig.select("pick", Provider::Claude, Some("two"), Some("high"))
        .unwrap();
    assert_eq!(
        selection(&rig),
        ("claude".into(), Some("two".into()), Some("high".into()))
    );
    let sent_with: (Option<String>, Option<String>) = rig
        .runtime
        .block_on(
            sqlx::query_as("SELECT model, effort FROM harness_runs WHERE id = ?1")
                .bind(run.run_id)
                .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(sent_with, (Some("one".into()), Some("low".into())));
    let told = rig.envelopes();
    let selected: Vec<_> = told
        .iter()
        .filter(|e| e["event"]["type"] == "selected")
        .collect();
    // The send that started the chat's first run told its own selection.
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0]["event"]["model"], "one");
    assert_eq!(selected[1]["event"]["provider"], "claude");
    assert_eq!(selected[1]["event"]["model"], "two");
    assert_eq!(selected[1]["event"]["effort"], "high");
    // The same command again is the same pick, told once.
    rig.select("pick", Provider::Claude, Some("two"), Some("high"))
        .unwrap();
    let again = rig.envelopes();
    assert_eq!(
        again
            .iter()
            .filter(|e| e["event"]["type"] == "selected")
            .count(),
        2
    );
    rig.resume();
    rig.close();
    rig.drain();
    assert_eq!(status_of(&rig, run.run_id), "completed");
    rig.finish();
}

/// No bridge takes a handoff without its recording, so another provider
/// can neither be picked for a started chat nor sent to: the refusal says
/// to start a new chat, and nothing is written.
#[test]
fn a_bridge_with_no_handoff_recording_cannot_take_over_a_started_chat() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    rig.open();
    rig.resume();
    rig.drain();
    let before = count(&rig, "SELECT COUNT(*) FROM harness_items");

    let refusal = "opencode cannot take over a chat that has started. Start a new chat to switch.";
    let picked = rig.select("pick", Provider::Opencode, Some("m"), None);
    assert_eq!(picked.err().as_deref(), Some(refusal));
    let sent = rig.say("send", Provider::Opencode, "hello", &options("m", None));
    assert_eq!(sent.err().as_deref(), Some(refusal));
    assert_eq!(selection(&rig).0, "claude");
    assert_eq!(count(&rig, "SELECT COUNT(*) FROM harness_items"), before);
    assert_eq!(count(&rig, "SELECT COUNT(*) FROM harness_runs"), 1);
    assert!(!rig.busy());
    // The refusal recorded nothing: the same id works once it can.
    rig.harness.allow_handoff_everywhere();
    rig.select("pick", Provider::Opencode, Some("m"), None)
        .unwrap();
    assert_eq!(selection(&rig).0, "opencode");
    rig.finish();
}

/// Ending Claude's process would kill a task it runs in the background, so
/// the chat cannot leave Claude while one is alive; once it has finished
/// and Claude's own turn about it is over, the switch is taken.
#[test]
fn a_switch_away_from_claude_is_refused_while_its_background_task_is_alive() {
    let scenario = Scenario::recorded("claude-background-agent").gate("task-done", |f| {
        f["subtype"] == "task_notification" && f["tool_use_id"] == "toolu_01RHoJhb1LDywxCL6b82JR7C"
    });
    let mut rig = Rig::start(scenario);
    rig.harness.allow_handoff_everywhere();
    rig.open();
    rig.until("task-done");
    assert!(!rig.busy(), "the turn that launched the task is over");

    let refusal = "Claude Code is still running a background task in this chat. Wait for it to finish, or stop it, before switching.";
    let picked = rig.select("pick", Provider::Codex, Some("gpt"), None);
    assert_eq!(picked.err().as_deref(), Some(refusal));
    let sent = rig.say("send", Provider::Codex, "hello", &options("gpt", None));
    assert_eq!(sent.err().as_deref(), Some(refusal));
    assert_eq!(selection(&rig).0, "claude");
    // Its own model can still be picked while the task runs.
    rig.select("own", Provider::Claude, Some("sonnet"), None)
        .unwrap();

    rig.resume();
    rig.close();
    rig.drain();
    assert!(!rig.busy());
    rig.select("pick", Provider::Codex, Some("gpt"), None)
        .unwrap();
    assert_eq!(selection(&rig).0, "codex");
    rig.finish();
}
