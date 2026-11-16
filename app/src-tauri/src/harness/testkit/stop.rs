//! Stop is a saved fact. It marks the run the student saw, holds what waits
//! behind it and leaves the turn to end by its own terminal; a failed turn
//! holds the queue too, startup holds whatever still waits, and Resume, or
//! the student's next message, lets it go again.

use serde_json::{json, Value};

use crate::harness::app::continuation::{FAILED_BEFORE, STOPPED};
use crate::harness::app::send::Sent;
use crate::harness::app::BusMsg;
use crate::harness::core::{load, Command, Kind, Outcome};
use crate::harness::{store, HarnessEvent, Provider, SendOptions};

use super::commands::{ended, Bench};
use super::continuation::{resend_as, seed_card};
use super::identity::as_turn;
use super::rig::Rig;
use super::scenario::Scenario;

fn send(rig: &mut Rig, id: &str, options: SendOptions) -> Outcome {
    rig.command(Command {
        id: id.into(),
        thread_id: Some(rig.thread),
        kind: Kind::Send {
            provider: rig.provider,
            text: "Replay request".into(),
            options,
            mode: crate::harness::manager::policy::Mode::Queue,
        },
    })
    .unwrap()
}

fn act(rig: &mut Rig, id: &str, kind: Kind) -> Outcome {
    rig.command(Command {
        id: id.into(),
        thread_id: Some(rig.thread),
        kind,
    })
    .unwrap()
}

/// A run's status and whether it is marked as stopped.
fn run_row(rig: &Rig, run_id: i64) -> (String, bool) {
    rig.runtime
        .block_on(
            sqlx::query_as(
                "SELECT execution_status, stop_requested_at IS NOT NULL FROM harness_runs WHERE id = ?1",
            )
            .bind(run_id)
            .fetch_one(&rig.pool),
        )
        .unwrap()
}

fn queued(rig: &Rig) -> Vec<(String, bool)> {
    let list = rig
        .runtime
        .block_on(store::queue::list(&rig.pool, rig.thread));
    list.unwrap().into_iter().map(|m| (m.id, m.held)).collect()
}

fn accepted(bench: &Bench, run_id: i64) -> BusMsg {
    BusMsg::Sent {
        thread_id: bench.thread,
        provider: Provider::Claude,
        run_id,
        outcome: Sent::Accepted,
    }
}

fn listed(bench: &Bench) -> Vec<(String, bool)> {
    let queue = bench
        .runtime
        .block_on(store::queue::list(&bench.pool, bench.thread));
    queue
        .unwrap()
        .into_iter()
        .map(|m| (m.text, m.held))
        .collect()
}

