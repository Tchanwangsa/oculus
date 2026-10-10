//! `MathField`: one formula in the visual maths field, `oculus-math-edit`'s
//! `Field` for JS. Every step returns a new `MathField` and leaves the
//! receiver as it was; the field after a command carries that command's
//! `step`. Offsets are UTF-16 units, converted in `crate::boundary`.

// Exported to JS, which never sees `#[must_use]`; wasm-bindgen refuses
// `const fn`s.
#![allow(clippy::must_use_candidate, clippy::missing_const_for_fn)]

use js_sys::{JSON, TypeError};
use oculus_math_edit::{Field, Selection, StopId};
use wasm_bindgen::prelude::*;

use crate::{boundary, render::parse_error};

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
/** What typing does at the head: maths, a `\text{}`-like run, or the
 *  pending `\command` (the view draws it as a chip). */
export type FieldMode = "math" | "text" | "command";

/** One key or input, for `MathField.run` (the edit model's `Command`). */
export type FieldCommand =
  /** Typed text: one key's character, or an IME's whole string. */
  | { insert: string }
  /** LaTeX whose `#0` takes the selection and whose `#?` are empty slots. */
  | { template: string }
  /** Pasted LaTeX, inserted only when the result renders. */
  | { paste: string }
  | "backspace"
  | "delete"
  /** ⌘Backspace: the caret's row or cell up to the caret. */
  | "deleteLine"
  | { left: { extend?: boolean } }
  | { right: { extend?: boolean } }
  /** ↑ and ↓: each stop's rendered x, by stop id; `NaN` or `null` where
   *  unmeasured. A plain array: a typed array does not stringify as one. */
  | { up: (number | null)[] }
  | { down: (number | null)[] }
  | { home: { extend?: boolean } }
  | { end: { extend?: boolean } }
  | "selectAll"
  | "tab"
  | "shiftTab"
  /** Enter or Shift+Enter. */
  | "enter"
  | "escape";

/** One text change, in UTF-16 units of the source it applies to. */
export interface FieldChange {
  from: number;
  to: number;
  insert: string;
}

/** What the view does beyond showing the new field: leave it, the note's
 *  caret going that way, or remove the maths (Backspace in an empty field). */
export type FieldEffect = "leaveLeft" | "leaveRight" | "leaveUp" | "leaveDown" | "removeMaths";

/** What a command did. Apply `changes`, then `rewrite`, each in reverse order. */
export interface FieldStep {
  /** Sorted, at most one, in the source before the command. */
  changes: FieldChange[];
  /** The change is an undo step of its own. */
  isolate: boolean;
  /** A shortcut's expansion, an undo step of its own, in the source after
   *  `changes`; the field's source is after both. */
  rewrite?: FieldChange[];
  effect?: FieldEffect;
}

export type FieldSlotKind =
  | "row" | "group" | "sup" | "sub" | "numer" | "denom" | "radicand" | "index"
  | "body" | "above" | "below" | "leftRight" | "cell" | "text" | "math" | "tag";

/** An ordered run of sibling atoms the caret moves between. */
export interface FieldSlot {
  kind: FieldSlotKind;
  /** A row's number (top-level rows split at `\\`), or a cell's row. */
  row?: number;
  /** A cell's column. */
  col?: number;
  /** `bare`: a one-token argument (`x^2`) that a second atom braces;
   *  `open`: a row, cell or `\over` side that grows with what is typed. */
  bounds: "delimited" | "bare" | "open";
  /** A text run: a stop between every character. */
  text: boolean;
  /** The content's UTF-16 range, inside its braces; empty for an empty slot. */
  from: number;
  to: number;
  /** The slot holding the atom this slot belongs to, by slot id. */
  parent: number | null;
}
"#;

#[wasm_bindgen]
pub struct MathField {
    field: Field,
    /// The step that made this field, as `FieldStep` JSON.
    step: Option<String>,
}

const fn wrap(field: Field) -> MathField {
    MathField { field, step: None }
}

fn js_error(message: &str) -> JsValue {
    js_sys::Error::new(message).into()
}

/// The boundary's JSON as a JS value; it is always valid JSON.
fn parsed(json: &str) -> JsValue {
    JSON::parse(json).unwrap_or(JsValue::UNDEFINED)
}

#[wasm_bindgen]
impl MathField {
    /// `source` with the caret at its end. Throws an `Error` named
    /// `ParseError` (KaTeX's message) when it does not parse: the view
    /// edits it as TeX.
    pub fn open(source: &str, display: bool) -> Result<Self, JsValue> {
        console_error_panic_hook::set_once();
        Field::new(source, display)
            .map(wrap)
            .map_err(|e| parse_error(&e))
    }

