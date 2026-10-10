//! Undo history, ported from `@codemirror/commands` 6.11.1's `history()`
//! (MIT, Marijn Haverbeke; see `NOTICE`).
//!
//! Two branches of events, done and undone. Each change event stores the
//! inverse of its changes and the selection before it; consecutive edits join
//! one event when they are adjacent, within `new_group_delay` of each other and
//! typed (`input.type…`/`delete…`) or unlabelled. Selection changes are kept
//! per event (`selections_after`) for `undo_selection`. A change made with
//! `add_to_history: false` is not recorded: both branches are rebased over it
//! (`add_mapping_to_branch`), and events it swallows entirely are dropped.
//!
//! The history is a value beside the `State`: feed it every ordinary
//! transaction with `update`; `undo`/`redo` return the transaction to apply
//! together with the history after it (do not `update` with that transaction).

use std::sync::Arc;

use super::change::{ChangeDesc, ChangeError, ChangeSet};
use super::selection::Selection;
use super::transaction::{Isolate, State, Transaction};

/// A selection-only event keeps at most this many selections, plus one.
const MAX_SELECTIONS_PER_EVENT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryConfig {
    /// Events kept per branch; a branch is trimmed once it is 20 past this.
    pub min_depth: usize,
    /// Milliseconds after which an edit starts a new event.
    pub new_group_delay: i64,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        HistoryConfig {
            min_depth: 100,
            new_group_delay: 500,
        }
    }
}

/// One undoable step. A selection-only event (only at the bottom of a branch)
/// has no changes, mapping or start selection.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEvent {
    /// The inverse of the event's changes: applying it undoes the event.
    pub changes: Option<ChangeSet>,
    /// Mapping from `add_to_history: false` changes that the events below
    /// this one still have to be mapped through.
    pub mapped: Option<ChangeDesc>,
    /// The selection before the event.
    pub start_selection: Option<Selection>,
    /// Selections after the event, for selection undo.
    pub selections_after: Vec<Selection>,
}

impl HistoryEvent {
    fn selection(selections: Vec<Selection>) -> Self {
        HistoryEvent {
            changes: None,
            mapped: None,
            start_selection: None,
            selections_after: selections,
        }
    }

    fn with_selections_after(&self, after: Vec<Selection>) -> Self {
        HistoryEvent {
            selections_after: after,
            ..self.clone()
        }
    }

    /// The event for `tr` on `start`, or `None` when it changes nothing.
    fn from_transaction(
        tr: &Transaction,
        start: &State,
        selection: Option<Selection>,
    ) -> Result<Option<Self>, ChangeError> {
        if tr.changes.is_empty() {
            return Ok(None);
        }
        Ok(Some(HistoryEvent {
            changes: Some(tr.changes.invert(&start.doc)?),
            mapped: None,
            start_selection: Some(selection.unwrap_or_else(|| start.selection.clone())),
            selections_after: Vec::new(),
        }))
    }

    fn has_changes(&self) -> bool {
        self.changes.as_ref().is_some_and(|c| !c.is_empty())
    }
}

type Branch = Vec<Arc<HistoryEvent>>;

/// What `undo`/`redo` return: the transaction and the history after it.
pub type Popped = Result<Option<(Transaction, History)>, ChangeError>;

/// Which branch `pop` takes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Done,
    Undone,
}

/// The undo history. Persistent: every operation returns a new value.
#[derive(Debug, Clone, PartialEq)]
pub struct History {
    done: Branch,
    undone: Branch,
    /// Time of the last recorded transaction; 0 when isolated.
    prev_time: i64,
    prev_user_event: Option<String>,
    config: HistoryConfig,
}

impl Default for History {
    fn default() -> Self {
        History::new(HistoryConfig::default())
    }
}

impl History {
    pub fn new(config: HistoryConfig) -> Self {
        History {
            done: Vec::new(),
            undone: Vec::new(),
            prev_time: 0,
            prev_user_event: None,
            config,
        }
    }

    /// The undo branch, oldest first.
    pub fn done(&self) -> impl Iterator<Item = &HistoryEvent> {
        self.done.iter().map(|e| &**e)
    }