/// Through Codex: a message waits behind the turn, Stop interrupts the turn.
/// The partial answer and the waiting message both survive; nothing goes
/// out until Resume, which starts the message as its own run.
#[test]
fn stop_keeps_the_partial_answer_and_the_queue_and_resume_starts_the_message() {
    const THREAD: &str = "01a09ebd-3374-7100-bb0f-c42308e70fba";
    const TURN: &str = "01a09ebd-33de-7d00-825b-ed4c057a5347";
    let mut scenario =
        Scenario::recorded("codex-interrupt").gate("terminal", |f| f["method"] == "turn/completed");
    let mut interrupt = scenario.fixture.expected_outbound[0].clone();
    interrupt.frame = json!({
        "jsonrpc": "2.0", "id": 2, "method": "turn/interrupt",
        "params": {"threadId": THREAD, "turnId": TURN},
    });
    interrupt.response = json!({});
    let mut again = scenario.fixture.expected_outbound[0].clone();
    again.frame["id"] = 3.into();
    again.response["turn"]["id"] = "second-turn".into();
    scenario
        .fixture
        .expected_outbound
        .extend([interrupt, again]);
    let frames = scenario.frames.clone();
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");
    let first: i64 = rig
        .runtime
        .block_on(sqlx::query_scalar("SELECT id FROM harness_runs").fetch_one(&rig.pool))
        .unwrap();
    let waiting = send(&mut rig, "mid-turn", SendOptions::default());
    let waiting = waiting.queued.expect("it waits behind the turn");

    let stop = Kind::Stop {
        run_id: Some(first),
    };
    let stopped = act(&mut rig, "stop", stop);
    assert_eq!(stopped.stopped, Some(first));
    assert_eq!(
        run_row(&rig, first),
        ("running".into(), true),
        "marked, not closed"
    );
    assert_eq!(
        queued(&rig),
        [(waiting.to_string(), true)],
        "held, not dropped"
    );
    // Stop again changes nothing and asks the provider nothing more.
    let again = Kind::Stop {
        run_id: Some(first),
    };
    act(&mut rig, "stop-again", again);

    // The CLI's own terminal is what ends the run.
    rig.resume();
    rig.drain();
    assert_eq!(run_row(&rig, first).0, "interrupted");
    assert_eq!(run_row(&rig, waiting).0, "queued", "nothing went out");
    assert!(!rig.busy());
    let snapshot = rig
        .runtime
        .block_on(load::read(&rig.pool, rig.thread))
        .unwrap();
    assert!(snapshot.queued[0].held);
    assert!(snapshot.runs[0].stop_requested_at.is_some());

    act(&mut rig, "resume", Kind::Resume);
    assert_eq!(run_row(&rig, waiting).0, "running", "its own run, sent");
    for frame in as_turn(&frames, TURN, "second-turn") {
        rig.feed(&frame);
    }
    rig.eof();
    let replay = rig.finish();
    let runs = replay.ledger["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[1]["id"], waiting);
    let partial = replay
        .items
        .iter()
        .find(|i| i["kind"] == "assistant" && i["run_id"] == first)
        .expect("the stopped turn's text is kept");
    assert!(!partial["content"].as_str().unwrap().is_empty());
    let questions: Vec<_> = replay
        .items
        .iter()
        .filter(|i| i["kind"] == "user")
        .collect();
    assert_eq!(questions.len(), 2);
    assert_eq!(questions[1]["run_id"], waiting);
    let kinds: Vec<_> = replay
        .envelopes
        .iter()
        .map(|e| e["event"]["type"].as_str().unwrap())
        .filter(|k| ["stop_requested", "queue", "turn_finished", "user_message"].contains(k))
        .collect();
    assert_eq!(
        kinds,
        [
            "user_message",
            "queue",
            "stop_requested",
            "queue",
            "turn_finished",
            "queue",
            "queue",
            "user_message",
            "turn_finished"
        ]
    );
    let stops: Vec<_> = replay
        .outbound
        .iter()
        .filter(|f| f["method"] == "turn/interrupt")
        .collect();
    assert_eq!(stops.len(), 1, "the provider was asked once");
}

/// Stop names a run. One that arrives after that run finished, when the
/// next message is already being answered, stops nothing and holds nothing.
#[test]
fn stop_for_a_run_that_just_finished_touches_nothing() {
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
    let first: i64 = rig
        .runtime
        .block_on(sqlx::query_scalar("SELECT id FROM harness_runs").fetch_one(&rig.pool))
        .unwrap();
    let second = send(&mut rig, "second", SendOptions::default());
    let second = second.queued.unwrap();
    let third = send(&mut rig, "third", SendOptions::default());
    let third = third.queued.unwrap();
    rig.resume();
    rig.drain();
    assert_eq!(run_row(&rig, first).0, "completed");
    assert_eq!(run_row(&rig, second).0, "running");

    // The click was made while the first run was on screen.
    let late = Kind::Stop {
        run_id: Some(first),
    };
    let stopped = act(&mut rig, "late-stop", late);
    assert_eq!(stopped.stopped, None);
    assert_eq!(run_row(&rig, second), ("running".into(), false));
    assert_eq!(
        queued(&rig),
        [(third.to_string(), false)],
        "the queue is not held"
    );

    // The recording holds no third `turn/start`, so the message is taken
    // out again before the turn ends and the queue would send it.
    act(&mut rig, "remove", Kind::RemoveQueued { run_id: third });
    for frame in as_turn(&frames, FIRST, "second-turn") {
        rig.feed(&frame);
    }
    rig.eof();
    let replay = rig.finish();
    assert_eq!(replay.ledger["runs"][1]["execution_status"], "completed");
    assert!(replay
        .outbound
        .iter()
        .all(|f| f["method"] != "turn/interrupt"));
}

/// Stop during a slow start: the run is saved and its start step has not
/// called the provider yet. The step finds the run stopped and does not
/// call; the run ends interrupted with no provider write at all.
#[test]
fn stop_during_a_slow_start_cancels_before_the_provider_is_called() {
    for name in ["claude-ls", "codex-ls", "opencode-v1-ls", "antigravity-ls"] {
        let mut scenario = Scenario::recorded(name);
        scenario.fixture.expected_outbound.clear();
        let mut rig = Rig::start(scenario);
        rig.hold_effects();
        let run = rig.send("slow").unwrap();
        let behind = send(&mut rig, "behind", SendOptions::default());
        let behind = behind.queued.unwrap();
        let stop = Kind::Stop {
            run_id: Some(run.run_id),
        };
        act(&mut rig, "stop", stop);
        assert_eq!(
            run_row(&rig, run.run_id),
            ("starting".into(), true),
            "{name}"
        );

        rig.run_effects();
        assert_eq!(run_row(&rig, run.run_id).0, "interrupted", "{name}");
        assert_eq!(queued(&rig), [(behind.to_string(), true)], "{name}");
        assert!(!rig.busy(), "{name}");
        let replay = rig.finish();
        assert!(
            replay.outbound.is_empty(),
            "{name}: the provider was never called"
        );
        assert!(
            replay.items.iter().any(|i| i["kind"] == "interrupted"),
            "{name}: the chat says the turn was stopped"
        );
        let ended = replay
            .envelopes
            .iter()
            .filter(|e| e["event"]["type"] == "turn_finished")
            .count();
        assert_eq!(ended, 1, "{name}");
    }
}

/// The provider fails the turn (here, opencode reporting its own error): the
/// message behind it is not sent into the same failure. It stays, held.
#[test]
fn a_failed_turn_leaves_the_queue_held() {
    let mut rig = Rig::start(Scenario::recorded("opencode-v1-error"));
    rig.open();
    let behind = send(&mut rig, "behind", SendOptions::default());
    let behind = behind.queued.unwrap();
    rig.resume();
    rig.drain();
    assert_eq!(queued(&rig), [(behind.to_string(), true)]);
    assert!(!rig.busy());
    rig.close();
    let replay = rig.finish();
    assert_eq!(replay.ledger["runs"][0]["execution_status"], "failed");
    assert_eq!(replay.ledger["runs"][1]["execution_status"], "queued");
    let told = replay
        .envelopes
        .iter()
        .rev()
        .find(|e| e["event"]["type"] == "queue")
        .unwrap();
    assert_eq!(told["event"]["messages"][0]["held"], true);
}

/// A sign-in failure is a failed start: the CLI refuses before any turn
/// exists. The run fails with the error on screen, and what waits is held,
/// a card's continuation cancelled with the reason on its card.
#[test]
fn a_sign_in_failure_leaves_the_queue_held() {
    let mut bench = Bench::new("stop-sign-in");
    let card = bench.runtime.block_on(seed_card(&bench.pool, bench.thread));
    let (first, sent) = bench.send("c1", "first");
    let (typed, _) = bench.send("c2", "typed");
    let settled = bench.app(HarnessEvent::CardSettled {
        proposal: card.clone(),
    });
    bench.play(vec![first, typed, settled]);
    let run = bench.got(sent).unwrap().run.unwrap();
    let refused = BusMsg::Sent {
        thread_id: bench.thread,
        provider: Provider::Claude,
        run_id: run.run_id,
        outcome: Sent::Failed("Not logged in · Please run /login".into()),
    };
    bench.play(vec![refused]);

    assert_eq!(listed(&bench), [("typed".to_string(), true)]);
    assert_eq!(bench.effects(), ["start first"], "nothing else was started");
    let statuses: Vec<String> = bench
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT execution_status FROM harness_runs ORDER BY id")
                .fetch_all(&bench.pool),
        )
        .unwrap();
    assert_eq!(statuses, ["failed", "queued", "cancelled"]);
    let state = bench
        .runtime
        .block_on(crate::db::proposals::get(&bench.pool, &card))
        .unwrap()
        .state;
    assert_eq!(
        state["continuation"],
        json!({"phase": "skipped", "reason": FAILED_BEFORE})
    );
    let kinds = bench.kinds();
    let tail: Vec<_> = kinds
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(String::as_str)
        .collect();
    assert_eq!(
        tail,
        ["error", "run_settled", "queue", "proposal", "turn_finished"]
    );
    let error: String = bench
        .runtime
        .block_on(
            sqlx::query_scalar("SELECT content FROM harness_items WHERE kind = 'error'")
                .fetch_one(&bench.pool),
        )
        .unwrap();
    assert!(error.contains("Not logged in"));

    // The student's next message lifts the hold and goes out behind it.
    let (next, _) = bench.send("c3", "next");
    bench.play(vec![next]);
    assert_eq!(bench.effects(), ["start first", "start typed"]);
    assert_eq!(listed(&bench), [("next".to_string(), false)]);
}

