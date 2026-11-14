use crate::harness::manager::policy::{resolve_delivery, Delivery, Mode};
use crate::harness::providers::capabilities;
use crate::harness::{Provider, SendOptions};

use super::super::command::Command;
use super::super::commit::Change;
use super::super::state::State;
use super::{Decision, Plan};

/// A message starts its run on an idle chat and waits behind a busy one, or
/// behind messages already waiting. A send with no chat makes one first, in
/// the same commit. A message from the student also lifts a hold on the
/// queue: it joins the back, and the held messages go out ahead of it.
pub(super) fn decide(
    state: &State,
    command: &Command,
    (provider, mode): (Provider, Mode),
    text: &str,
    options: &SendOptions,
) -> Result<Decision, String> {
    let mut changes = Vec::new();
    match &state.thread {
        Some(row) if row.provider != provider => {
            return Err(format!(
                "thread {} is a {} thread",
                row.id,
                row.provider.label()
            ));
        }
        Some(_) => {}
        None => changes.push(Change::CreateThread {
            provider,
            options: options.clone(),
            first_message: text.to_string(),
        }),
    }
    if state.held > 0 {
        changes.push(Change::Unhold);
    }
    // No bridge steers or restarts a turn (`capabilities.rs`), so a plain
    // send starts or queues. A message the policy would steer, or send by
    // restarting the turn, waits in the queue: neither has a change to
    // commit, and the running turn is left as it is.
    let slash = text.trim_start().starts_with('/');
    let caps = capabilities::of(provider);
    changes.push(match resolve_delivery(mode, &state.chat(), &caps, slash) {
        Delivery::Start => Change::Begin {
            provider,
            text: text.to_string(),
            options: options.clone(),
            receipt: command.id.clone(),
            queued: None,
        },
        Delivery::Queue | Delivery::Steer | Delivery::Restart | Delivery::StopThenQueue => {
            Change::Enqueue {
                provider,
                text: text.to_string(),
                options: options.clone(),
            }
        }
    });
    Ok(Decision::Commit(Plan {
        changes,
        text: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{Live, OpenRun};
    use super::*;
    use crate::harness::core::Kind;
    use crate::harness::store::ThreadRow;

    fn thread(provider: Provider) -> ThreadRow {
        ThreadRow {
            id: 4,
            provider,
            provider_session_id: None,
            model: None,
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
            waiting: 0,
            held: 0,
            open: busy.then_some(OpenRun {
                run_id: 9,
                starting: false,
                stop_requested: false,
            }),
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

    #[test]
    fn a_steer_or_a_restart_waits_in_the_queue_while_no_bridge_can_do_either() {
        let options = SendOptions::default();
        let busy = state(Some(thread(Provider::Codex)), true);
        for mode in [Mode::Steer, Mode::Restart, Mode::Queue] {
            let codex = (Provider::Codex, mode);
            let Decision::Commit(plan) =
                decide(&busy, &send(Some(4)), codex, "correction", &options).unwrap()
            else {
                panic!("a send asks the provider nothing");
            };
            assert!(
                matches!(plan.changes.as_slice(), [Change::Enqueue { .. }]),
                "{mode:?}: the running turn is left as it is"
            );
        }
        // On an idle chat any mode is a plain start.
        let idle = state(Some(thread(Provider::Codex)), false);
        let queue = (Provider::Codex, Mode::Queue);
        let Decision::Commit(plan) = decide(&idle, &send(Some(4)), queue, "hi", &options).unwrap()
        else {
            panic!("a send asks the provider nothing");
        };
        assert!(matches!(plan.changes.as_slice(), [Change::Begin { .. }]));
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
    fn another_providers_chat_refuses_the_send() {
        let options = SendOptions::default();
        let refused = decide(
            &state(Some(thread(Provider::Claude)), false),
            &send(Some(4)),
            (Provider::Codex, Mode::Auto),
            "hello",
            &options,
        );
        assert_eq!(refused.err().unwrap(), "thread 4 is a Claude Code thread");
    }
}
