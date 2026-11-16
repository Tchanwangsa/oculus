//! Native steering against a fake bridge: the steer row inside the running
//! run, the dispatch re-checks, the missed-steer rule, "Send now", Stop and
//! requests around a steer, and the queue claim a steer never lets go.

use serde_json::json;

use crate::harness::core::decide::steer as refusals;
use crate::harness::core::Kind;
use crate::harness::manager::policy::Mode;
use crate::harness::requests::{Action, Decision, LiveRoute, NewRequest, RequestOption};
use crate::harness::{HarnessEvent, Steered};

use super::super::commands::Bench;
use super::super::requests::{ask, Fake};
use super::bench::{accept, delivery, end, queue, reply, report, running, runs, say, steer, told};

#[test]
fn a_steer_is_a_row_inside_the_running_run_and_the_run_settles_once_with_one_answer() {
    let (mut bench, run) = running("steer-row", true, false);
    reply(&mut bench, "Working on it.");
    let written = steer(&mut bench, "steer-1", "Use metres, not feet.");
    // The row is the run's, in its attempt; the message is a run that
    // never runs, and no ordinal was taken.
    let (kind, run_id, attempt_id, meta): (String, i64, i64, String) = bench
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT kind, run_id, attempt_id, meta FROM harness_items WHERE id = ?1",
            )
            .bind(written.item_id)
            .fetch_one(&bench.pool),
        )
        .unwrap();
    assert_eq!(
        (kind.as_str(), run_id, attempt_id),
        ("steer", run.run_id, run.attempt_id)
    );
    let meta: serde_json::Value = serde_json::from_str(&meta).unwrap();
    assert_eq!(meta["message"], written.message);
    assert_eq!(meta["delivery"], "sending");
    assert_eq!(
        runs(&bench),
        [
            (run.run_id, "running".into(), 1),
            (written.message, "steered".into(), -1)
        ]
    );
    let steers = bench.steers();
    assert_eq!(steers.len(), 1);
    assert_eq!(steers[0].text, "Use metres, not feet.");
    let envelope = told(&bench, "steer").pop().unwrap();
    assert_eq!(envelope["itemId"], written.item_id);
    assert_eq!(envelope["runId"], run.run_id);
    assert_eq!(envelope["event"]["message"], written.message);
    // The provider takes it, then echoes it.
    report(&mut bench, written, Steered::Accepted);
    assert_eq!(
        delivery(&bench, written.item_id).as_deref(),
        Some("written")
    );
    let client = steers[0].client.clone();
    let echo = bench.session(HarnessEvent::SteerEchoed { client_id: client });
    bench.play(vec![echo]);
    assert_eq!(
        delivery(&bench, written.item_id).as_deref(),
        Some("delivered")
    );
    let updates: Vec<_> = told(&bench, "steer_updated")
        .into_iter()
        .map(|e| e["event"]["delivery"].clone())
        .collect();
    assert_eq!(updates, [json!("written"), json!("delivered")]);
    // A steer never lets the run go: it is still open, and only its own
    // terminal settles it.
    assert!(bench.open());
    reply(&mut bench, "In metres: 3.");
    end(&mut bench, "1", "completed");
    assert!(!bench.open());
    let settled = told(&bench, "run_settled");
    assert_eq!(settled.len(), 1, "one settlement");
    let answer: String = bench
        .runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT content FROM harness_items WHERE id =
                   (SELECT answer_item_id FROM harness_runs WHERE id = ?1)",
            )
            .bind(run.run_id)
            .fetch_one(&bench.pool),
        )
        .unwrap();
    assert_eq!(answer, "In metres: 3.", "one answer, after the correction");
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_runs WHERE ordinal > 0"),
        1
    );
    // The order the timeline reads: question, early reply, steer, answer.
    let kinds: Vec<String> = bench
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT kind FROM harness_items ORDER BY id").fetch_all(&bench.pool),
        )
        .unwrap();
    assert_eq!(kinds, ["user", "assistant", "steer", "assistant"]);
    // A reload lists only the run that ran.
    let snapshot = bench
        .runtime
        .block_on(crate::harness::core::load::read(&bench.pool, bench.thread))
        .unwrap();
    assert_eq!(snapshot.runs.len(), 1);
}