/// Stop on the Bench's scripted bus: the provider is asked only once it has
/// the turn, a waiting continuation is cancelled with the reason on its
/// card, and Resume lets the student's message go.
#[test]
fn stop_asks_the_provider_once_it_has_the_turn_and_resume_lets_the_queue_go() {
    let mut bench = Bench::new("stop-bench");
    let card = bench.runtime.block_on(seed_card(&bench.pool, bench.thread));
    let (first, sent) = bench.send("c1", "first");
    let (typed, _) = bench.send("c2", "typed");
    let settled = bench.app(HarnessEvent::CardSettled {
        proposal: card.clone(),
    });
    bench.play(vec![first, typed, settled]);
    let run = bench.got(sent).unwrap().run.unwrap().run_id;

    let stop = || Kind::Stop { run_id: Some(run) };
    let (stopping, stopped) = bench.command("c3", stop());
    bench.play(vec![stopping]);
    assert_eq!(bench.got(stopped).unwrap().stopped, Some(run));
    assert_eq!(
        bench.effects(),
        ["start first"],
        "the provider has no turn to stop yet"
    );
    assert_eq!(listed(&bench), [("typed".to_string(), true)]);
    let state = bench
        .runtime
        .block_on(crate::db::proposals::get(&bench.pool, &card))
        .unwrap()
        .state;
    assert_eq!(
        state["continuation"],
        json!({"phase": "skipped", "reason": STOPPED})
    );

    // The start step took the run before the stop and now reports: the
    // provider has the turn, so it is asked to stop it, once.
    let taken = sqlx::query("UPDATE harness_runs SET execution_status = 'running' WHERE id = ?1")
        .bind(run)
        .execute(&bench.pool);
    bench.runtime.block_on(taken).unwrap();
    bench.play(vec![accepted(&bench, run), accepted(&bench, run)]);
    assert_eq!(bench.effects(), ["start first", "interrupt"]);
    let (again, _) = bench.command("c4", stop());
    bench.play(vec![again]);
    assert_eq!(bench.effects(), ["start first", "interrupt"]);

    bench.play(vec![bench.app(ended("interrupted"))]);
    assert_eq!(bench.effects(), ["start first", "interrupt"], "held");
    let (resume, _) = bench.command("c5", Kind::Resume);
    bench.play(vec![resume]);
    assert_eq!(bench.effects(), ["start first", "interrupt", "start typed"]);
    assert!(listed(&bench).is_empty());
}

