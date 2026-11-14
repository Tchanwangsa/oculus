//! A message that waits is a run (`store/queue.rs`): saved when it is
//! accepted, edited, removed and reordered by command, started as the same
//! run when its turn comes, and never mistaken for a run that ran by
//! anything that reads `harness_runs`.

use serde_json::{json, Value};

use crate::harness::core::{load, Command, Kind};
use crate::harness::{store, HarnessEvent, Provider, SendOptions};

use super::commands::{ended, Bench};
use super::identity::as_turn;
use super::rig::Rig;
use super::scenario::Scenario;

fn runs(bench: &Bench) -> Vec<(i64, String, i64, Option<i64>)> {
    bench
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT id, execution_status, ordinal, initiating_item_id FROM harness_runs ORDER BY id",
            )
            .fetch_all(&bench.pool),
        )
        .unwrap()
}

fn listed(bench: &Bench) -> Vec<String> {
    let queue = bench
        .runtime
        .block_on(store::queue::list(&bench.pool, bench.thread));
    queue.unwrap().into_iter().map(|m| m.text).collect()
}

/// The newest `queue` event's texts: what the webview's list shows.
fn told(bench: &Bench) -> Vec<String> {
    let events = bench.events();
    let queue = events
        .iter()
        .rev()
        .find(|e| e["event"]["type"] == "queue")
        .expect("the queue was told");
    queue["event"]["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["text"].as_str().unwrap().to_string())
        .collect()
}

/// Through a real bridge: a message sent mid-turn is saved as a queued run,
/// has no row and no place in the timeline's order until the turn ends,
/// then starts as that same run and is answered.
#[test]
fn a_message_sent_mid_turn_waits_as_a_run_and_starts_as_the_same_run() {
    const FIRST: &str = "01a08b4a-5f68-7c42-bc82-ab8e70d37d01";
    let mut scenario =
        Scenario::recorded("codex-ls").gate("terminal", |f| f["method"] == "turn/completed");
    let mut again = scenario.fixture.expected_outbound[0].clone();
    again.frame["id"] = 2.into();
    again.response["turn"]["id"] = "second-turn".into();
    scenario.fixture.expected_outbound.push(again);
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");

    let waiting = rig
        .command(Command {
            id: "mid-turn".into(),
            thread_id: Some(rig.thread),
            kind: Kind::Send {
                provider: rig.provider,
                text: "Replay request".into(),
                options: SendOptions::default(),
            },
        })
        .unwrap();
    let queued = waiting.queued.expect("it waits");
    assert!(waiting.run.is_none());
    let (status, ordinal, question): (String, i64, Option<i64>) = rig
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT execution_status, ordinal, initiating_item_id FROM harness_runs WHERE id = ?1",
            )
            .bind(queued)
            .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!((status.as_str(), question), ("queued", None));
    assert!(ordinal < 0, "no place in the order turns ran in yet");
    let snapshot = rig
        .runtime
        .block_on(load::read(&rig.pool, rig.thread))
        .unwrap();
    assert_eq!(snapshot.queued.len(), 1, "a reload lists it");
    assert_eq!(snapshot.queued[0].id, queued.to_string());
    assert_eq!(snapshot.runs.len(), 1, "and draws no run for it");

    // The turn ends: the waiting run goes out, by itself.
    rig.resume();
    rig.drain();
    for frame in as_turn(&frames, FIRST, "second-turn") {
        rig.feed(&frame);
    }
    rig.eof();
    let replay = rig.finish();
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert_eq!(runs[1]["id"], queued, "the same run, started");
    assert_eq!(runs[1]["execution_status"], "completed");
    let questions: Vec<_> = replay
        .items
        .iter()
        .filter(|i| i["kind"] == "user")
        .collect();
    assert_eq!(questions.len(), 2);
    assert_eq!(runs[1]["initiating_item_id"], questions[1]["id"]);
    let first_answer = runs[0]["answer_item_id"].as_i64().unwrap();
    assert!(
        questions[1]["id"].as_i64().unwrap() > first_answer,
        "its question was written when it started, after the first turn's rows"
    );
    let answer = replay
        .items
        .iter()
        .find(|i| i["id"] == runs[1]["answer_item_id"])
        .expect("its own answer");
    assert_eq!(answer["run_id"], queued);
    // Told as it joined the queue, and again, empty, before its question.
    let kinds: Vec<_> = replay
        .envelopes
        .iter()
        .map(|e| e["event"]["type"].as_str().unwrap())
        .filter(|k| ["queue", "user_message", "turn_finished"].contains(k))
        .collect();
    assert_eq!(
        kinds,
        [
            "user_message",
            "queue",
            "turn_finished",
            "queue",
            "user_message",
            "turn_finished"
        ]
    );
    assert!(!replay.busy);
}