#[test]
fn a_steer_that_arrives_after_the_turn_finished_starts_as_its_own_run_once() {
    let (mut bench, run) = running("steer-late", true, false);
    let written = steer(&mut bench, "late", "And in inches?");
    // The turn ends before the steer is written: the provider has no turn
    // to take it into.
    reply(&mut bench, "3 metres.");
    end(&mut bench, "1", "completed");
    bench.clear();
    report(&mut bench, written, Steered::Missed);
    assert_eq!(delivery(&bench, written.item_id), None, "its row is gone");
    let updated = told(&bench, "steer_updated");
    assert_eq!(updated[0]["event"]["delivery"], "requeued");
    // The same message identity becomes the next run, at once, once.
    let starts = bench.starts();
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].run.run_id, written.message);
    assert_eq!(starts[0].text, "And in inches?");
    assert_eq!(
        runs(&bench),
        [
            (run.run_id, "completed".into(), 1),
            (written.message, "starting".into(), 2)
        ]
    );
    assert_eq!(
        bench.questions(),
        ["question", "And in inches?"],
        "a question row of its own, once"
    );
    // Reported again (a repeated bus message): nothing more starts.
    report(&mut bench, written, Steered::Missed);
    assert_eq!(bench.starts().len(), 1);
}

#[test]
fn a_steer_the_provider_declines_waits_first_in_the_queue_behind_the_running_turn() {
    let (mut bench, run) = running("steer-declined", true, false);
    say(&mut bench, "later", "afterwards", Mode::Queue).unwrap();
    let written = steer(&mut bench, "now", "correction");
    report(&mut bench, written, Steered::Declined("busy".into()));
    assert_eq!(
        queue(&bench),
        [("correction".into(), false), ("afterwards".into(), false)],
        "first in line, ahead of what already waited"
    );
    // The turn still has the chat: nothing starts until it ends.
    assert!(bench.starts().len() == 1 && bench.open());
    reply(&mut bench, "done");
    end(&mut bench, "1", "completed");
    let starts = bench.starts();
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[1].run.run_id, written.message);
    assert_eq!(starts[1].text, "correction");
    assert_ne!(starts[1].run.run_id, run.run_id);
}

#[test]
fn a_finished_target_is_a_normal_start_and_a_run_not_yet_taken_queues() {
    let mut bench = Bench::new("steer-recheck");
    bench.steer_by(true, false);
    // Nothing running: an explicit steer is an ordinary start.
    let outcome = say(&mut bench, "idle", "hello", Mode::Steer).unwrap();
    let first = outcome.run.expect("a start");
    assert!(outcome.steered.is_none());
    // Saved, not yet taken by its provider: no turn to write into.
    let waits = say(&mut bench, "early", "wait", Mode::Steer).unwrap();
    assert!(waits.queued.is_some() && waits.steered.is_none());
    // Taken: now a send steers.
    accept(&mut bench, first, "1");
    let steered = say(&mut bench, "in", "now", Mode::Auto).unwrap();
    assert!(steered.steered.is_some());
    assert!(bench.steers().len() == 1);
}

#[test]
fn a_send_during_stop_a_slash_command_and_an_explicit_queue_never_steer() {
    let (mut bench, run) = running("steer-never", true, false);
    say(&mut bench, "cmd", "/compact", Mode::Steer).unwrap();
    say(&mut bench, "q", "for later", Mode::Queue).unwrap();
    let (stop, got) = bench.command(
        "stop",
        Kind::Stop {
            run_id: Some(run.run_id),
        },
    );
    bench.play(vec![stop]);
    bench.got(got).unwrap();
    let during = say(&mut bench, "auto", "while stopping", Mode::Auto).unwrap();
    assert!(during.queued.is_some());
    assert!(bench.steers().is_empty(), "nothing went into the turn");
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_items WHERE kind = 'steer'"),
        0
    );
}

