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

const CLAUDE_SAID: &str = "This directory holds data for what looks like the \"oculus\" app";
const CODEX_SAID: &str = "This directory contains agent, canvas session, course, lecture";

/// Each handoff of the chat: its run, kind, status, how many messages were
/// left out and the run ordinals it stands for.
fn handoffs(rig: &Rig) -> Vec<(i64, String, String, i64, i64, i64)> {
    let rows = sqlx::query_as(
        "SELECT run_id, kind, status, omitted, covered_from, covered_to FROM harness_handoffs
          WHERE thread_id = ?1 ORDER BY id",
    )
    .bind(rig.thread)
    .fetch_all(&rig.pool);
    rig.runtime.block_on(rows).unwrap()
}

/// The provider's current binding in the chat: its id, session and coverage.
fn binding(rig: &Rig, provider: Provider) -> store::Binding {
    let found = store::bindings::find(&rig.pool, rig.thread, provider);
    rig.runtime
        .block_on(found)
        .unwrap()
        .expect("a current binding")
}

/// The block a prompt opens with, and the message after it.
fn split(prompt: &str) -> (&str, &str) {
    let (block, message) = prompt
        .split_once("</earlier-conversation>\n\nThe student's new message:\n")
        .unwrap_or_else(|| panic!("no handoff block in: {prompt}"));
    assert!(block.starts_with("<earlier-conversation from=\"Oculus\">\n"));
    (block, message)
}

