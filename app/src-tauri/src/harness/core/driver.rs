//! The driver: what a chat's saved runs say should happen next. After every
//! commit and every turn's end the consumer reads the chat's runs
//! ([`read`]) and takes the one [`Step`] they imply. It goes by what is
//! saved, never by a notification, so a waiting run cannot be skipped and
//! none is started twice: asking again gives the same answer until the
//! step has changed the rows.

use sqlx::SqlitePool;

use crate::harness::store::queue::{self, Open, Waiting};
use crate::harness::Provider;

use super::effect::Start;

/// A chat's runs as they are saved.
pub(in crate::harness) struct Saved {
    /// The run going out or being answered.
    pub open: Option<Open>,
    /// The first waiting run that is not held.
    pub next: Option<Waiting>,
}

pub(in crate::harness) async fn read(pool: &SqlitePool, thread_id: i64) -> Result<Saved, String> {
    Ok(Saved {
        open: queue::open(pool, thread_id).await?,
        next: queue::next(pool, thread_id).await?,
    })
}

/// What the consumer holds in memory about the chat's provider calls: none
/// of it outlives the process, and neither does a provider call.
#[derive(Clone, Copy, Default)]
pub(in crate::harness) struct Calls {
    /// The open run was already handed to its start step.
    pub started: bool,
    /// Something no row shows holds the chat: a turn the CLI started
    /// itself, a rewind waiting on the provider, a settle that has not
    /// landed.
    pub held: bool,
}

pub(in crate::harness) enum Step {
    /// Hand the `starting` run to its start step.
    Start(Box<Start>),
    /// Nothing is open: the first waiting run starts.
    Promote(Box<Waiting>),
    /// Nothing to do until a row changes.
    Wait,
}

pub(in crate::harness) fn step(
    thread_id: i64,
    provider: Provider,
    saved: Saved,
    calls: Calls,
) -> Step {
    match (saved.open, saved.next) {
        (Some(open), _) if open.starting && !calls.started => Step::Start(Box::new(Start {
            thread_id,
            provider,
            run: open.run,
            text: open.text,
            options: open.options,
            respawn: open.respawn,
        })),
        (Some(_), _) => Step::Wait,
        (None, Some(next)) if !calls.held => Step::Promote(Box::new(next)),
        (None, _) => Step::Wait,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::store::RunStart;
    use crate::harness::SendOptions;

    fn open(starting: bool) -> Option<Open> {
        Some(Open {
            run: RunStart {
                run_id: 7,
                attempt_id: 8,
                binding_id: 1,
                item_id: 3,
            },
            starting,
            text: "asked".into(),
            options: SendOptions::default(),
            respawn: false,
        })
    }

    fn next() -> Option<Waiting> {
        Some(Waiting {
            run_id: 9,
            text: "waits".into(),
            options: SendOptions::default(),
        })
    }

    fn calls(started: bool, held: bool) -> Calls {
        Calls { started, held }
    }

    fn take(open: Option<Open>, next: Option<Waiting>, calls: Calls) -> Step {
        step(4, Provider::Codex, Saved { open, next }, calls)
    }

    #[test]
    fn a_starting_run_is_handed_to_its_start_step_once() {
        let Step::Start(start) = take(open(true), next(), calls(false, false)) else {
            panic!("the starting run goes out");
        };
        assert_eq!((start.thread_id, start.run.run_id), (4, 7));
        assert_eq!(start.text, "asked");
        assert!(matches!(
            take(open(true), next(), calls(true, false)),
            Step::Wait
        ));
    }

    #[test]
    fn nothing_starts_behind_an_open_run_or_a_held_chat() {
        assert!(matches!(
            take(open(false), next(), calls(true, false)),
            Step::Wait
        ));
        assert!(matches!(take(None, next(), calls(false, true)), Step::Wait));
        assert!(matches!(take(None, None, calls(false, false)), Step::Wait));
    }

    #[test]
    fn the_first_waiting_run_starts_when_nothing_is_open() {
        let Step::Promote(waiting) = take(None, next(), calls(false, false)) else {
            panic!("the waiting run is next");
        };
        assert_eq!(waiting.run_id, 9);
    }
}
