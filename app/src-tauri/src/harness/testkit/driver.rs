//! The driver and `starting`: a send returns at its commit, the start step
//! makes the provider call from what was saved, and a restart in the gap
//! between the two loses nothing and starts nothing.

use crate::harness::core::{load, Command, Kind};
use crate::harness::{store, SendOptions};

use super::identity::as_turn;
use super::rig::Rig;
use super::scenario::Scenario;

fn run_row(rig: &Rig, run_id: i64) -> (String, i64, i64, Option<i64>) {
    rig.runtime
        .block_on(
            sqlx::query_as(
                "SELECT execution_status, queue_held, ordinal, initiating_item_id FROM harness_runs WHERE id = ?1",
            )
            .bind(run_id)
            .fetch_one(&rig.pool),
        )
        .unwrap()
}

fn questions(rig: &Rig) -> i64 {
    rig.runtime
        .block_on(
            sqlx::query_scalar("SELECT COUNT(*) FROM harness_items WHERE kind = 'user'")
                .fetch_one(&rig.pool),
        )
        .unwrap()
}

/// A recording whose provider must never be written to.
fn unsent(name: &str) -> Scenario {
    let mut scenario = Scenario::recorded(name);
    scenario.fixture.expected_outbound.clear();
    scenario
}

/// The app dies after the send's commit and before its start step ran: the
/// provider never heard the message. It is back in the queue at the next
/// launch, held, and nothing starts it.
#[test]
fn a_crash_before_the_start_step_keeps_the_message_and_starts_nothing_at_launch() {
    for name in ["claude-ls", "codex-ls", "opencode-v1-ls", "antigravity-ls"] {
        let mut rig = Rig::start(unsent(name));
        rig.hold_effects();
        let sent = rig.send("typed").unwrap();
        assert_eq!(run_row(&rig, sent.run_id).0, "starting", "{name}");
        assert_eq!(questions(&rig), 1, "its question is on screen");

        let mut rig = rig.restart();
        rig.startup().unwrap();
        rig.drain();
        let (status, held, ordinal, question) = run_row(&rig, sent.run_id);
        assert_eq!((status.as_str(), held), ("queued", 1), "{name}");
        assert!(ordinal < 0 && question.is_none(), "{name}");
        assert_eq!(questions(&rig), 0, "it is a waiting message again");
        let snapshot = rig
            .runtime
            .block_on(load::read(&rig.pool, rig.thread))
            .unwrap();
        assert!(snapshot.runs.is_empty() && snapshot.items.is_empty());
        assert_eq!(snapshot.queued.len(), 1, "{name}");
        assert_eq!(snapshot.queued[0].text, "Replay request");
        assert!(snapshot.queued[0].held, "{name}: nothing starts by itself");
        // The same command again is still that message, not a second one.
        assert_eq!(rig.send("typed").unwrap().run_id, sent.run_id);
        assert!(!rig.busy());
        let replay = rig.finish();
        assert!(
            replay.outbound.is_empty(),
            "{name}: the provider heard nothing"
        );
        assert_eq!(replay.ledger["runs"].as_array().unwrap().len(), 1, "{name}");
    }
}

/// The app dies after the start step took the run for its provider call:
/// whether the provider heard is not known, so nothing is sent again. The
/// question stays where it was, with its run cut off.
#[test]
fn a_crash_after_the_start_step_took_the_run_keeps_its_question_and_resends_nothing() {
    let mut rig = Rig::start(unsent("codex-ls"));
    rig.hold_effects();
    let sent = rig.send("typed").unwrap();
    let taken = rig.runtime.block_on(store::claim_start(&rig.pool, &sent));
    assert!(taken.unwrap());
    let again = rig.runtime.block_on(store::claim_start(&rig.pool, &sent));
    assert!(!again.unwrap(), "a run is taken for its provider call once");

    let rig = rig.restart();
    rig.startup().unwrap();
    rig.drain();
    assert_eq!(run_row(&rig, sent.run_id).0, "interrupted");
    assert_eq!(questions(&rig), 1, "the message is still in the chat");
    let replay = rig.finish();
    assert!(replay.outbound.is_empty());
    assert!(replay
        .items
        .iter()
        .any(|i| i["kind"] == "delivery_notice" && i["run_id"] == sent.run_id));
}

/// A send returns once it is saved. A message sent before the first has
/// even reached its provider waits behind it, and each goes out from what
/// was saved, in turn.
#[test]
fn sends_return_at_their_commit_and_go_out_one_at_a_time() {
    const FIRST: &str = "01a08b4a-5f68-7c42-bc82-ab8e70d37d01";
    let mut scenario = Scenario::recorded("codex-ls");
    let mut again = scenario.fixture.expected_outbound[0].clone();
    again.frame["id"] = 2.into();
    again.response["turn"]["id"] = "second-turn".into();
    scenario.fixture.expected_outbound.push(again);
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.hold_effects();

    let first = rig.send("first").unwrap();
    assert_eq!(
        run_row(&rig, first.run_id).0,
        "starting",
        "saved, not yet sent"
    );
    let second = rig
        .command(Command {
            id: "second".into(),
            thread_id: Some(rig.thread),
            kind: Kind::Send {
                provider: rig.provider,
                text: "Replay request".into(),
                options: SendOptions::default(),
                mode: Default::default(),
            },
        })
        .unwrap();
    let second = second.queued.expect("the chat has a run going out");

    rig.run_effects();
    assert_eq!(
        run_row(&rig, first.run_id).0,
        "running",
        "the provider has it"
    );
    assert_eq!(run_row(&rig, second).0, "queued");
    rig.resume();
    rig.drain();
    assert_eq!(run_row(&rig, first.run_id).0, "completed");
    assert_eq!(run_row(&rig, second).0, "starting", "its turn has come");
    rig.run_effects();
    for frame in as_turn(&frames, FIRST, "second-turn") {
        rig.feed(&frame);
    }
    rig.eof();
    let replay = rig.finish();
    let statuses: Vec<_> = replay.ledger["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["execution_status"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(statuses, ["completed", "completed"]);
    assert!(!replay.busy);
}

/// The start step works from the saved run. One whose chat was deleted
/// while it waited to go out is not sent, and nothing of it comes back.
#[test]
fn the_start_step_sends_nothing_for_a_run_that_is_gone() {
    let mut rig = Rig::start(unsent("claude-ls"));
    rig.hold_effects();
    rig.send("typed").unwrap();
    let deleted = rig.command(Command {
        id: "delete".into(),
        thread_id: Some(rig.thread),
        kind: Kind::DeleteThread,
    });
    deleted.unwrap();
    rig.run_effects();
    let rows: i64 = rig
        .runtime
        .block_on(sqlx::query_scalar("SELECT COUNT(*) FROM harness_items").fetch_one(&rig.pool))
        .unwrap();
    assert_eq!(
        rows, 0,
        "a failed start wrote no error row into a deleted chat"
    );
    let replay = rig.finish();
    assert!(replay.outbound.is_empty());
}