/// Edit, reorder and remove are commands on waiting runs. A run takes its
/// ordinal when it starts, so the last one typed can be the next turn.
#[test]
fn waiting_messages_are_edited_reordered_and_removed_and_go_out_in_queue_order() {
    let mut bench = Bench::new("queue-commands");
    let (first, _) = bench.send("c1", "first");
    let (two, second) = bench.send("c2", "second");
    let (three, third) = bench.send("c3", "third");
    let (four, fourth) = bench.send("c4", "fourth");
    bench.play(vec![first, two, three, four]);
    let [second, third, fourth] =
        [second, third, fourth].map(|sent| bench.got(sent).unwrap().queued.unwrap());
    assert_eq!(listed(&bench), ["second", "third", "fourth"]);
    assert_eq!(told(&bench), ["second", "third", "fourth"]);

    let edit = |run_id, text: &str| Kind::EditQueued {
        run_id,
        text: text.into(),
    };
    let (edited, _) = bench.command("c5", edit(third, "third, edited"));
    let (emptied, refused) = bench.command("c6", edit(third, "  "));
    let order = vec![fourth, second];
    let (moved, _) = bench.command("c7", Kind::ReorderQueued { order });
    let (removed, _) = bench.command("c8", Kind::RemoveQueued { run_id: second });
    bench.play(vec![edited, emptied, moved, removed]);
    assert!(bench
        .got(refused)
        .unwrap_err()
        .contains("remove it instead"));
    assert_eq!(listed(&bench), ["fourth", "third, edited"]);
    assert_eq!(told(&bench), ["fourth", "third, edited"]);
    let removed = runs(&bench).into_iter().find(|r| r.0 == second).unwrap();
    assert_eq!(removed.1, "cancelled");

    // The turn ends: the head of the queue starts, as run number two.
    bench.play(vec![bench.app(ended("completed"))]);
    assert_eq!(bench.effects(), ["start first", "start fourth"]);
    let started = runs(&bench).into_iter().find(|r| r.0 == fourth).unwrap();
    assert_eq!((started.1.as_str(), started.2), ("starting", 2));
    assert!(started.3.is_some(), "its question row is written now");
    assert_eq!(bench.questions(), ["first", "fourth"]);
    assert_eq!(told(&bench), ["third, edited"]);

    // A message that already went out is no longer there to change.
    let (late, changed) = bench.command("c9", edit(fourth, "too late"));
    let (gone, dropped) = bench.command("c10", Kind::RemoveQueued { run_id: fourth });
    bench.play(vec![late, gone]);
    assert!(bench.got(changed).is_ok() && bench.got(dropped).is_ok());
    assert_eq!(bench.questions(), ["first", "fourth"]);
    let still = runs(&bench).into_iter().find(|r| r.0 == fourth).unwrap();
    assert_eq!(still.1, "starting");
}

/// What the composer chose for a message rides the queue with it: the
/// options are stored whole on the waiting run.
#[test]
fn a_waiting_message_keeps_the_options_it_was_sent_with() {
    let mut bench = Bench::new("queue-options");
    let (first, _) = bench.send("c1", "first");
    let options: SendOptions = serde_json::from_value(json!({
        "model": "gpt-other", "reasoningEffort": "high", "context": "the moment", "at": 42
    }))
    .unwrap();
    let kind = Kind::Send {
        provider: Provider::Claude,
        text: "second".into(),
        options,
    };
    let (second, _) = bench.command("c2", kind);
    bench.play(vec![first, second, bench.app(ended("completed"))]);
    let starts = bench.starts();
    let start = starts.last().unwrap();
    assert_eq!(start.text, "second");
    assert_eq!(start.options.model.as_deref(), Some("gpt-other"));
    assert_eq!(start.options.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(start.options.context.as_deref(), Some("the moment"));
    assert_eq!(start.options.at, Some(42));
    assert!(
        start.respawn,
        "the chat's model changed as the message went out"
    );
    let (model, at): (Option<String>, Option<String>) = bench
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT t.model, i.meta FROM harness_threads t JOIN harness_items i ON i.thread_id = t.id
                  WHERE i.content = 'second'",
            )
            .fetch_one(&bench.pool),
        )
        .unwrap();
    assert_eq!(model.as_deref(), Some("gpt-other"));
    assert_eq!(
        serde_json::from_str::<Value>(&at.unwrap()).unwrap()["at"],
        42
    );
}

/// A continuation waits in the same queue, in order, and is never listed
/// as the student's.
#[test]
fn a_continuation_waits_its_turn_between_the_students_messages() {
    let mut bench = Bench::new("queue-continuation");
    let card = bench
        .runtime
        .block_on(super::continuation::seed_card(&bench.pool, bench.thread));
    let (first, _) = bench.send("c1", "first");
    let (before, _) = bench.send("c2", "before");
    let settled = bench.app(HarnessEvent::CardSettled {
        proposal: card.clone(),
    });
    let (after, _) = bench.send("c3", "after");
    bench.play(vec![first, before, settled, after]);
    assert_eq!(listed(&bench), ["before", "after"]);

    for _ in 0..3 {
        bench.play(vec![bench.app(ended("completed"))]);
    }
    assert_eq!(
        bench.effects(),
        [
            "start first",
            "start before",
            "start Sync finished — continuing",
            "start after"
        ],
        "first come, first served"
    );
    assert_eq!(
        bench.starts()[2].options.continuation.as_deref(),
        Some(card.as_str())
    );
    let origins: Vec<String> = bench
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT origin FROM harness_runs ORDER BY ordinal")
                .fetch_all(&bench.pool),
        )
        .unwrap();
    assert_eq!(origins, ["student", "student", "continuation", "student"]);
}

