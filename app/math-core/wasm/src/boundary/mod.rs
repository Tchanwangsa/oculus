//! What `MathField` hands across the wasm boundary, in plain Rust.
//!
//! The edit model counts bytes of the source; JS counts UTF-16 code units
//! (a string's indices, `data-s`/`data-e`, CodeMirror's positions). Every
//! offset is converted here and nowhere else: an offset coming in by
//! `oculus_math_edit::utf16::byte_offset`, offsets going out by a
//! [`Units`] table of the source they index. Stop and slot ids are indices
//! and cross as they are. Commands come in as JSON ([`command`]); a step
//! ([`step`]) and the slots ([`slots`]) go out as JSON, with the shapes
//! `field.rs` declares to TypeScript.

mod command;
mod slots;
mod step;
#[cfg(test)]
mod tests;

use oculus_math_edit::{
    Affinity, Field, Mode, SHORTCUTS,
    utf16::{Units, byte_offset},
};

pub use command::command;
pub use slots::slots;
pub use step::step;

/// `field` with the caret at UTF-16 offset `unit`.
///
/// Of the stops there, the first (`after` false, the one that goes with
/// the text before it) or the last. An error when `unit` is past the end
/// or between a surrogate pair.
pub fn caret_at(field: Field, unit: u32, after: bool) -> Result<Field, String> {
    let byte = byte_offset(field.source(), unit as usize)
        .ok_or_else(|| format!("{unit} is not a character boundary of the source"))?;
    let affinity = if after {
        Affinity::After
    } else {
        Affinity::Before
    };
    Ok(field.caret_at(byte, affinity))
}

/// Each stop's UTF-16 offset, by stop id.
#[must_use]
pub fn stops(field: &Field) -> Vec<u32> {
    let units = Units::new(field.source());
    field
        .stops()
        .stops()
        .iter()
        .map(|stop| units.of(stop.offset))
        .collect()
}

/// Each stop's slot id, by stop id.
#[must_use]
pub fn stop_slots(field: &Field) -> Vec<u32> {
    field
        .stops()
        .stops()
        .iter()
        .map(|stop| stop.slot.0 as u32)
        .collect()
}

/// The selection's range in UTF-16, `[from, to]`.
#[must_use]
pub fn selected(field: &Field) -> [u32; 2] {
    let range = field.selected();
    let units = Units::new(field.source());
    [units.of(range.start), units.of(range.end)]
}

#[must_use]
pub const fn mode(mode: Mode) -> &'static str {
    match mode {
        Mode::Math => "math",
        Mode::Text => "text",
        Mode::Command => "command",
    }
}

/// The shortcut table as JSON, `[[keys, LaTeX], …]`; in the LaTeX, `#0`
/// takes the selection and `#?` is an empty slot.
#[must_use]
pub fn shortcuts() -> String {
    serde_json::to_string(SHORTCUTS).unwrap_or_default()
}