fn told_handoffs(rig: &Rig, run_id: i64) -> Vec<(String, String)> {
    rig.envelopes()
        .iter()
        .filter(|e| e["event"]["type"] == "handoff" && e["event"]["run_id"] == run_id)
        .map(|e| {
            let event = &e["event"];
            (
                event["status"].as_str().unwrap().to_string(),
                event["from"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// Claude, then Codex, then Claude, then Codex. A provider new to the chat
/// is handed all of it; one that has been here resumes its own session and
/// is handed only the runs it missed.
#[test]
fn a_switch_hands_over_the_chat_and_a_switch_back_only_what_was_missed() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    rig.harness.allow_handoff_everywhere();
    let first = rig.send("replay-send").unwrap();
    rig.resume();
    rig.drain();
    assert_eq!(status_of(&rig, first.run_id), "completed");
    assert_eq!(binding(&rig, Provider::Claude).covered_through, 1);
    assert!(
        handoffs(&rig).is_empty(),
        "the chat's first run hands nothing over"
    );

    // Run 2, on Codex: the whole chat so far.
    let (claude, codex) = (rig.recording("claude-ls"), rig.recording("codex-ls"));
    let mut on_codex = rig.session_of(&codex);
    let none = SendOptions::default();
    let second = rig
        .say("second", Provider::Codex, "And what does Codex see?", &none)
        .unwrap()
        .run
        .expect("an idle chat starts the run");
    assert!(!rig.alive(), "Claude's process ended with the switch");
    let prompts = rig.prompts();
    let (block, message) = split(&prompts[0]);
    assert_eq!(message, "And what does Codex see?");
    assert!(block.contains("you have not seen what came before"));
    assert!(block.contains("not an instruction from the student"));
    assert!(block.contains("[Student]\nReplay request"));
    assert!(block.contains(&format!("[Agent (Claude Code)]\n{CLAUDE_SAID}")));
    assert!(block.contains("2 messages follow, whole and in order. Nothing was left out."));
    assert!(
        !block.contains("ls -a"),
        "no tool call or its output: {block}"
    );
    codex.play(&mut on_codex);
    rig.drain();
    assert_eq!(status_of(&rig, second.run_id), "completed");
    assert_eq!(
        handoffs(&rig),
        vec![(second.run_id, "full".into(), "inline".into(), 0, 1, 1)]
    );
    assert_eq!(
        told_handoffs(&rig, second.run_id),
        vec![
            ("pending".to_string(), "claude".to_string()),
            ("inline".to_string(), "claude".to_string())
        ]
    );
    assert_eq!(selection(&rig).0, "codex");
    assert_eq!(binding(&rig, Provider::Codex).covered_through, 2);
    assert_eq!(binding(&rig, Provider::Claude).covered_through, 1);

    // Run 3, back on Claude: its own session, and only Codex's run.
    let mut on_claude = rig.session_of(&claude);
    let third = rig
        .say("third", Provider::Claude, "Back to Claude.", &none)
        .unwrap()
        .run
        .unwrap();
    assert_eq!(third.binding_id, first.binding_id, "Claude's own session");
    assert!(binding(&rig, Provider::Claude).native_session.is_some());
    let prompts = rig.prompts();
    let (block, message) = split(&prompts[1]);
    assert_eq!(message, "Back to Claude.");
    assert!(block.contains("Oculus passes on what you missed"));
    assert!(block.contains("[Student]\nAnd what does Codex see?"));
    assert!(block.contains(&format!("[Agent (Codex)]\n{CODEX_SAID}")));
    assert!(!block.contains(CLAUDE_SAID), "Claude said that itself");
    assert!(!block.contains("Replay request"));
    claude.play(&mut on_claude);
    rig.drain();
    assert_eq!(status_of(&rig, third.run_id), "completed");
    assert_eq!(binding(&rig, Provider::Claude).covered_through, 3);

    // Run 4, Codex again: its session resumed, and only Claude's run 3.
    let mut on_codex = rig.session_of(&codex);
    let fourth = rig
        .say("fourth", Provider::Codex, "And Codex once more.", &none)
        .unwrap()
        .run
        .unwrap();
    assert_eq!(fourth.binding_id, second.binding_id, "Codex's own session");
    let prompts = rig.prompts();
    let (block, message) = split(&prompts[2]);
    assert_eq!(message, "And Codex once more.");
    assert!(block.contains("[Student]\nBack to Claude."));
    assert!(block.contains(&format!("[Agent (Claude Code)]\n{CLAUDE_SAID}")));
    assert!(!block.contains("And what does Codex see?"));
    assert!(!block.contains(CODEX_SAID));
    codex.play(&mut on_codex);
    rig.drain();
    assert_eq!(status_of(&rig, fourth.run_id), "completed");
    let kinds: Vec<(String, String, i64, i64)> = handoffs(&rig)
        .into_iter()
        .map(|(_, kind, status, _, from, to)| (kind, status, from, to))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("full".into(), "inline".into(), 1, 1),
            ("delta".into(), "inline".into(), 2, 2),
            ("delta".into(), "inline".into(), 3, 3),
        ]
    );
    assert_eq!(
        count(&rig, "SELECT COUNT(*) FROM harness_bindings"),
        2,
        "one binding per provider, each current"
    );
    // A reload reads each run's provider and its handoff.
    let snapshot = rig
        .runtime
        .block_on(load::read(&rig.pool, rig.thread))
        .unwrap();
    let runs = serde_json::to_value(&snapshot.runs).unwrap();
    let providers: Vec<&str> = runs
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["provider"].as_str().unwrap())
        .collect();
    assert_eq!(providers, ["claude", "codex", "claude", "codex"]);
    assert!(runs[0]["handoff"].is_null());
    assert_eq!(runs[1]["handoff"]["kind"], "full");
    assert_eq!(runs[1]["handoff"]["from"], "claude");
    assert_eq!(runs[2]["handoff"]["from"], "codex");
    assert_eq!(runs[2]["handoff"]["status"], "inline");
    rig.finish();
}

/// A chat too long for the budget: the student's first message and the
/// newest exchange go, the middle is dropped whole, and the block says how
/// many messages that was.
#[test]
fn a_handoff_over_budget_drops_the_middle_and_says_how_many() {
    // The chat's own provider sends nothing in this replay.
    let mut scenario = Scenario::recorded("claude-ls");
    scenario.fixture.expected_outbound.clear();
    let mut rig = Rig::start(scenario);
    rig.harness.allow_handoff_everywhere();
    // Forty earlier exchanges, as a chat from before runs holds them.
    for i in 0..40 {
        for (kind, who) in [("user", "question"), ("assistant", "answer")] {
            let text = format!("{who} {i} {}", "z".repeat(500));
            let row = sqlx::query(
                "INSERT INTO harness_items (thread_id, kind, content) VALUES (?1, ?2, ?3)",
            )
            .bind(rig.thread)
            .bind(kind)
            .bind(text)
            .execute(&rig.pool);
            rig.runtime.block_on(row).unwrap();
        }
    }
    let codex = rig.recording("codex-ls");
    let _on_codex = rig.session_of(&codex);
    let run = rig
        .say(
            "send",
            Provider::Codex,
            "Carry on.",
            &SendOptions::default(),
        )
        .unwrap()
        .run
        .unwrap();
    rig.drain();
    let prompts = rig.prompts();
    let (block, message) = split(&prompts[0]);
    assert_eq!(message, "Carry on.");
    assert!(
        block.contains("[Student]\nquestion 0 "),
        "the first message"
    );
    assert!(block.contains("answer 39 ") && block.contains("question 39 "));
    assert!(!block.contains("question 12 "), "the middle is dropped");
    let (_, kind, status, omitted, ..) = handoffs(&rig).remove(0);
    assert_eq!((kind.as_str(), status.as_str()), ("full", "inline"));
    assert!(omitted > 40, "{omitted} of 80 left out");
    assert!(block.contains(&format!("{omitted} other messages were left out to fit.")));
    let bytes = count(&rig, "SELECT bytes FROM harness_handoffs");
    assert!(bytes <= 16_000 && bytes > 12_000, "{bytes}");
    let told = rig.envelopes();
    let told = told
        .iter()
        .find(|e| e["event"]["type"] == "handoff")
        .unwrap();
    assert_eq!(told["event"]["omitted"], omitted);
    assert_eq!(told["event"]["run_id"], run.run_id);
    rig.finish();
}

/// A message so long that not even the block's opening lines fit beside it
/// in a new session: the send fails where the student sees it, and the
/// provider is never called.
#[test]
fn a_handoff_that_cannot_fit_fails_the_send_before_the_provider_hears_it() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    rig.harness.allow_handoff_everywhere();
    rig.open();
    rig.resume();
    rig.drain();
    let huge = "x".repeat(200_000);
    // No Codex session is registered: bringing one up would fail the replay.
    let run = rig
        .say("send", Provider::Codex, &huge, &SendOptions::default())
        .unwrap()
        .run
        .unwrap();
    rig.drain();
    assert_eq!(status_of(&rig, run.run_id), "failed");
    let (_, kind, status, ..) = handoffs(&rig).remove(0);
    assert_eq!((kind.as_str(), status.as_str()), ("full", "failed"));
    assert_eq!(told_handoffs(&rig, run.run_id)[0].0, "failed");
    let error: String = rig
        .runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT content FROM harness_items WHERE kind = 'error' AND run_id = ?1",
            )
            .bind(run.run_id)
            .fetch_one(&rig.pool),
        )
        .unwrap();
    assert!(error.starts_with("The earlier conversation does not fit beside this message"));
    assert!(rig.prompts().is_empty());
    assert_eq!(binding(&rig, Provider::Codex).covered_through, 0);
    rig.finish();
}

