use crate::harness::manager::policy::{resolve_delivery, Delivery, Mode};
use crate::harness::{Provider, SendOptions};

use super::super::command::Command;
use super::super::commit::Change;
use super::super::state::State;
use super::{select, steer, Decision, Plan};

/// A message starts its run on an idle chat, waits behind a busy one or
/// behind messages already waiting, or is written into the running run
/// (`manager/policy.rs` decides which from the send's mode and the bridge).
/// A send with no chat makes one first, in the same commit. A message that
/// starts or waits also lifts a hold on the queue: it joins the back, and
/// the held messages go out ahead of it. A run takes the chat's selection
/// when it starts, not when it was typed (`commit::begin`). A steer declines
/// the approvals its run waits on: the student answered with a message.
pub(super) fn decide(
    state: &State,
    command: &Command,
    (provider, mode): (Provider, Mode),
    text: &str,
    options: &SendOptions,
) -> Result<Decision, String> {
    let mut changes = Vec::new();
    match &state.thread {
        // A send carries the composer's selection, which becomes the chat's:
        // another provider's is the switch, refused where it cannot be made.
        Some(row) => {
            if let Some(reason) = select::refusal(state, provider) {
                return Err(reason);
            }
            let (model, effort) = (options.model.as_ref(), options.reasoning_effort.as_ref());
            changes.extend(select::change(row, provider, model, effort));
        }
        None => changes.push(Change::CreateThread {
            provider,
            options: options.clone(),
            first_message: text.to_string(),
        }),
    }
    // A slash command is never written into a turn, and a steer goes to the
    // bridge of the run it joins, whatever the composer has picked since.
    let slash = text.trim_start().starts_with('/');
    let bridge = state.open.map_or(provider, |open| open.provider);
    let caps = state.live.steering.of(bridge);
    let (text, options) = (text.to_string(), options.clone());
    let delivery = match resolve_delivery(mode, &state.chat(), &caps, slash) {
        // While a question waits, typed text is its answer (the composer
        // sends that as `Respond`), so a message sent anyway waits.
        Delivery::Steer | Delivery::Restart if state.questions > 0 => Delivery::Queue,
        delivery => delivery,
    };
    match (delivery, state.open) {
        (Delivery::Steer | Delivery::Restart, Some(open)) => {
            changes.extend(steer::declines(state));
            changes.push(Change::Steer {
                text,
                options,
                target: steer::target(open),
                from: None,
                restart: delivery == Delivery::Restart,
            });
        }
        // The bridge cannot restart a turn: it is stopped, and the message
        // goes out first once it has ended.
        (Delivery::StopThenQueue, Some(open)) => {
            if !open.stop_requested {
                changes.push(Change::RequestStop {
                    run_id: open.run_id,
                });
            }
            changes.push(Change::Enqueue {
                text,
                options,
                front: true,
            });
        }
        (Delivery::Start, _) => {
            unhold(state, &mut changes);
            changes.push(Change::Begin {
                text,
                options,
                receipt: command.id.clone(),
                queued: None,
            });
        }
        _ => {
            // The student's message lifts a hold on the queue: it joins
            // the back, and the held messages go out ahead of it.
            unhold(state, &mut changes);
            changes.push(Change::Enqueue {
                text,
                options,
                front: false,
            });
        }
    }
    Ok(Decision::Commit(Plan {
        changes,
        text: None,
    }))
}

