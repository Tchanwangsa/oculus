//! How a message reaches a chat that may be mid-turn: started, queued,
//! steered into the running turn, or sent by stopping that turn and
//! starting again. One pure function of what the chat is doing and what its
//! bridge can do (`providers/capabilities.rs`), so every surface gives the
//! same answer and each branch has a test.

use crate::harness::providers::capabilities::Capabilities;

/// What the student asked for with the send, as `harness_send` takes it.
#[derive(serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// The composer's plain send: whatever the chat's default is.
    #[default]
    Auto,
    /// Wait behind the running turn.
    Queue,
    /// Into the running turn.
    Steer,
    /// Stop the running turn and start again with this.
    Restart,
}

/// What becomes of the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// Its own run, now.
    Start,
    /// A waiting run, behind what is running or already waits.
    Queue,
    /// A message written into the running turn; the run goes on.
    Steer,
    /// The running turn is interrupted and the run restarted with this.
    Restart,
    /// The bridge cannot restart a turn: the turn is stopped and the
    /// message waits first in line, as its own run.
    StopThenQueue,
}

/// The run that has the chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Active {
    /// Saved and not yet with its provider: there is no turn to steer.
    pub starting: bool,
    /// The student already stopped it.
    pub stopping: bool,
}

/// What the chat is doing when the message arrives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chat {
    pub active: Option<Active>,
    /// Something no run stands for holds the chat: a turn its CLI started
    /// itself, a rewind waiting on the provider.
    pub held: bool,
    /// Messages already wait in its queue.
    pub waiting: bool,
}