    /// The redo branch, oldest first.
    pub fn undone(&self) -> impl Iterator<Item = &HistoryEvent> {
        self.undone.iter().map(|e| &**e)
    }

    /// The time of the last recorded transaction; 0 after an isolation, an
    /// undo or a redo.
    pub fn prev_time(&self) -> i64 {
        self.prev_time
    }

    /// The user event of the last recorded transaction.
    pub fn prev_user_event(&self) -> Option<&str> {
        self.prev_user_event.as_deref()
    }

    /// The number of undoable change events.
    pub fn undo_depth(&self) -> usize {
        depth(&self.done)
    }

    /// The number of redoable change events.
    pub fn redo_depth(&self) -> usize {
        depth(&self.undone)
    }

    /// The history after `tr`, applied to `start` (not an undo or redo).
    /// Refused when `tr`'s changes are not for `start.doc` (its length, or a
    /// boundary inside a surrogate pair).
    pub fn update(&self, start: &State, tr: &Transaction) -> Result<History, ChangeError> {
        if tr.changes.length() != start.doc.len() {
            return Err(ChangeError::LengthMismatch {
                expected: start.doc.len(),
                got: tr.changes.length(),
            });
        }
        let mut state = self.clone();
        if matches!(tr.isolate, Some(Isolate::Full | Isolate::Before)) {
            state = state.isolate();
        }
        if !tr.add_to_history {
            return Ok(if tr.changes.is_empty() {
                state
            } else {
                state.add_mapping(tr.changes.desc())
            });
        }
        if let Some(event) = HistoryEvent::from_transaction(tr, start, None)? {
            state = state.add_changes(event, tr);
        } else if tr.selection.is_some() {
            state = state.add_selection(start.selection.clone(), tr.time, tr.user_event());
        }
        if matches!(tr.isolate, Some(Isolate::Full | Isolate::After)) {
            state = state.isolate();
        }
        Ok(state)
    }

    /// Undo one event: the transaction to apply to `state` (user event
    /// `"undo"`) and the history after it. `None` when there is nothing to undo;
    /// refused when `state.doc` is not the document this history recorded.
    pub fn undo(&self, state: &State, time: i64) -> Popped {
        self.pop(Side::Done, state, false, time)
    }

    /// Redo one event; see `undo`.
    pub fn redo(&self, state: &State, time: i64) -> Popped {
        self.pop(Side::Undone, state, false, time)
    }

    /// Undo a selection change, or else a change event.
    pub fn undo_selection(&self, state: &State, time: i64) -> Popped {
        self.pop(Side::Done, state, true, time)
    }

    /// Redo a selection change, or else a change event.
    pub fn redo_selection(&self, state: &State, time: i64) -> Popped {
        self.pop(Side::Undone, state, true, time)
    }

    fn with_branches(
        &self,
        done: Branch,
        undone: Branch,
        time: i64,
        event: Option<String>,
    ) -> Self {
        History {
            done,
            undone,
            prev_time: time,
            prev_user_event: event,
            config: self.config,
        }
    }

    fn isolate(self) -> Self {
        if self.prev_time != 0 {
            let (done, undone) = (self.done.clone(), self.undone.clone());
            self.with_branches(done, undone, 0, None)
        } else {
            self
        }
    }

    fn add_changes(&self, event: HistoryEvent, tr: &Transaction) -> Self {
        let time = tr.time;
        let user_event = tr.user_event();
        let mut done = self.done.clone();
        let last = done.last().cloned();
        let joins = last.as_ref().is_some_and(|last| {
            let (Some(last_changes), Some(changes)) = (&last.changes, &event.changes) else {
                return false;
            };
            !last_changes.is_empty()
                && user_event.is_none_or(joinable_user_event)
                && ((last.selections_after.is_empty()
                    && time.saturating_sub(self.prev_time) < self.config.new_group_delay
                    && is_adjacent(last_changes, changes))
                    || user_event == Some("input.type.compose"))
        });
        if joins {
            let last = last.expect("joins checked it");
            let changes = event
                .changes
                .as_ref()
                .expect("joins checked it")
                .compose(last.changes.as_ref().expect("joins checked it"));
            let joined = HistoryEvent {
                changes: Some(changes),
                mapped: last.mapped.clone(),
                start_selection: last.start_selection.clone(),
                selections_after: Vec::new(),
            };
            let to = done.len() - 1;
            done = update_branch(&done, to, self.config.min_depth, joined);
        } else {
            let to = done.len();
            done = update_branch(&done, to, self.config.min_depth, event);
        }
        self.with_branches(done, Vec::new(), time, user_event.map(str::to_owned))
    }

