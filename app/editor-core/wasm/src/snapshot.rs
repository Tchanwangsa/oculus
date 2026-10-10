//! One editor state as the mirror sees it: document, selection, undo history
//! and parse tree. A value: every step returns a new snapshot and leaves this
//! one as it was, and a clone shares the document, the history's events and
//! the tree.

use std::sync::Arc;

use oculus_editor_core::markdown::{self, NodeType, Tree};
use oculus_editor_core::text::{ChangeSet, History, Popped, State, Text, Transaction};

use crate::json::{self, millis};

#[derive(Clone)]
pub struct Snapshot {
    state: State,
    history: History,
    tree: Arc<Tree>,
    /// The changes that led here from the previous snapshot; `None` for a
    /// seed.
    changes: Option<ChangeSet>,
}

/// Which history command `pop` runs.
#[derive(Clone, Copy)]
pub enum Command {
    Undo,
    Redo,
    UndoSelection,
    RedoSelection,
}

impl Snapshot {
    /// The state of `doc` with `selection` (`EditorSelection` JSON) and the
    /// history from `history` (`historyField.toJSON`, plus optionally
    /// `prevTime` and `prevUserEvent`), or an empty one.
    pub fn seed(doc: &str, selection: &str, history: Option<&str>) -> Result<Snapshot, String> {
        let doc = Text::of(doc);
        let selection = json::selection(selection)?;
        let state = State::with_selection(doc, selection).map_err(|e| e.to_string())?;
        let history = match history {
            Some(h) => json::history(h)?,
            None => History::default(),
        };
        history
            .check(state.doc.len())
            .map_err(|e| format!("history does not fit the document: {e}"))?;
        let tree = Arc::new(markdown::parse(&state.doc));
        Ok(Snapshot {
            state,
            history,
            tree,
            changes: None,
        })
    }

    /// The snapshot after a transaction: `changes` (`ChangeSet` JSON) over
    /// this document, the selection it sets (`None` maps this one), its
    /// user event, `addToHistory`, `isolateHistory` and time.
    pub fn apply(
        &self,
        changes: &str,
        selection: Option<&str>,
        user_event: Option<&str>,
        add_to_history: bool,
        isolate: Option<&str>,
        time: f64,
    ) -> Result<Snapshot, String> {
        let mut tr = Transaction::new(json::change_set(changes)?, millis(time)?)
            .with_add_to_history(add_to_history);
        tr.selection = selection.map(json::selection).transpose()?;
        tr.set_user_event(user_event);
        tr.isolate = isolate.map(json::isolate).transpose()?;
        let history = self
            .history
            .update(&self.state, &tr)
            .map_err(|e| e.to_string())?;
        self.next(tr, history)
    }

    /// The snapshot after the history command, or `None` when it has
    /// nothing to undo or redo.
    pub fn pop(&self, command: Command, time: f64) -> Result<Option<Snapshot>, String> {
        let (state, time) = (&self.state, millis(time)?);
        let popped: Popped = match command {
            Command::Undo => self.history.undo(state, time),
            Command::Redo => self.history.redo(state, time),
            Command::UndoSelection => self.history.undo_selection(state, time),
            Command::RedoSelection => self.history.redo_selection(state, time),
        };
        match popped.map_err(|e| e.to_string())? {
            Some((tr, history)) => self.next(tr, history).map(Some),
            None => Ok(None),
        }
    }

    fn next(&self, tr: Transaction, history: History) -> Result<Snapshot, String> {
        let state = self.state.apply(&tr).map_err(|e| e.to_string())?;
        let tree = Arc::new(markdown::reparse(&self.tree, &state.doc, &tr.changes));
        Ok(Snapshot {
            state,
            history,
            tree,
            changes: Some(tr.changes),
        })
    }

    pub fn len(&self) -> usize {
        self.state.doc.len()
    }

    pub fn is_empty(&self) -> bool {
        self.state.doc.is_empty()
    }

    pub fn text(&self) -> String {
        self.state.doc.to_string()
    }

    pub fn slice(&self, from: usize, to: usize) -> Result<String, String> {
        self.state
            .doc
            .slice_string(from, to)
            .map_err(|e| e.to_string())
    }

    pub fn selection_json(&self) -> String {
        json::selection_json(&self.state.selection)
    }

    pub fn undo_depth(&self) -> usize {
        self.history.undo_depth()
    }

    pub fn redo_depth(&self) -> usize {
        self.history.redo_depth()
    }

    pub fn history_json(&self) -> String {
        json::history_json(&self.history)
    }

    /// `ChangeSet.toJSON` of the changes that led here; `None` for a seed.
    pub fn changes_json(&self) -> Option<String> {
        self.changes.as_ref().map(json::change_set_json)
    }

    /// The changed ranges of those changes, adjacent ones joined, as
    /// `fromA, toA, fromB, toB` per range (`iterChangedRanges`); empty for
    /// a seed.
    pub fn changed_ranges(&self) -> Vec<u32> {
        let Some(changes) = &self.changes else {
            return Vec::new();
        };
        changes
            .iter_changed_ranges(false)
            .flat_map(|c| [c.from_a, c.to_a, c.from_b, c.to_b].map(|p| p as u32))
            .collect()
    }

    /// The parse tree as `type id, from, to` per node in pre-order.
    pub fn tree(&self) -> Vec<u32> {
        self.tree.triples()
    }
}

/// Lezer's node names, indexed by type id.
pub fn node_names() -> Vec<String> {
    NodeType::ALL.iter().map(|t| t.name().to_owned()).collect()
}
