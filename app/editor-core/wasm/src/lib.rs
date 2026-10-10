//! The editor core in WebAssembly, for mirroring CodeMirror in the browser
//! ("shadow mode"): `Shadow` is one CodeMirror state's document, selection,
//! undo history and parse tree, built from CodeMirror's JSON forms. Every
//! step returns a new `Shadow` and leaves the receiver as it was, so a
//! `StateField` can hold one per state, including states that are computed
//! and dropped.
//!
//! Positions are UTF-16 units and times JavaScript milliseconds (whole
//! numbers in a `number`). Refusals become exceptions; nothing panics across
//! the boundary. The logic is in `Snapshot`, testable natively; this file only
//! wraps it. Build with `bun run editor-wasm` from `app/`.

// No `forbid(unsafe_code)` here: the `wasm_bindgen` macros expand to unsafe code.

mod json;
mod snapshot;
#[cfg(test)]
mod tests;

pub use snapshot::{Command, Snapshot, node_names};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Shadow(Snapshot);

fn js(result: Result<Snapshot, String>) -> Result<Shadow, JsError> {
    result.map(Shadow).map_err(|e| JsError::new(&e))
}

impl Shadow {
    fn pop(&self, command: Command, time: f64) -> Result<Option<Shadow>, JsError> {
        match self.0.pop(command, time) {
            Ok(next) => Ok(next.map(Shadow)),
            Err(e) => Err(JsError::new(&e)),
        }
    }
}

#[wasm_bindgen]
impl Shadow {
    /// The state of `doc` with `selection` (`EditorSelection` JSON, whose
    /// ranges may also carry `goalColumn`, `bidiLevel`, `assoc`, `from` and
    /// `to`) and the undo history `historyField.toJSON` gave (its selections
    /// as rich, and optionally a live history's `prevTime` and
    /// `prevUserEvent`), or an empty one.
    pub fn seed(doc: &str, selection: &str, history: Option<String>) -> Result<Shadow, JsError> {
        js(Snapshot::seed(doc, selection, history.as_deref()))
    }

    /// The state after a transaction: its `changes.toJSON()`, the selection
    /// it set (omitted: mapped), user event, `addToHistory`,
    /// `isolateHistory` ("before", "after" or "full") and time.
    pub fn apply(
        &self,
        changes: &str,
        selection: Option<String>,
        user_event: Option<String>,
        add_to_history: bool,
        isolate: Option<String>,
        time: f64,
    ) -> Result<Shadow, JsError> {
        js(self.0.apply(
            changes,
            selection.as_deref(),
            user_event.as_deref(),
            add_to_history,
            isolate.as_deref(),
            time,
        ))
    }

    /// The state after `undo`; `undefined` when there is nothing to undo.
    pub fn undo(&self, time: f64) -> Result<Option<Shadow>, JsError> {
        self.pop(Command::Undo, time)
    }

    pub fn redo(&self, time: f64) -> Result<Option<Shadow>, JsError> {
        self.pop(Command::Redo, time)
    }

    #[wasm_bindgen(js_name = undoSelection)]
    pub fn undo_selection(&self, time: f64) -> Result<Option<Shadow>, JsError> {
        self.pop(Command::UndoSelection, time)
    }

    #[wasm_bindgen(js_name = redoSelection)]
    pub fn redo_selection(&self, time: f64) -> Result<Option<Shadow>, JsError> {
        self.pop(Command::RedoSelection, time)
    }

    /// The document's length.
    pub fn length(&self) -> usize {
        self.0.len()
    }

    pub fn text(&self) -> String {
        self.0.text()
    }

    pub fn slice(&self, from: usize, to: usize) -> Result<String, JsError> {
        self.0.slice(from, to).map_err(|e| JsError::new(&e))
    }

    /// `selection.toJSON()`, as a string.
    #[wasm_bindgen(js_name = selectionJson)]
    pub fn selection_json(&self) -> String {
        self.0.selection_json()
    }

    #[wasm_bindgen(js_name = undoDepth)]
    pub fn undo_depth(&self) -> usize {
        self.0.undo_depth()
    }

    #[wasm_bindgen(js_name = redoDepth)]
    pub fn redo_depth(&self) -> usize {
        self.0.redo_depth()
    }

    /// `historyField.toJSON()`, as a string.
    #[wasm_bindgen(js_name = historyJson)]
    pub fn history_json(&self) -> String {
        self.0.history_json()
    }

    /// `changes.toJSON()` of the step that made this state (for an undo,
    /// the changes the history chose), as a string; `undefined` for a seed.
    #[wasm_bindgen(js_name = changesJson)]
    pub fn changes_json(&self) -> Option<String> {
        self.0.changes_json()
    }

    /// That step's changed ranges, `fromA, toA, fromB, toB` each, adjacent
    /// ones joined (`iterChangedRanges`).
    #[wasm_bindgen(js_name = changedRanges)]
    pub fn changed_ranges(&self) -> Vec<u32> {
        self.0.changed_ranges()
    }

    /// The parse tree, `type id, from, to` per node in pre-order from the
    /// `Document`; `nodeNames()[id]` names a type.
    pub fn tree(&self) -> Vec<u32> {
        self.0.tree()
    }

    /// Lezer's node names, indexed by type id.
    #[wasm_bindgen(js_name = nodeNames)]
    pub fn node_names() -> Vec<String> {
        node_names()
    }
}