/// Everything that reads `harness_runs` sees waiting and cancelled runs
/// beside the ones that ran. None of them is taken for a turn: not by a
/// rewind, the answer a thread is named from, settlement, a reload, or the
/// repair at startup.
#[test]
fn readers_of_runs_tell_a_waiting_run_from_one_that_ran() {
    let mut bench = Bench::new("queue-readers");
    let card = bench
        .runtime
        .block_on(super::continuation::seed_card(&bench.pool, bench.thread));
    let mut script = bench.turn("c1", "first", "anchor-1");
    let (second, _) = bench.send("c2", "second");
    let (waits, waiting) = bench.send("c3", "waits");
    let (goes, removed) = bench.send("c4", "removed");
    let settled = bench.app(HarnessEvent::CardSettled {
        proposal: card.clone(),
    });
    script.extend([second, waits, goes, settled]);
    bench.play(script);
    let waiting = bench.got(waiting).unwrap().queued.unwrap();
    let removed = bench.got(removed).unwrap().queued.unwrap();
    // The second message reached its provider, as its start step records.
    let sent = bench.starts()[1].run;
    assert!(bench
        .runtime
        .block_on(store::claim_start(&bench.pool, &sent))
        .unwrap());
    let (remove, _) = bench.command("c5", Kind::RemoveQueued { run_id: removed });
    bench.play(vec![remove]);

    // Settlement closes only a run that is running: a waiting one has no
    // terminal to hear.
    let unsettled = bench
        .runtime
        .block_on(store::settle_run(&bench.pool, waiting, 0, "completed"))
        .unwrap();
    assert!(unsettled.is_none());

    // Naming reads the newest answer of a run that ran. (The play above
    // already claimed the name; the claim is reopened to read its seed.)
    bench
        .runtime
        .block_on(
            sqlx::query("UPDATE harness_threads SET title_generated = 0").execute(&bench.pool),
        )
        .unwrap();
    let seed = bench
        .runtime
        .block_on(store::claim_naming(&bench.pool, bench.thread))
        .unwrap()
        .expect("the first exchange names the thread");
    assert_eq!(
        (seed.first_message.as_str(), seed.reply.as_str()),
        ("first", "answer to first")
    );

    // A reload draws the runs that have rows, and lists what waits.
    let snapshot = bench
        .runtime
        .block_on(load::read(&bench.pool, bench.thread))
        .unwrap();
    let drawn: Vec<_> = snapshot
        .runs
        .iter()
        .map(|r| r.execution_status.as_str())
        .collect();
    assert_eq!(drawn, ["completed", "running"]);
    let queued: Vec<_> = snapshot.queued.iter().map(|m| m.text.as_str()).collect();
    assert_eq!(queued, ["waits"], "not the removed one, nor the card's");

    // Startup's repair interrupts what was running, cancels the card's
    // continuation with the reason on its card, and leaves the student's
    // waiting message where it was.
    let repaired = bench.runtime.block_on(
        crate::harness::app::startup::Startup::default().ready(|| async { Ok(bench.pool.clone()) }),
    );
    repaired.unwrap();
    let after: Vec<_> = runs(&bench)
        .into_iter()
        .map(|r| (r.1, r.3.is_some()))
        .collect();
    assert_eq!(
        after,
        [
            ("completed".to_string(), true),
            ("interrupted".to_string(), true),
            ("queued".to_string(), false),
            ("cancelled".to_string(), false),
            ("cancelled".to_string(), false),
        ]
    );
    assert_eq!(listed(&bench), ["waits"]);
    let state = bench
        .runtime
        .block_on(crate::db::proposals::get(&bench.pool, &card))
        .unwrap()
        .state;
    assert_eq!(state["continuation"]["phase"], "skipped");

    // A rewind takes the runs asked from the rows it removes, and nothing
    // that only waits.
    let question: i64 = bench.count("SELECT MIN(id) FROM harness_items WHERE kind = 'user'");
    bench
        .runtime
        .block_on(store::truncate_from(&bench.pool, bench.thread, question))
        .unwrap();
    let left: Vec<_> = runs(&bench).into_iter().map(|r| r.1).collect();
    assert_eq!(left, ["queued", "cancelled", "cancelled"]);
    assert_eq!(listed(&bench), ["waits"]);
}