/// Decide how `mode` is delivered. `command` is a message that is a slash
/// command, which is never written into a running turn.
pub fn resolve_delivery(mode: Mode, chat: &Chat, caps: &Capabilities, command: bool) -> Delivery {
    let Some(active) = chat.active else {
        // Nothing to steer or restart. A turn of the CLI's own has no run to
        // attach a message to, and a queue goes out in the order it was
        // typed, so either makes the message wait.
        return match chat.held || chat.waiting {
            true => Delivery::Queue,
            false => Delivery::Start,
        };
    };
    // A run that is being stopped takes nothing more, and a command is
    // never folded into a turn.
    if mode == Mode::Queue || command || active.stopping {
        return Delivery::Queue;
    }
    match mode {
        Mode::Restart if caps.steer_by_restart && !active.starting => Delivery::Restart,
        Mode::Restart => Delivery::StopThenQueue,
        // No provider turn exists yet for a run that is still starting.
        _ if active.starting => Delivery::Queue,
        _ if caps.active_steering => Delivery::Steer,
        Mode::Steer if caps.steer_by_restart => Delivery::Restart,
        // A bridge that can neither steer nor restart queues, and a plain
        // send never restarts a turn by itself.
        _ => Delivery::Queue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::providers::capabilities;
    use crate::harness::Provider;

    fn caps(active_steering: bool, steer_by_restart: bool) -> Capabilities {
        Capabilities {
            active_steering,
            steer_by_restart,
            ..capabilities::of(Provider::Codex)
        }
    }

    fn running() -> Chat {
        Chat {
            active: Some(Active {
                starting: false,
                stopping: false,
            }),
            ..Chat::default()
        }
    }

    const MODES: [Mode; 4] = [Mode::Auto, Mode::Queue, Mode::Steer, Mode::Restart];

    #[test]
    fn an_idle_chat_starts_whatever_was_asked_and_a_held_or_waiting_one_queues() {
        let all = caps(true, true);
        for mode in MODES {
            assert_eq!(
                resolve_delivery(mode, &Chat::default(), &all, false),
                Delivery::Start,
                "{mode:?}: a finished target is a normal start"
            );
            for chat in [
                Chat {
                    held: true,
                    ..Chat::default()
                },
                Chat {
                    waiting: true,
                    ..Chat::default()
                },
            ] {
                assert_eq!(resolve_delivery(mode, &chat, &all, false), Delivery::Queue);
            }
        }
    }

    #[test]
    fn a_plain_send_steers_where_the_bridge_can_and_queues_where_it_cannot() {
        let chat = running();
        assert_eq!(
            resolve_delivery(Mode::Auto, &chat, &caps(true, false), false),
            Delivery::Steer
        );
        assert_eq!(
            resolve_delivery(Mode::Auto, &chat, &caps(true, true), false),
            Delivery::Steer
        );
        // It never restarts a turn by itself, even where that is all the
        // bridge offers.
        assert_eq!(
            resolve_delivery(Mode::Auto, &chat, &caps(false, true), false),
            Delivery::Queue
        );
        assert_eq!(
            resolve_delivery(Mode::Auto, &chat, &caps(false, false), false),
            Delivery::Queue
        );
    }

    #[test]
    fn an_explicit_steer_falls_back_to_a_restart_then_to_the_queue() {
        let chat = running();
        let steer = |caps| resolve_delivery(Mode::Steer, &chat, &caps, false);
        assert_eq!(steer(caps(true, true)), Delivery::Steer);
        assert_eq!(steer(caps(false, true)), Delivery::Restart);
        assert_eq!(steer(caps(false, false)), Delivery::Queue);
    }

    #[test]
    fn an_explicit_restart_stops_and_queues_where_the_bridge_cannot_restart() {
        let chat = running();
        let restart = |caps| resolve_delivery(Mode::Restart, &chat, &caps, false);
        assert_eq!(restart(caps(false, true)), Delivery::Restart);
        assert_eq!(restart(caps(true, true)), Delivery::Restart);
        assert_eq!(restart(caps(true, false)), Delivery::StopThenQueue);
        assert_eq!(restart(caps(false, false)), Delivery::StopThenQueue);
    }

    #[test]
    fn a_run_with_no_turn_yet_is_not_steered_and_one_being_stopped_takes_nothing() {
        let all = caps(true, true);
        let starting = Chat {
            active: Some(Active {
                starting: true,
                stopping: false,
            }),
            ..Chat::default()
        };
        assert_eq!(
            resolve_delivery(Mode::Auto, &starting, &all, false),
            Delivery::Queue
        );
        assert_eq!(
            resolve_delivery(Mode::Steer, &starting, &all, false),
            Delivery::Queue
        );
        assert_eq!(
            resolve_delivery(Mode::Restart, &starting, &all, false),
            Delivery::StopThenQueue
        );
        let stopping = Chat {
            active: Some(Active {
                starting: false,
                stopping: true,
            }),
            ..Chat::default()
        };
        for mode in MODES {
            assert_eq!(
                resolve_delivery(mode, &stopping, &all, false),
                Delivery::Queue,
                "{mode:?}: nothing is sent into a stopped turn"
            );
        }
    }

    #[test]
    fn a_slash_command_and_an_explicit_queue_never_enter_a_running_turn() {
        let (chat, all) = (running(), caps(true, true));
        for mode in MODES {
            assert_eq!(resolve_delivery(mode, &chat, &all, true), Delivery::Queue);
        }
        assert_eq!(
            resolve_delivery(Mode::Queue, &chat, &all, false),
            Delivery::Queue
        );
    }

    /// No bridge steers yet, so a plain send can only start or queue on any
    /// of them, whatever the chat is doing.
    #[test]
    fn every_bridge_starts_or_queues_a_plain_send_today() {
        let chats = [
            Chat::default(),
            running(),
            Chat {
                waiting: true,
                ..Chat::default()
            },
        ];
        for caps in capabilities::all() {
            for chat in &chats {
                let delivery = resolve_delivery(Mode::Auto, chat, &caps, false);
                assert!(
                    matches!(delivery, Delivery::Start | Delivery::Queue),
                    "{:?}: {delivery:?}",
                    caps.provider
                );
            }
        }
    }
}