    fn add_selection(&self, selection: Selection, time: i64, user_event: Option<&str>) -> Self {
        let last = self
            .done
            .last()
            .map_or(&[][..], |e| &e.selections_after[..]);
        if let Some(last) = last.last()
            && time.saturating_sub(self.prev_time) < self.config.new_group_delay
            && user_event
                .is_some_and(|e| self.prev_user_event.as_deref() == Some(e) && select_user_event(e))
            && eq_selection_shape(last, &selection)
        {
            return self.clone();
        }
        self.with_branches(
            add_selection(&self.done, selection),
            self.undone.clone(),
            time,
            user_event.map(str::to_owned),
        )
    }

    fn add_mapping(&self, mapping: &ChangeDesc) -> Self {
        self.with_branches(
            add_mapping_to_branch(&self.done, mapping),
            add_mapping_to_branch(&self.undone, mapping),
            self.prev_time,
            self.prev_user_event.clone(),
        )
    }

    fn pop(&self, side: Side, state: &State, only_selection: bool, time: i64) -> Popped {
        let branch = match side {
            Side::Done => &self.done,
            Side::Undone => &self.undone,
        };
        let Some(event) = branch.last() else {
            return Ok(None);
        };
        let selection = match (event.selections_after.first(), &event.start_selection) {
            (Some(first), _) => first.clone(),
            (None, Some(start)) => start.map(
                &event
                    .changes
                    .as_ref()
                    .expect("a change event")
                    .inverted_desc(),
                1,
            ),
            (None, None) => state.selection.clone(),
        };
        let (tr, rest) = if only_selection && !event.selections_after.is_empty() {
            let tr = Transaction::new(ChangeSet::empty(state.doc.len()), time)
                .with_selection(event.selections_after.last().expect("non-empty").clone())
                .with_user_event(match side {
                    Side::Done => "select.undo",
                    Side::Undone => "select.redo",
                });
            (tr, pop_selection(branch))
        } else {
            let Some(changes) = event.changes.as_ref() else {
                return Ok(None);
            };
            let mut rest: Branch = branch[..branch.len() - 1].to_vec();
            if let Some(mapped) = &event.mapped {
                rest = add_mapping_to_branch(&rest, mapped);
            }
            let tr = Transaction::new(changes.clone(), time)
                .with_selection(event.start_selection.clone().expect("a change event"))
                .with_user_event(match side {
                    Side::Done => "undo",
                    Side::Undone => "redo",
                });
            (tr, rest)
        };
        // What the history field does when it sees the transaction.
        let other = match side {
            Side::Done => &self.undone,
            Side::Undone => &self.done,
        };
        if tr.changes.length() != state.doc.len() {
            return Err(ChangeError::LengthMismatch {
                expected: state.doc.len(),
                got: tr.changes.length(),
            });
        }
        let other = match HistoryEvent::from_transaction(&tr, state, Some(selection))? {
            Some(item) => update_branch(other, other.len(), self.config.min_depth, item),
            None => add_selection(other, state.selection.clone()),
        };
        let history = match side {
            Side::Done => self.with_branches(rest, other, 0, None),
            Side::Undone => self.with_branches(other, rest, 0, None),
        };
        Ok(Some((tr, history)))
    }
}

fn depth(branch: &Branch) -> usize {
    match branch.first() {
        Some(first) if first.changes.is_none() => branch.len() - 1,
        _ => branch.len(),
    }
}

/// `input.type` or `delete`, alone or followed by a dot.
fn joinable_user_event(event: &str) -> bool {
    ["input.type", "delete"]
        .iter()
        .any(|p| prefix_event(event, p))
}

fn select_user_event(event: &str) -> bool {
    prefix_event(event, "select")
}

