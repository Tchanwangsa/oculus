//! The queue's commands. Whether a message still waits is checked by the
//! commit itself, on the row: a message that went out a moment ago is
//! simply no longer there to change, which is no error for the student.

use super::super::commit::Change;
use super::super::state::State;
use super::{Decision, Plan};

fn plan(state: &State, change: Change) -> Result<Decision, String> {
    if state.thread.is_none() {
        return Err("the queue is a thread's".into());
    }
    Ok(Decision::Commit(Plan {
        changes: vec![change],
        text: None,
    }))
}

pub(super) fn edit(state: &State, run_id: i64, text: &str) -> Result<Decision, String> {
    if text.trim().is_empty() {
        return Err("a waiting message cannot be emptied; remove it instead".into());
    }
    let text = text.to_string();
    plan(state, Change::EditQueued { run_id, text })
}

pub(super) fn remove(state: &State, run_id: i64) -> Result<Decision, String> {
    let failed = false;
    plan(state, Change::CloseQueued { run_id, failed })
}

pub(super) fn reorder(state: &State, order: &[i64]) -> Result<Decision, String> {
    let order = order.to_vec();
    plan(state, Change::ReorderQueued { order })
}

/// Stop marks the run and holds the queue; the driver then asks the
/// provider. Stop names the run the student saw: when that run is over, or
/// another has taken its place, nothing is touched, not even the queue. A
/// stop that names none takes whatever turn is open, the CLI's own included.
pub(super) fn stop(state: &State, run_id: Option<i64>) -> Result<Decision, String> {
    if state.thread.is_none() {
        return Err("a stop names a thread".into());
    }
    let open = state
        .open
        .filter(|open| run_id.is_none_or(|id| id == open.run_id));
    let changes = match open {
        // Already stopping: a second click is the same stop.
        Some(open) if open.stop_requested => Vec::new(),
        Some(open) => vec![
            Change::RequestStop {
                run_id: open.run_id,
            },
            Change::HoldQueue,
        ],
        None if run_id.is_none() && state.live.own_turn => vec![Change::HoldQueue],
        None => Vec::new(),
    };
    Ok(Decision::Commit(Plan {
        changes,
        text: None,
    }))
}

/// Force stop is a stop that does not wait for the CLI: the run is marked
/// and the queue held, if a stop had not done so already, and the consumer
/// then ends the session. It names its run, and one that is over is left
/// alone.
pub(super) fn force_stop(state: &State, run_id: i64) -> Result<Decision, String> {
    if state.thread.is_none() {
        return Err("a stop names a thread".into());
    }
    let changes = match state.open.filter(|open| open.run_id == run_id) {
        Some(open) if !open.stop_requested => {
            vec![Change::RequestStop { run_id }, Change::HoldQueue]
        }
        _ => Vec::new(),
    };
    Ok(Decision::Commit(Plan {
        changes,
        text: None,
    }))
}

/// Resume lifts the hold; the driver starts the first waiting message when
/// nothing is open.
pub(super) fn resume(state: &State) -> Result<Decision, String> {
    plan(state, Change::Unhold)
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{Live, OpenRun};
    use super::*;
    use crate::harness::store::ThreadRow;
    use crate::harness::Provider;

    fn state(open: Option<i64>, own_turn: bool) -> State {
        State {
            thread: Some(ThreadRow {
                id: 4,
                provider: Provider::Claude,
                model: None,
                subject_code: None,
                lecture: None,
            }),
            live: Live {
                held: own_turn,
                own_turn,
            },
            question: None,
            newest_anchor: None,
            session: None,
            waiting: 0,
            held: 0,
            open: open.map(|run_id| OpenRun {
                run_id,
                starting: false,
                stop_requested: false,
            }),
        }
    }

    fn changes(state: &State, run_id: Option<i64>) -> Vec<Change> {
        match stop(state, run_id).unwrap() {
            Decision::Commit(plan) => plan.changes,
            Decision::Ask(_) => panic!("a stop asks the provider nothing before its commit"),
        }
    }

    #[test]
    fn stop_marks_the_run_it_names_and_holds_the_queue() {
        let marked = changes(&state(Some(7), false), Some(7));
        assert!(matches!(
            marked.as_slice(),
            [Change::RequestStop { run_id: 7 }, Change::HoldQueue]
        ));
        let whatever = changes(&state(Some(7), false), None);
        assert!(matches!(
            whatever.as_slice(),
            [Change::RequestStop { run_id: 7 }, Change::HoldQueue]
        ));
    }

    #[test]
    fn a_second_stop_of_a_run_already_stopping_changes_nothing() {
        let mut stopping = state(Some(7), false);
        stopping.open = Some(OpenRun {
            run_id: 7,
            starting: false,
            stop_requested: true,
        });
        assert!(changes(&stopping, Some(7)).is_empty());
    }

    #[test]
    fn force_stop_marks_a_run_no_stop_reached_and_leaves_any_other_alone() {
        let Ok(Decision::Commit(plan)) = force_stop(&state(Some(7), false), 7) else {
            panic!("a force stop commits");
        };
        assert!(matches!(
            plan.changes.as_slice(),
            [Change::RequestStop { run_id: 7 }, Change::HoldQueue]
        ));
        for other in [state(Some(8), false), state(None, false)] {
            let Ok(Decision::Commit(plan)) = force_stop(&other, 7) else {
                panic!("a force stop commits");
            };
            assert!(plan.changes.is_empty());
        }
    }

    #[test]
    fn stop_for_a_run_that_is_over_touches_nothing() {
        assert!(changes(&state(None, false), Some(7)).is_empty());
        // The next run has started meanwhile: it is not the one meant.
        assert!(changes(&state(Some(8), false), Some(7)).is_empty());
        // A turn of the CLI's own is not the run it named either.
        assert!(changes(&state(None, true), Some(7)).is_empty());
    }

    #[test]
    fn a_stop_that_names_no_run_reaches_a_turn_the_cli_started() {
        let own = changes(&state(None, true), None);
        assert!(matches!(own.as_slice(), [Change::HoldQueue]));
        assert!(changes(&state(None, false), None).is_empty());
    }
}