/// The app dies after the start step took the run and before anyone knew
/// whether the provider had the prompt. That session's contents are not
/// known, so the next send starts a new one and hands it the whole chat.
#[test]
fn a_crash_between_pending_and_inline_forces_a_fresh_session_with_the_whole_chat() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    rig.harness.allow_handoff_everywhere();
    rig.open();
    rig.resume();
    rig.drain();
    // Codex has been here once, so it has a session to resume.
    let codex = rig.recording("codex-ls");
    let mut on_codex = rig.session_of(&codex);
    let none = SendOptions::default();
    let second = rig
        .say("second", Provider::Codex, "On Codex.", &none)
        .unwrap()
        .run
        .unwrap();
    codex.play(&mut on_codex);
    rig.drain();
    let claude = rig.recording("claude-ls");
    let mut on_claude = rig.session_of(&claude);
    rig.say("third", Provider::Claude, "On Claude.", &none)
        .unwrap();
    claude.play(&mut on_claude);
    rig.drain();
    assert!(!rig.busy());

    // Run 4 for Codex is written with its delta and taken by its start
    // step; then the app is gone.
    rig.hold_effects();
    let fourth = rig
        .say("fourth", Provider::Codex, "Lost on the way.", &none)
        .unwrap()
        .run
        .unwrap();
    assert_eq!(fourth.binding_id, second.binding_id);
    let taken = rig.runtime.block_on(store::claim_start(&rig.pool, &fourth));
    assert!(taken.unwrap());
    let mut rig = rig.restart();
    rig.harness.allow_handoff_everywhere();
    rig.startup().unwrap();
    rig.drain();
    assert_eq!(status_of(&rig, fourth.run_id), "interrupted");
    let pending: Vec<_> = handoffs(&rig)
        .into_iter()
        .filter(|(run, ..)| *run == fourth.run_id)
        .collect();
    assert_eq!(pending[0].2, "pending", "nobody knows whether it arrived");

    let mut on_codex = rig.session_of(&codex);
    let fifth = rig
        .say("fifth", Provider::Codex, "Again.", &none)
        .unwrap()
        .run
        .unwrap();
    assert_ne!(fifth.binding_id, second.binding_id, "a fresh session");
    let fresh = binding(&rig, Provider::Codex);
    assert_eq!(fresh.id, fifth.binding_id);
    assert_eq!(fresh.native_session, None, "nothing to resume");
    let prompts = rig.prompts();
    let (block, message) = split(prompts.last().unwrap());
    assert_eq!(message, "Again.");
    assert!(block.contains("you have not seen what came before"));
    for said in [
        "[Student]\nReplay request",
        "[Student]\nOn Codex.",
        "[Student]\nOn Claude.",
        "[Student]\nLost on the way.",
    ] {
        assert!(block.contains(said), "{said}");
    }
    let last = handoffs(&rig).pop().unwrap();
    assert_eq!((last.0, last.1.as_str()), (fifth.run_id, "full"));
    codex.play(&mut on_codex);
    rig.drain();
    assert_eq!(status_of(&rig, fifth.run_id), "completed");
    assert_eq!(handoffs(&rig).pop().unwrap().2, "inline");
    rig.finish();
}

