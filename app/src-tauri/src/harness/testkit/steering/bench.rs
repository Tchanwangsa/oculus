//! The fake bridge's side of the steering checks: a chat whose run its
//! provider has taken, steers sent into it, and the bridge's answers to
//! them played back on the bus.

use serde_json::Value;

use crate::harness::app::send::Sent;
use crate::harness::app::BusMsg;
use crate::harness::core::Kind;
use crate::harness::manager::policy::Mode;
use crate::harness::store::steer::Written;
use crate::harness::store::{self, RunStart};
use crate::harness::{HarnessEvent, Provider, SendOptions, Steered};

use super::super::commands::{Bench, Result};

/// A chat whose first run its provider has taken, as turn `1` of session
/// `1`, on a bridge that steers natively (`native`) or by restart.
pub(super) fn running(name: &str, native: bool, restart: bool) -> (Bench, RunStart) {
    let mut bench = Bench::new(name);
    bench.steer_by(native, restart);
    let run = start(&mut bench, "send", "question");
    (bench, run)
}

/// A run sent and taken by its provider, as the start step would.
pub(super) fn start(bench: &mut Bench, id: &str, text: &str) -> RunStart {
    let (msg, got) = bench.send(id, text);
    bench.play(vec![msg]);
    let run = bench.got(got).unwrap().run.expect("the run starts");
    accept(bench, run, "1");
    run
}

/// The start step of `run` took it and its provider accepted it as `turn`.
pub(super) fn accept(bench: &mut Bench, run: RunStart, turn: &str) {
    let claimed = store::claim_start(&bench.pool, &run);
    assert!(bench.runtime.block_on(claimed).unwrap());
    bench.play(vec![
        BusMsg::Sent {
            thread_id: bench.thread,
            provider: Provider::Claude,
            run_id: run.run_id,
            outcome: Sent::Accepted,
        },
        BusMsg::Bound {
            thread_id: bench.thread,
            generation: 1,
            turn: turn.into(),
        },
    ]);
}

/// A send under `mode`, decided and committed.
pub(super) fn say(bench: &mut Bench, id: &str, text: &str, mode: Mode) -> Result {
    let kind = Kind::Send {
        provider: Provider::Claude,
        text: text.into(),
        options: SendOptions::default(),
        mode,
    };
    let (msg, got) = bench.command(id, kind);
    bench.play(vec![msg]);
    bench.got(got)
}

/// A steer sent and handed to its effect: what it wrote.
pub(super) fn steer(bench: &mut Bench, id: &str, text: &str) -> Written {
    let outcome = say(bench, id, text, Mode::Auto).unwrap();
    let written = outcome.steered.expect("the message steers");
    assert!(outcome.run.is_none() && outcome.queued.is_none());
    written
}

/// The steer's effect claims its write, as `app/steer.rs` does, and the
/// bridge says `outcome`.
pub(super) fn report(bench: &mut Bench, written: Written, outcome: Steered) {
    let claim = store::steer::claim(&bench.pool, bench.thread, written.item_id);
    let claimed = bench.runtime.block_on(claim).unwrap();
    let outcome = match claimed {
        true => outcome,
        false => Steered::Withdrawn,
    };
    bench.play(vec![BusMsg::Steered {
        thread_id: bench.thread,
        provider: Provider::Claude,
        written,
        outcome,
    }]);
}

/// The bridge's turn `turn` ends with `status`.
pub(super) fn end(bench: &mut Bench, turn: &str, status: &str) {
    let ended = HarnessEvent::TurnFinished {
        status: status.into(),
        turn: Some(turn.into()),
    };
    let msg = bench.session(ended);
    bench.play(vec![msg]);
}

/// The agent says `text` in the open turn.
pub(super) fn reply(bench: &mut Bench, text: &str) {
    let said = HarnessEvent::AssistantMessage {
        text: text.into(),
        source: None,
    };
    let msg = bench.session(said);
    bench.play(vec![msg]);
}

/// A steer's delivery, as its row holds it; `None` once the row is gone.
pub(super) fn delivery(bench: &Bench, item_id: i64) -> Option<String> {
    bench
        .runtime
        .block_on(
            sqlx::query_scalar(
                "SELECT json_extract(meta, '$.delivery') FROM harness_items WHERE id = ?1",
            )
            .bind(item_id)
            .fetch_optional(&bench.pool),
        )
        .unwrap()
}

/// Every run of the chat that ran or runs: id, status, ordinal.
pub(super) fn runs(bench: &Bench) -> Vec<(i64, String, i64)> {
    bench
        .runtime
        .block_on(
            sqlx::query_as(
                "SELECT id, execution_status, ordinal FROM harness_runs
                  WHERE thread_id = ?1 ORDER BY id",
            )
            .bind(bench.thread)
            .fetch_all(&bench.pool),
        )
        .unwrap()
}

/// The chat's waiting messages, first to go out first.
pub(super) fn queue(bench: &Bench) -> Vec<(String, bool)> {
    let list = store::queue::list(&bench.pool, bench.thread);
    bench
        .runtime
        .block_on(list)
        .unwrap()
        .into_iter()
        .map(|m| (m.text, m.held))
        .collect()
}

/// The envelopes of one event type told so far.
pub(super) fn told(bench: &Bench, kind: &str) -> Vec<Value> {
    bench
        .events()
        .into_iter()
        .filter(|e| e["event"]["type"] == kind)
        .collect()
}