#[test]
fn a_steer_into_a_turn_the_cli_started_itself_queues() {
    let mut bench = Bench::new("steer-unprompted");
    bench.steer_by(true, false);
    let own = bench.session(HarnessEvent::UnpromptedTurn { after: Vec::new() });
    let started = bench.session(HarnessEvent::TurnStarted {
        turn: Some("7".into()),
    });
    bench.play(vec![own, started]);
    let outcome = say(&mut bench, "s", "go on", Mode::Steer).unwrap();
    assert!(outcome.queued.is_some() && outcome.steered.is_none());
    assert!(bench.steers().is_empty() && bench.starts().is_empty());
}

#[test]
fn two_steers_in_a_row_join_the_one_run_in_order() {
    let (mut bench, run) = running("steer-two", true, false);
    let a = steer(&mut bench, "a", "first correction");
    let b = steer(&mut bench, "b", "second correction");
    report(&mut bench, a, Steered::Accepted);
    report(&mut bench, b, Steered::Accepted);
    assert!(a.item_id < b.item_id && a.message != b.message);
    reply(&mut bench, "both done");
    end(&mut bench, "1", "completed");
    assert_eq!(told(&bench, "run_settled").len(), 1);
    assert_eq!(
        bench.count("SELECT COUNT(*) FROM harness_runs WHERE ordinal > 0"),
        1
    );
    let owners: Vec<i64> = bench
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT run_id FROM harness_items WHERE kind = 'steer' ORDER BY id")
                .fetch_all(&bench.pool),
        )
        .unwrap();
    assert_eq!(owners, [run.run_id, run.run_id]);
}

#[test]
fn stop_before_a_steer_is_written_holds_it_and_stop_after_keeps_it_in_the_run() {
    // Stopped before the write: the effect's claim refuses, and the
    // message waits first in the queue, held like everything Stop holds.
    let (mut bench, run) = running("steer-stop-before", true, false);
    let written = steer(&mut bench, "s", "correction");
    let (stop, got) = bench.command(
        "stop",
        Kind::Stop {
            run_id: Some(run.run_id),
        },
    );
    bench.play(vec![stop]);
    bench.got(got).unwrap();
    report(&mut bench, written, Steered::Accepted);
    assert_eq!(delivery(&bench, written.item_id), None);
    assert_eq!(queue(&bench), [("correction".into(), true)]);
    end(&mut bench, "1", "interrupted");
    assert_eq!(runs(&bench)[0].1, "interrupted");
    assert_eq!(bench.starts().len(), 1, "a held message does not start");

    // Stopped after the write: the steer stays where it went.
    let (mut bench, run) = running("steer-stop-after", true, false);
    let written = steer(&mut bench, "s", "correction");
    report(&mut bench, written, Steered::Accepted);
    let (stop, got) = bench.command(
        "stop",
        Kind::Stop {
            run_id: Some(run.run_id),
        },
    );
    bench.play(vec![stop]);
    bench.got(got).unwrap();
    end(&mut bench, "1", "interrupted");
    assert_eq!(
        delivery(&bench, written.item_id).as_deref(),
        Some("written")
    );
    assert!(queue(&bench).is_empty());
    assert_eq!(told(&bench, "run_settled").len(), 1);
}

#[test]
fn send_now_moves_a_waiting_message_into_the_running_run_keeping_its_identity() {
    let (mut bench, run) = running("steer-send-now", true, false);
    let waiting = say(&mut bench, "w", "use SI units", Mode::Queue)
        .unwrap()
        .queued
        .unwrap();
    let (msg, got) = bench.command("now", Kind::SteerQueued { run_id: waiting });
    bench.play(vec![msg]);
    let written = bench.got(got).unwrap().steered.expect("written in");
    assert_eq!(written.message, waiting, "the same message");
    assert!(queue(&bench).is_empty());
    let queues = told(&bench, "queue");
    assert_eq!(queues.last().unwrap()["event"]["messages"], json!([]));
    assert_eq!(
        runs(&bench)[1],
        (waiting, "steered".into(), -1),
        "one row changed status"
    );
    // The same id again changes nothing more.
    let (again, got) = bench.command("now", Kind::SteerQueued { run_id: waiting });
    bench.play(vec![again]);
    assert_eq!(bench.got(got).unwrap().steered, Some(written));
    assert_eq!(bench.steers().len(), 1);
    // Missed after all: back to the queue under that same identity, and
    // it starts as that run when the turn ends.
    report(&mut bench, written, Steered::Missed);
    assert_eq!(queue(&bench), [("use SI units".into(), false)]);
    end(&mut bench, "1", "completed");
    assert_eq!(bench.starts()[1].run.run_id, waiting);
    assert_ne!(waiting, run.run_id);
}