/// The app dies before the start step took the run: the provider heard
/// nothing, the message goes back to the queue and its handoff is planned
/// again when it starts, into the same session.
#[test]
fn a_crash_before_the_start_step_leaves_no_handoff_behind() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    rig.harness.allow_handoff_everywhere();
    rig.open();
    rig.resume();
    rig.drain();
    rig.hold_effects();
    let none = SendOptions::default();
    let second = rig
        .say("second", Provider::Codex, "On Codex.", &none)
        .unwrap()
        .run
        .unwrap();
    assert_eq!(handoffs(&rig).len(), 1);
    let mut rig = rig.restart();
    rig.harness.allow_handoff_everywhere();
    rig.startup().unwrap();
    rig.drain();
    assert_eq!(status_of(&rig, second.run_id), "queued");
    assert!(handoffs(&rig).is_empty());

    let codex = rig.recording("codex-ls");
    let mut on_codex = rig.session_of(&codex);
    rig.command(crate::harness::core::Command {
        id: "resume".into(),
        thread_id: Some(rig.thread),
        kind: crate::harness::core::Kind::Resume,
    })
    .unwrap();
    rig.drain();
    assert_eq!(status_of(&rig, second.run_id), "running");
    let (block, message) = {
        let prompts = rig.prompts();
        let (block, message) = split(&prompts[0]);
        (block.to_string(), message.to_string())
    };
    assert_eq!(message, "On Codex.");
    assert!(block.contains("[Student]\nReplay request"));
    codex.play(&mut on_codex);
    rig.drain();
    assert_eq!(binding(&rig, Provider::Codex).id, second.binding_id);
    assert_eq!(handoffs(&rig)[0].2, "inline");
    rig.finish();
}