    #[wasm_bindgen(getter)]
    pub fn source(&self) -> String {
        self.field.source().to_owned()
    }

    #[wasm_bindgen(getter)]
    pub fn display(&self) -> bool {
        self.field.display()
    }

    #[wasm_bindgen(getter, unchecked_return_type = "FieldMode")]
    pub fn mode(&self) -> String {
        boundary::mode(self.field.mode()).to_owned()
    }

    /// The `\command` being typed, without its backslash.
    #[wasm_bindgen(getter)]
    pub fn pending(&self) -> Option<String> {
        self.field.pending().map(str::to_owned)
    }

    /// The selection's anchor, a stop id.
    #[wasm_bindgen(getter)]
    pub fn anchor(&self) -> u32 {
        self.field.selection().anchor.0 as u32
    }

    /// The selection's head (the caret), a stop id.
    #[wasm_bindgen(getter)]
    pub fn head(&self) -> u32 {
        self.field.selection().head.0 as u32
    }

    /// The selection's UTF-16 range, `[from, to]`: what copy takes.
    #[wasm_bindgen(getter)]
    pub fn selected(&self) -> Vec<u32> {
        boundary::selected(&self.field).to_vec()
    }

    /// Whether Space is the view's (its list of picks).
    #[wasm_bindgen(getter, js_name = spaceFree)]
    pub fn space_free(&self) -> bool {
        self.field.space_free()
    }

    /// Each stop's UTF-16 offset, by stop id (←/→ order; never
    /// decreasing, and stops of different slots can share one).
    pub fn stops(&self) -> Vec<u32> {
        boundary::stops(&self.field)
    }

    /// Each stop's slot id, by stop id.
    #[wasm_bindgen(js_name = stopSlots)]
    pub fn stop_slots(&self) -> Vec<u32> {
        boundary::stop_slots(&self.field)
    }

    /// Every slot, by slot id, each before the slots inside it.
    #[wasm_bindgen(unchecked_return_type = "FieldSlot[]")]
    pub fn slots(&self) -> JsValue {
        parsed(&boundary::slots(&self.field))
    }

    /// The field with the caret at UTF-16 `offset` (after an undo or an
    /// outside edit): of the stops there, the first (`after` false) or the
    /// last. Throws when `offset` is no character boundary of the source.
    #[wasm_bindgen(js_name = caretAt)]
    pub fn caret_at(&self, offset: u32, after: bool) -> Result<Self, JsValue> {
        boundary::caret_at(self.field.clone(), offset, after)
            .map(wrap)
            .map_err(|e| js_error(&e))
    }

    /// The field with this selection (stop ids), widened so both ends
    /// share a slot; an id past the last stop is the last.
    pub fn select(&self, anchor: u32, head: u32) -> Self {
        let selection = Selection {
            anchor: StopId(anchor as usize),
            head: StopId(head as usize),
        };
        wrap(self.field.clone().select(selection))
    }

    /// The field with a pending `\command` (`""` right after `\`), or none.
    #[wasm_bindgen(js_name = withPending)]
    pub fn with_pending(&self, name: Option<String>) -> Self {
        wrap(self.field.clone().with_pending(name))
    }

    /// The field after `command`, carrying its `step`. Throws a `TypeError`
    /// for a malformed command.
    pub fn run(
        &self,
        #[wasm_bindgen(unchecked_param_type = "FieldCommand")] command: &JsValue,
    ) -> Result<Self, JsValue> {
        let json: String = JSON::stringify(command)
            .map_err(|_| JsValue::from(TypeError::new("bad field command")))?
            .into();
        let command = boundary::command(&json).map_err(|e| JsValue::from(TypeError::new(&e)))?;
        let outcome = self.field.run(&command);
        let step = boundary::step(&self.field, &outcome);
        Ok(Self {
            field: outcome.field,
            step: Some(step),
        })
    }

    /// The command that made this field; `undefined` for one `open`,
    /// `caretAt`, `select` or `withPending` made.
    #[wasm_bindgen(getter, unchecked_return_type = "FieldStep | undefined")]
    pub fn step(&self) -> JsValue {
        self.step.as_deref().map_or(JsValue::UNDEFINED, parsed)
    }
}

/// The field's shortcut table, `[keys, LaTeX]` pairs: typing the keys in
/// maths gives the LaTeX (`#0` takes the selection, `#?` is an empty slot).
#[wasm_bindgen(unchecked_return_type = "[string, string][]")]
pub fn shortcuts() -> JsValue {
    parsed(&boundary::shortcuts())
}