#[test]
fn send_now_is_refused_with_its_reason_where_the_turn_cannot_take_it() {
    let (mut bench, _) = running("steer-send-now-refused", false, false);
    let waiting = say(&mut bench, "w", "later", Mode::Queue)
        .unwrap()
        .queued
        .unwrap();
    let (msg, got) = bench.command("now", Kind::SteerQueued { run_id: waiting });
    bench.play(vec![msg]);
    assert_eq!(bench.got(got).unwrap_err(), refusals::CANNOT_STEER);
    // Refused, so nothing was recorded: the same id works once it can.
    bench.steer_by(true, false);
    let (msg, got) = bench.command("now", Kind::SteerQueued { run_id: waiting });
    bench.play(vec![msg]);
    assert!(bench.got(got).unwrap().steered.is_some());
}

#[test]
fn a_message_sent_while_an_approval_waits_declines_it_and_steers() {
    let (mut bench, _) = running("steer-approval", true, false);
    let (fake, asked) = ask(&bench, json!(3), &[]);
    bench.play(vec![asked]);
    let written = steer(&mut bench, "s", "Don't run that; read the file instead.");
    assert_eq!(fake.replies(), ["decline"]);
    let status: String = bench
        .runtime
        .block_on(sqlx::query_scalar("SELECT status FROM harness_requests").fetch_one(&bench.pool))
        .unwrap();
    assert_eq!(status, "resolved");
    assert_eq!(bench.steers().len(), 1);
    assert_eq!(
        delivery(&bench, written.item_id).as_deref(),
        Some("sending")
    );
}

#[test]
fn a_message_sent_while_a_question_waits_is_not_a_steer() {
    let (mut bench, _) = running("steer-question", true, false);
    let question = HarnessEvent::RequestAsked {
        request: NewRequest {
            action: Action::Question,
            payload: json!({ "questions": [{
                "id": "colour", "header": "Colour", "text": "Which colour?",
                "options": [{ "label": "Red", "description": "" }],
                "multi": false, "allowsTypedAnswer": true, "secret": false,
            }] }),
            options: vec![RequestOption {
                id: "answer".into(),
                label: "Answer".into(),
                decision: Decision::Accept,
                scope: None,
            }],
            native_ref: "5".into(),
            paths: Vec::new(),
        },
        route: Some(LiveRoute(std::sync::Arc::new(Fake::default()))),
    };
    let asked = bench.session(question);
    bench.play(vec![asked]);
    let outcome = say(&mut bench, "s", "red", Mode::Auto).unwrap();
    assert!(outcome.queued.is_some(), "typed text answers the question");
    assert!(bench.steers().is_empty());
    let waiting = outcome.queued.unwrap();
    let (msg, got) = bench.command("now", Kind::SteerQueued { run_id: waiting });
    bench.play(vec![msg]);
    assert_eq!(bench.got(got).unwrap_err(), refusals::QUESTION_WAITS);
}

#[test]
fn a_steer_never_lets_a_waiting_message_start_before_the_runs_own_terminal() {
    let (mut bench, _) = running("steer-claim", true, false);
    say(&mut bench, "w", "next", Mode::Queue).unwrap();
    let written = steer(&mut bench, "s", "fix");
    report(&mut bench, written, Steered::Accepted);
    let echo = bench.session(HarnessEvent::SteerEchoed {
        client_id: bench.steers()[0].client.clone(),
    });
    bench.play(vec![echo]);
    assert_eq!(bench.starts().len(), 1, "the waiting message still waits");
    end(&mut bench, "1", "completed");
    assert_eq!(bench.starts().len(), 2);
    assert_eq!(bench.starts()[1].text, "next");
}