fn unhold(state: &State, changes: &mut Vec<Change>) {
    if state.held > 0 {
        changes.push(Change::Unhold);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{Live, OpenRun, Takeover};
    use super::*;
    use crate::harness::core::Kind;
    use crate::harness::store::ThreadRow;

    fn thread(provider: Provider) -> ThreadRow {
        ThreadRow {
            id: 4,
            provider,
            model: None,
            effort: None,
            subject_code: None,
            lecture: None,
        }
    }

    fn state(thread: Option<ThreadRow>, busy: bool) -> State {
        State {
            thread,
            live: Live::default(),
            question: None,
            newest_anchor: None,
            holder: None,
            waiting: 0,
            held: 0,
            open: busy.then_some(OpenRun {
                run_id: 9,
                starting: false,
                stop_requested: false,
                provider: Provider::Codex,
                attempt_id: 1,
                binding_id: 1,
            }),
            request: None,
            pending: 0,
            questions: 0,
            approvals: Vec::new(),
            queued: None,
        }
    }

    fn send(thread_id: Option<i64>) -> Command {
        Command {
            id: "c1".into(),
            thread_id,
            kind: Kind::DeleteThread,
        }
    }

    fn changes(state: &State, command: &Command) -> Vec<Change> {
        let options = SendOptions::default();
        let codex = (Provider::Codex, Mode::Auto);
        match decide(state, command, codex, "hello", &options).unwrap() {
            Decision::Commit(plan) => plan.changes,
            Decision::Ask(_) => panic!("a send asks the provider nothing"),
        }
    }

    #[test]
    fn a_send_starts_on_an_idle_chat_under_its_own_id_and_waits_behind_a_busy_one() {
        let idle = changes(&state(Some(thread(Provider::Codex)), false), &send(Some(4)));
        assert!(matches!(
            idle.as_slice(),
            [Change::Begin { receipt, text, .. }] if receipt == "c1" && text == "hello"
        ));
        let busy = changes(&state(Some(thread(Provider::Codex)), true), &send(Some(4)));
        assert!(matches!(busy.as_slice(), [Change::Enqueue { text, .. }] if text == "hello"));
        // An idle chat whose queue is not empty: the new message goes behind it.
        let mut behind = state(Some(thread(Provider::Codex)), false);
        behind.waiting = 2;
        let queued = changes(&behind, &send(Some(4)));
        assert!(matches!(queued.as_slice(), [Change::Enqueue { .. }]));
        // Held messages are released by the student's next one, which goes last.
        behind.held = 2;
        let released = changes(&behind, &send(Some(4)));
        assert!(matches!(
            released.as_slice(),
            [Change::Unhold, Change::Enqueue { .. }]
        ));
        // Whatever the app holds the chat for makes a message wait too.
        let mut held = state(Some(thread(Provider::Codex)), false);
        held.live.held = true;
        let waits = changes(&held, &send(Some(4)));
        assert!(matches!(waits.as_slice(), [Change::Enqueue { .. }]));
    }

    fn decided(state: &State, mode: Mode, text: &str) -> Vec<Change> {
        let codex = (Provider::Codex, mode);
        let options = SendOptions::default();
        match decide(state, &send(Some(4)), codex, text, &options).unwrap() {
            Decision::Commit(plan) => plan.changes,
            Decision::Ask(_) => panic!("a send asks the provider nothing"),
        }
    }

    #[test]
    fn a_bridge_that_cannot_steer_queues_a_steer_and_stops_for_a_restart() {
        let busy = state(Some(thread(Provider::Codex)), true);
        for mode in [Mode::Steer, Mode::Queue, Mode::Auto] {
            assert!(
                matches!(
                    decided(&busy, mode, "correction").as_slice(),
                    [Change::Enqueue { front: false, .. }]
                ),
                "{mode:?}: the running turn is left as it is"
            );
        }
        assert!(matches!(
            decided(&busy, Mode::Restart, "again").as_slice(),
            [
                Change::RequestStop { run_id: 9 },
                Change::Enqueue { front: true, .. }
            ]
        ));
        // On an idle chat any mode is a plain start.
        let idle = state(Some(thread(Provider::Codex)), false);
        assert!(matches!(
            decided(&idle, Mode::Queue, "hi").as_slice(),
            [Change::Begin { .. }]
        ));
    }

    #[test]
    fn a_send_steers_a_turn_its_provider_has_taken_and_declines_what_it_waits_on() {
        let mut busy = state(Some(thread(Provider::Codex)), true);
        busy.live.steering = crate::harness::core::state::Steering::read(|p| {
            crate::harness::providers::capabilities::Capabilities {
                active_steering: true,
                ..crate::harness::providers::capabilities::of(p)
            }
        });
        // Not taken yet: no turn to write into.
        assert!(matches!(
            decided(&busy, Mode::Auto, "fix").as_slice(),
            [Change::Enqueue { .. }]
        ));
        busy.live.accepted = true;
        assert!(matches!(
            decided(&busy, Mode::Auto, "fix").as_slice(),
            [Change::Steer { from: None, restart: false, target, .. }] if target.run_id == 9
        ));
        assert!(matches!(
            decided(&busy, Mode::Auto, "/compact").as_slice(),
            [Change::Enqueue { .. }]
        ));
        busy.approvals = vec![(5, "decline".into())];
        assert!(matches!(
            decided(&busy, Mode::Auto, "fix").as_slice(),
            [Change::Respond { request_id: 5, option_id, .. }, Change::Steer { .. }] if option_id == "decline"
        ));
        busy.questions = 1;
        assert!(matches!(
            decided(&busy, Mode::Auto, "red").as_slice(),
            [Change::Enqueue { .. }]
        ));
    }

    #[test]
    fn a_send_with_no_chat_makes_one_in_the_same_commit() {
        let made = changes(&state(None, false), &send(None));
        assert!(matches!(
            made.as_slice(),
            [Change::CreateThread { first_message, .. }, Change::Begin { .. }] if first_message == "hello"
        ));
    }

    #[test]
    fn a_send_under_another_provider_switches_the_chat_where_the_bridge_takes_handoffs() {
        let options = SendOptions {
            model: Some("gpt".into()),
            ..SendOptions::default()
        };
        let codex = (Provider::Codex, Mode::Auto);
        let mut claude = state(Some(thread(Provider::Claude)), false);
        let refused = decide(&claude, &send(Some(4)), codex, "hello", &options);
        assert_eq!(
            refused.err().unwrap(),
            "Codex cannot take over a chat that has started. Start a new chat to switch."
        );
        claude.live.takeover = Takeover::read(|p| p == Provider::Codex);
        let Decision::Commit(plan) =
            decide(&claude, &send(Some(4)), codex, "hello", &options).unwrap()
        else {
            panic!("a send asks the provider nothing");
        };
        assert!(matches!(
            plan.changes.as_slice(),
            [
                Change::Select { provider: Provider::Codex, model: Some(model), effort: None },
                Change::Begin { .. }
            ] if model == "gpt"
        ));
        // Mid-turn the pick is stored and the message waits: it goes out
        // under whatever is chosen when its turn comes.
        claude.open = Some(OpenRun {
            run_id: 9,
            starting: false,
            stop_requested: false,
            provider: Provider::Codex,
            attempt_id: 1,
            binding_id: 1,
        });
        let Decision::Commit(plan) =
            decide(&claude, &send(Some(4)), codex, "hello", &options).unwrap()
        else {
            panic!("a send asks the provider nothing");
        };
        assert!(matches!(
            plan.changes.as_slice(),
            [Change::Select { .. }, Change::Enqueue { .. }]
        ));
        // A background task of the old provider's holds the chat to it.
        claude.live.background = true;
        let held = decide(&claude, &send(Some(4)), codex, "hello", &options);
        assert!(held.err().unwrap().contains("background task"));
    }
}
