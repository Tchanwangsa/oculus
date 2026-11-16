//! Restart steering against a fake bridge: the turn is stopped, its
//! attempt superseded on the terminal it sends, and the run carries on as
//! a second attempt with the correction. The ACP transport checks live in
//! `antigravity.rs`.

use crate::harness::core::Kind;
use crate::harness::manager::policy::Mode;

use super::bench::{accept, delivery, end, queue, reply, running, say, told};

fn attempts(bench: &super::super::commands::Bench, run_id: i64) -> Vec<(i64, String, String)> {
    bench
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT ordinal, reason, status FROM harness_attempts WHERE run_id = ?1 ORDER BY id",
            )
            .bind(run_id)
            .fetch_all(&bench.pool),
        )
        .unwrap()
}

#[test]
fn a_restart_steer_supersedes_the_attempt_and_the_run_settles_once_on_the_second() {
    let (mut bench, run) = running("restart-steer", false, true);
    say(&mut bench, "w", "waiting", Mode::Queue).unwrap();
    reply(&mut bench, "Feet: 10.");
    let written = say(&mut bench, "s", "In metres.", Mode::Steer)
        .unwrap()
        .steered
        .expect("a restart steer is a steer row too");
    assert_eq!(
        delivery(&bench, written.item_id).as_deref(),
        Some("restart")
    );
    assert!(bench.effects().contains(&"interrupt".to_string()));
    assert!(
        bench.steers().is_empty(),
        "nothing is written into the turn"
    );
    // The old turn's terminal closes its attempt only: no settlement, and
    // the waiting message stays behind the run.
    end(&mut bench, "1", "interrupted");
    assert!(told(&bench, "run_settled").is_empty());
    assert!(bench.open());
    assert_eq!(
        attempts(&bench, run.run_id),
        [
            (1, "initial".into(), "superseded".into()),
            (2, "steering_restart".into(), "starting".into())
        ]
    );
    let restarted = told(&bench, "attempt_restarted");
    assert_eq!(restarted.len(), 1);
    assert_eq!(restarted[0]["event"]["superseded"], run.attempt_id);
    // The driver sends the correction as the run's second attempt, after
    // the terminal and not before.
    let starts = bench.starts();
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[1].run.run_id, run.run_id);
    assert_eq!(starts[1].text, "In metres.");
    assert_ne!(starts[1].run.attempt_id, run.attempt_id);
    assert_eq!(queue(&bench), [("waiting".into(), false)]);
    assert_eq!(
        delivery(&bench, written.item_id).as_deref(),
        Some("delivered")
    );
    accept(&mut bench, starts[1].run, "2");
    reply(&mut bench, "Metres: 3.");
    end(&mut bench, "2", "completed");
    let settled = told(&bench, "run_settled");
    assert_eq!(settled.len(), 1);
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
    assert_eq!(answer, "Metres: 3.");
    let early: (String, Option<String>) = bench
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT display_role, lifecycle FROM harness_items WHERE content = 'Feet: 10.'",
            )
            .fetch_one(&bench.pool),
        )
        .unwrap();
    assert_eq!(early, ("commentary".into(), Some("partial".into())));
    // Only now does the waiting message go.
    assert_eq!(bench.starts()[2].text, "waiting");
}

#[test]
fn a_plain_send_never_restarts_a_turn_by_itself() {
    let (mut bench, _) = running("restart-auto", false, true);
    let outcome = say(&mut bench, "a", "later", Mode::Auto).unwrap();
    assert!(outcome.queued.is_some() && outcome.steered.is_none());
    assert!(!bench.effects().contains(&"interrupt".to_string()));
}

#[test]
fn two_corrections_restart_once_and_go_together() {
    let (mut bench, run) = running("restart-two", false, true);
    say(&mut bench, "a", "first", Mode::Steer).unwrap();
    say(&mut bench, "b", "second", Mode::Steer).unwrap();
    let interrupts = bench.effects().iter().filter(|e| *e == "interrupt").count();
    assert_eq!(interrupts, 1);
    end(&mut bench, "1", "interrupted");
    assert_eq!(bench.starts()[1].text, "first\n\nsecond");
    assert_eq!(attempts(&bench, run.run_id).len(), 2);
}

#[test]
fn stop_during_a_restart_ends_the_run_and_holds_the_correction() {
    let (mut bench, run) = running("restart-stop", false, true);
    say(&mut bench, "s", "correction", Mode::Steer).unwrap();
    let (stop, got) = bench.command(
        "stop",
        Kind::Stop {
            run_id: Some(run.run_id),
        },
    );
    bench.play(vec![stop]);
    bench.got(got).unwrap();
    end(&mut bench, "1", "interrupted");
    assert_eq!(told(&bench, "run_settled").len(), 1);
    assert_eq!(attempts(&bench, run.run_id).len(), 1);
    assert_eq!(queue(&bench), [("correction".into(), true)]);
    assert_eq!(bench.starts().len(), 1);
}

#[test]
fn a_restart_on_a_bridge_that_cannot_restart_stops_the_turn_and_sends_first_after() {
    let (mut bench, run) = running("restart-unable", false, false);
    say(&mut bench, "w", "waiting", Mode::Queue).unwrap();
    let outcome = say(&mut bench, "r", "start over", Mode::Restart).unwrap();
    assert!(outcome.queued.is_some());
    assert_eq!(
        told(&bench, "stop_requested")[0]["event"]["run_id"],
        run.run_id
    );
    assert_eq!(
        queue(&bench),
        [("start over".into(), false), ("waiting".into(), false)]
    );
    end(&mut bench, "1", "interrupted");
    assert_eq!(bench.starts()[1].text, "start over");
}

#[test]
fn a_restart_cancel_that_misses_requeues_the_same_message_and_keeps_the_old_answer() {
    let (mut bench, run) = running("restart-missed", false, true);
    reply(&mut bench, "Finished first.");
    let written = say(&mut bench, "s", "correction", Mode::Steer)
        .unwrap()
        .steered
        .unwrap();
    end(&mut bench, "1", "completed");
    assert_eq!(
        attempts(&bench, run.run_id),
        [(1, "initial".into(), "completed".into())]
    );
    assert_eq!(told(&bench, "run_settled").len(), 1);
    let starts = bench.starts();
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[1].run.run_id, written.message);
    assert_eq!(starts[1].text, "correction");
    let answer: String = bench.runtime.block_on(sqlx::query_scalar(
        "SELECT content FROM harness_items WHERE id = (SELECT answer_item_id FROM harness_runs WHERE id = ?1)"
    ).bind(run.run_id).fetch_one(&bench.pool)).unwrap();
    assert_eq!(answer, "Finished first.");
}

#[test]
fn a_failed_restart_cancel_holds_its_correction_and_opens_no_attempt() {
    let (mut bench, run) = running("restart-failed", false, true);
    say(&mut bench, "s", "correction", Mode::Steer).unwrap();
    end(&mut bench, "1", "failed");
    assert_eq!(
        attempts(&bench, run.run_id),
        [(1, "initial".into(), "failed".into())]
    );
    assert_eq!(queue(&bench), [("correction".into(), true)]);
    assert_eq!(bench.starts().len(), 1);
    assert_eq!(told(&bench, "run_settled").len(), 1);
}