/// A turn the CLI started itself has no run. A stop that names none
/// reaches it, and Resume cannot release a message into it: the driver
/// waits for that turn's own end.
#[test]
fn resume_does_not_release_a_message_into_a_turn_the_cli_started() {
    let mut bench = Bench::new("stop-own-turn");
    let own = bench.session(HarnessEvent::UnpromptedTurn { after: Vec::new() });
    let started = bench.session(HarnessEvent::TurnStarted {
        turn: Some("2".into()),
    });
    let (typed, sent) = bench.send("c1", "typed");
    bench.play(vec![own, started, typed]);
    assert!(
        bench.got(sent).unwrap().queued.is_some(),
        "it waits for the CLI's turn"
    );

    let (stop, _) = bench.command("c2", Kind::Stop { run_id: None });
    bench.play(vec![stop]);
    assert_eq!(bench.effects(), ["interrupt"]);
    assert_eq!(listed(&bench), [("typed".to_string(), true)]);

    let (resume, _) = bench.command("c3", Kind::Resume);
    bench.play(vec![resume]);
    assert_eq!(
        bench.effects(),
        ["interrupt"],
        "the CLI's turn is still open"
    );
    assert_eq!(listed(&bench), [("typed".to_string(), false)]);

    let over = bench.session(HarnessEvent::TurnFinished {
        status: "interrupted".into(),
        turn: Some("2".into()),
    });
    bench.play(vec![over]);
    assert_eq!(bench.effects(), ["interrupt", "start typed"]);
}

