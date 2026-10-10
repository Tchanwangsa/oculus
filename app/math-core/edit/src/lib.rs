// The fork's `bon` macros use syn 3, its `phf` and `strum` macros syn 2:
// build-time only.
#![allow(clippy::multiple_crate_versions)]
//! The visual maths field's edit model, over the formula's LaTeX source.
//!
//! The field edits the source text itself; this crate says where a caret
//! can be. [`stops`] parses a formula with source mapping on and lays out
//! its **slots** (ordered runs of sibling atoms: a group's content, a
//! script, a numerator, a cell, a `\text{}` run) and its **stops** (the
//! caret positions between a slot's atoms, plus every character of a text
//! slot). Stops come in ←/→ order, which is source order, and each is a
//! byte offset of the source; [`utf16`] converts for the DOM and
//! CodeMirror. Pure Rust, with no DOM.
//!
//! The rules for the tricky shapes (bare arguments, shared offsets,
//! macros, arrays, text, empty slots) are on the types and in `build/`.

mod build;
pub mod check;
mod parse;
mod slot;
mod stops;
pub mod utf16;

pub use parse::parse;
pub use slot::{Bounds, Slot, SlotId, SlotKind, SlotPath, Stop, StopId};
pub use stops::{Affinity, Stops, stops};