fn prefix_event(event: &str, prefix: &str) -> bool {
    event
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

/// `branch[..to]` plus `event`, dropping the oldest events once the branch is
/// 20 past `max_len` (so it keeps `max_len + 1` and the new one).
fn update_branch(branch: &Branch, to: usize, max_len: usize, event: HistoryEvent) -> Branch {
    let start = if to + 1 > max_len + 20 {
        to - max_len - 1
    } else {
        0
    };
    let mut out = branch[start..to].to_vec();
    out.push(Arc::new(event));
    out
}

/// Whether a changed range of `a` (old side) touches one of `b` (new side).
fn is_adjacent(a: &ChangeDesc, b: &ChangeDesc) -> bool {
    let ranges: Vec<(usize, usize)> = a
        .iter_changed_ranges(false)
        .map(|c| (c.from_a, c.to_a))
        .collect();
    b.iter_changed_ranges(false).any(|c| {
        ranges
            .iter()
            .any(|&(from, to)| c.to_b >= from && c.from_b <= to)
    })
}

/// Same number of ranges, and the same ones empty.
fn eq_selection_shape(a: &Selection, b: &Selection) -> bool {
    a.ranges().len() == b.ranges().len()
        && a.ranges()
            .iter()
            .zip(b.ranges())
            .all(|(x, y)| x.is_empty() == y.is_empty())
}

/// Records `selection` after the top event (or as a selection-only event in
/// an empty branch), unless it equals the last one recorded.
fn add_selection(branch: &Branch, selection: Selection) -> Branch {
    let Some(last) = branch.last() else {
        return vec![Arc::new(HistoryEvent::selection(vec![selection]))];
    };
    let after = &last.selections_after;
    let mut sels = after[after.len().saturating_sub(MAX_SELECTIONS_PER_EVENT)..].to_vec();
    if sels.last().is_some_and(|s| s.eq(&selection, false)) {
        return branch.clone();
    }
    sels.push(selection);
    update_branch(
        branch,
        branch.len() - 1,
        usize::MAX / 2,
        last.with_selections_after(sels),
    )
}

/// The branch with its top event's last selection removed.
fn pop_selection(branch: &Branch) -> Branch {
    let mut out = branch.clone();
    let last = out.last_mut().expect("a non-empty branch");
    let after = &last.selections_after;
    *last = Arc::new(last.with_selections_after(after[..after.len() - 1].to_vec()));
    out
}

/// Maps the branch over `mapping` from the top down. An event whose changes
/// map away entirely is dropped, its selections carried down, and the
/// mapping continues below it composed with what it stored.
fn add_mapping_to_branch(branch: &Branch, mapping: &ChangeDesc) -> Branch {
    let mut mapping = mapping.clone();
    let mut length = branch.len();
    let mut selections: Vec<Selection> = Vec::new();
    while length > 0 {
        let event = map_event(&branch[length - 1], &mapping, selections);
        if event.has_changes() {
            let mut out = branch[..length].to_vec();
            out[length - 1] = Arc::new(event);
            return out;
        }
        length -= 1;
        selections = event.selections_after;
        match event.mapped {
            Some(mapped) => mapping = mapped,
            // Only a selection-only event lacks a mapping, and it is the
            // bottom of the branch.
            None => break,
        }
    }
    if selections.is_empty() {
        Vec::new()
    } else {
        vec![Arc::new(HistoryEvent::selection(selections))]
    }
}

fn map_event(event: &HistoryEvent, mapping: &ChangeDesc, extra: Vec<Selection>) -> HistoryEvent {
    let mut selections: Vec<Selection> = event
        .selections_after
        .iter()
        .map(|s| s.map(mapping, -1))
        .collect();
    selections.extend(extra);
    let Some(changes) = &event.changes else {
        return HistoryEvent::selection(selections);
    };
    let mapped_changes = changes.map(mapping, false);
    let before = mapping.map_desc(changes.desc(), true);
    let full_mapping = match &event.mapped {
        Some(mapped) => mapped.compose_desc(&before),
        None => before.clone(),
    };
    HistoryEvent {
        changes: Some(mapped_changes),
        mapped: Some(full_mapping),
        start_selection: event.start_selection.as_ref().map(|s| s.map(&before, -1)),
        selections_after: selections,
    }
}