/// What still waits when the app closes is held at the next launch, and
/// Resume sends it with everything it was queued with: the options are
/// saved on the run, so they outlive the process that took them.
#[test]
fn a_waiting_message_is_held_at_startup_and_resumes_with_its_options() {
    let context = "the lecture's moment";
    let mut scenario =
        Scenario::recorded("codex-ls").gate("terminal", |f| f["method"] == "turn/completed");
    let prompt = format!(
        "Replay request\n\n---\n\n## What the student had open when this was sent\n\n{context}"
    );
    let mut resumed = scenario.fixture.expected_outbound[0].clone();
    resumed.frame = resend_as(resumed.frame.take(), &prompt);
    resumed.response["turn"]["id"] = "resumed-turn".into();
    scenario.fixture.expected_outbound.push(resumed);
    let mut rig = Rig::start(scenario);
    rig.open();
    rig.until("terminal");
    let options: SendOptions =
        serde_json::from_value(json!({"context": context, "at": 42})).unwrap();
    let waiting = send(&mut rig, "mid-turn", options).queued.unwrap();
    assert_eq!(queued(&rig), [(waiting.to_string(), false)]);

    // Killed mid-turn, started again.
    let mut rig = rig.restart();
    rig.startup().unwrap();
    rig.drain();
    assert_eq!(
        queued(&rig),
        [(waiting.to_string(), true)],
        "nothing starts at launch"
    );
    assert!(!rig.busy());

    act(&mut rig, "resume", Kind::Resume);
    assert_eq!(run_row(&rig, waiting).0, "running");
    let meta: Option<String> = rig
        .runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT i.meta FROM harness_items i JOIN harness_runs r ON r.initiating_item_id = i.id
                  WHERE r.id = ?1",
            )
            .bind(waiting)
            .fetch_one(&rig.pool),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&meta.unwrap()).unwrap()["at"],
        42
    );
    let replay = rig.finish();
    let sent = replay.outbound.last().unwrap().to_string();
    assert!(
        sent.contains(context),
        "the provider got what the student had open"
    );
    let statuses: Vec<_> = replay.ledger["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["execution_status"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(statuses, ["interrupted", "running"]);
}