/// A message typed while Claude answers waits; the student then picks
/// Codex. When its turn comes it goes out under the chat's selection as it
/// stands then: on Codex, with the chat handed over.
#[test]
fn a_message_queued_before_a_switch_runs_on_the_new_provider() {
    let scenario = Scenario::recorded("claude-ls").gate("answer", |f| f["type"] == "assistant");
    let mut rig = Rig::start(scenario);
    rig.harness.allow_handoff_everywhere();
    rig.open();
    rig.until("answer");
    let none = SendOptions::default();
    let waiting = rig
        .say("second", Provider::Claude, "Typed for Claude.", &none)
        .unwrap()
        .queued
        .expect("a busy chat queues the message");
    rig.select("pick", Provider::Codex, None, None).unwrap();
    assert_eq!(status_of(&rig, waiting), "queued");

    let codex = rig.recording("codex-ls");
    let mut on_codex = rig.session_of(&codex);
    rig.resume();
    rig.drain();
    assert_eq!(status_of(&rig, waiting), "running");
    let provider: String = rig
        .runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT b.provider FROM harness_runs r JOIN harness_bindings b ON b.id = r.binding_id
                  WHERE r.id = ?1",
            )
            .bind(waiting)
            .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(provider, "codex");
    let prompts = rig.prompts();
    let (block, message) = split(&prompts[0]);
    assert_eq!(message, "Typed for Claude.");
    assert!(block.contains(&format!("[Agent (Claude Code)]\n{CLAUDE_SAID}")));
    let selected = rig.envelopes();
    let selected = selected
        .iter()
        .rfind(|e| e["event"]["type"] == "run_selected")
        .unwrap();
    assert_eq!(selected["event"]["provider"], "codex");
    assert_eq!(selected["runId"], waiting);
    codex.play(&mut on_codex);
    rig.drain();
    assert_eq!(status_of(&rig, waiting), "completed");
    rig.finish();
}

/// A run that never reaches its provider (here, stopped before its start
/// step took it) delivered nothing: its handoff is marked failed, and the
/// next run hands the same history over again, with the lost question.
#[test]
fn a_send_the_provider_never_heard_leaves_its_handoff_undelivered() {
    let mut rig = Rig::start(Scenario::recorded("claude-ls"));
    rig.harness.allow_handoff_everywhere();
    rig.open();
    rig.resume();
    rig.drain();
    let none = SendOptions::default();
    rig.hold_effects();
    let lost = rig
        .say("second", Provider::Codex, "Never arrives.", &none)
        .unwrap()
        .run
        .unwrap();
    // The run is withdrawn before its start step: stopped while starting.
    rig.command(crate::harness::core::Command {
        id: "stop".into(),
        thread_id: Some(rig.thread),
        kind: crate::harness::core::Kind::Stop {
            run_id: Some(lost.run_id),
        },
    })
    .unwrap();
    rig.run_effects();
    assert_eq!(status_of(&rig, lost.run_id), "interrupted");
    assert_eq!(handoffs(&rig)[0].2, "failed", "it was never sent");
    assert_eq!(
        told_handoffs(&rig, lost.run_id)
            .iter()
            .map(|(status, _)| status.as_str())
            .collect::<Vec<_>>(),
        ["pending", "failed"]
    );
    assert!(rig.prompts().is_empty());
    assert_eq!(binding(&rig, Provider::Codex).covered_through, 0);

    let codex = rig.recording("codex-ls");
    let _on_codex = rig.session_of(&codex);
    rig.say("third", Provider::Codex, "Now it does.", &none)
        .unwrap();
    rig.run_effects();
    let prompts = rig.prompts();
    let (block, message) = split(&prompts[0]);
    assert_eq!(message, "Now it does.");
    assert!(block.contains("[Student]\nReplay request"));
    assert!(block.contains("[Student]\nNever arrives."));
    assert_eq!(handoffs(&rig)[1].2, "inline");
    rig.finish();
}
