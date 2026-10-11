//! Transactions and a minimal editor state: a document and a selection, after
//! `@codemirror/state` 6.7.6's `Transaction` and `EditorState` (MIT, Marijn
//! Haverbeke; see `NOTICE`).
//! A transaction carries the few annotations the history reads — user event,
//! `addToHistory`, `isolateHistory` and the time — and nothing else; the
//! extension system (facets, effects, fields) is out of scope.

use super::Text;
use super::change::{ChangeError, ChangeSet, ChangeSpec};
use super::selection::{Selection, SelectionError, SelectionRange};

/// `isolateHistory`: which sides of a transaction may not join a history
/// event with its neighbours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolate {
    Before,
    After,
    Full,
}

/// A change to a `State`.
#[derive(Debug, Clone)]
pub struct Transaction {
    /// Over the start document.
    pub changes: ChangeSet,
    /// The new selection, in the new document's coordinates; `None` maps the
    /// start selection through `changes`.
    pub selection: Option<Selection>,
    /// Private so that `""` is stored as `None`, as CodeMirror treats it.
    user_event: Option<String>,
    /// `false` keeps the change out of the undo history (it is mapped over
    /// instead), as `Transaction.addToHistory.of(false)`.
    pub add_to_history: bool,
    pub isolate: Option<Isolate>,
    /// Milliseconds, as `Transaction.time`. A parameter, never the clock, so
    /// history grouping replays deterministically.
    pub time: i64,
}

impl Transaction {
    /// A transaction applying `changes` at `time`, with no annotations.
    pub fn new(changes: ChangeSet, time: i64) -> Self {
        Transaction {
            changes,
            selection: None,
            user_event: None,
            add_to_history: true,
            isolate: None,
            time,
        }
    }

    pub fn with_selection(mut self, selection: Selection) -> Self {
        self.selection = Some(selection);
        self
    }

    /// `"input.type"`, `"delete.backward"`, `"select"`…; `""` means none.
    pub fn with_user_event(mut self, event: &str) -> Self {
        self.set_user_event(Some(event));
        self
    }

    pub fn set_user_event(&mut self, event: Option<&str>) {
        self.user_event = event.filter(|e| !e.is_empty()).map(str::to_owned);
    }

    pub fn user_event(&self) -> Option<&str> {
        self.user_event.as_deref()
    }

    pub fn with_add_to_history(mut self, add: bool) -> Self {
        self.add_to_history = add;
        self
    }

    pub fn with_isolate(mut self, isolate: Isolate) -> Self {
        self.isolate = Some(isolate);
        self
    }

    /// Whether the user event is `event` or starts with `event` and a dot.
    pub fn is_user_event(&self, event: &str) -> bool {
        self.user_event.as_deref().is_some_and(|e| {
            e == event
                || (e.len() > event.len()
                    && e.starts_with(event)
                    && e.as_bytes()[event.len()] == b'.')
        })
    }
}

/// A refused transaction or spec.
#[derive(Debug, Clone, PartialEq)]
pub enum StateError {
    Change(ChangeError),
    Selection(SelectionError),
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::Change(e) => e.fmt(f),
            StateError::Selection(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for StateError {}

impl From<ChangeError> for StateError {
    fn from(e: ChangeError) -> Self {
        StateError::Change(e)
    }
}

impl From<SelectionError> for StateError {
    fn from(e: SelectionError) -> Self {
        StateError::Selection(e)
    }
}

/// What `change_by_range`'s callback returns for one range: changes in the
/// start document's coordinates, and the range in the coordinates after them.
pub struct RangeChange {
    pub changes: Vec<ChangeSpec>,
    pub range: SelectionRange,
}

/// A document and a selection (multiple ranges allowed, as with the app's
/// `drawSelection`).
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub doc: Text,
    pub selection: Selection,
}

impl State {
    /// `doc` with a cursor at 0.
    pub fn new(doc: Text) -> Self {
        State {
            doc,
            selection: Selection::single(0, 0),
        }
    }

    /// `doc` with `selection`, refused if it reaches past the document.
    pub fn with_selection(doc: Text, selection: Selection) -> Result<Self, StateError> {
        selection.check(doc.len())?;
        Ok(State { doc, selection })
    }

    /// The change set for `specs` on this document.
    pub fn changes(&self, specs: &[ChangeSpec]) -> Result<ChangeSet, ChangeError> {
        ChangeSet::of(specs, self.doc.len())
    }

    /// The state after `tr`. The start selection maps with assoc -1.
    pub fn apply(&self, tr: &Transaction) -> Result<State, StateError> {
        let doc = tr.changes.apply(&self.doc)?;
        let selection = match &tr.selection {
            Some(selection) => {
                selection.check(doc.len())?;
                selection.clone()
            }
            None => self.selection.map(&tr.changes, -1),
        };
        Ok(State { doc, selection })
    }

    /// Runs `f` on each range and combines the results, as CodeMirror's
    /// `changeByRange`: each range's changes are in the start document's
    /// coordinates, and every returned range is mapped through the changes
    /// made for the other ranges.
    pub fn change_by_range(
        &self,
        mut f: impl FnMut(&SelectionRange) -> RangeChange,
    ) -> Result<(ChangeSet, Selection), ChangeError> {
        let ranges = self.selection.ranges();
        let first = f(&ranges[0]);
        let mut changes = self.changes(&first.changes)?;
        let mut out = vec![first.range];
        for range in &ranges[1..] {
            let result = f(range);
            let new_changes = self.changes(&result.changes)?;
            let new_mapped = new_changes.map(&changes, false);
            for r in &mut out {
                *r = r.map(&new_mapped, -1);
            }
            let map_by = changes.map_desc(&new_changes, true);
            out.push(result.range.map(&map_by, -1));
            changes = changes.compose(&new_mapped);
        }
        let selection = Selection::create(out, self.selection.main_index())
            .expect("one range per selection range");
        Ok((changes, selection))
    }

    /// Replaces every range with `text`, leaving a cursor (assoc -1) after
    /// each insertion. Refused, as in CodeMirror, when a range has `from > to`
    /// (which mapping a range over a replacement inside it can produce).
    pub fn replace_selection(&self, text: &str) -> Result<(ChangeSet, Selection), ChangeError> {
        let text = Text::of(text);
        self.change_by_range(|range| RangeChange {
            changes: vec![ChangeSpec::Replace {
                from: range.from(),
                to: range.to(),
                insert: text.clone(),
            }],
            range: SelectionRange::cursor(range.from() + text.len(), -1, None, None),
        })
    }
}
