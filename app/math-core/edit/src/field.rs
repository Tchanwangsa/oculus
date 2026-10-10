//! The field's state between commands, and what a command returns.
//!
//! A [`Field`] is one formula's source, its stops, a selection and the
//! pending `\command`; [`Field::run`] applies a [`crate::Command`] and
//! returns an [`Outcome`]: the text changes against the old source, the
//! field after them, and what the view has to do. Nothing here owns undo:
//! the note's history does, and after an undo the view builds a new field
//! and places the caret with [`Field::caret_at`].

use core::ops::Range;

use katex::types::ParseError;

use crate::{
    command::{takes_space, widen},
    parse::renders,
    slot::StopId,
    stops::{Affinity, Stops, stops},
};

/// The selection: two caret stops.
///
/// Each is an index into the source's [`Stops::stops`]. An index names one stop even where several share an
/// offset (`\frac ab`'s 7), so it survives between commands on unchanged
/// source; after an outside change [`Field::caret_at`] places it from a
/// source offset. Plain integers, so it crosses the wasm boundary as is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: StopId,
    pub head: StopId,
}

impl Selection {
    /// A collapsed selection: a caret.
    #[must_use]
    pub const fn caret(stop: StopId) -> Self {
        Self {
            anchor: stop,
            head: stop,
        }
    }

    #[must_use]
    pub fn is_caret(&self) -> bool {
        self.anchor == self.head
    }
}

/// What typing does at the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    Math,
    /// In a `\text{}`-like run: characters are typed literally.
    Text,
    /// A `\command` is being typed (the view draws it as a chip).
    Command,
}

/// One text change, in bytes of the source before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// What the view does after a command, beyond showing the new field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    /// Leave the field, the note's caret going this way: an arrow past
    /// the edge, Esc, Enter in inline maths.
    Leave(Direction),
    /// Remove the maths from the note: Backspace in an empty field.
    RemoveMaths,
}

/// A formula being edited.
#[derive(Clone, Debug)]
pub struct Field {
    source: String,
    display: bool,
    renders: bool,
    stops: Stops,
    selection: Selection,
    pending: Option<String>,
}

/// What a command did.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// Sorted, non-overlapping, in bytes of the old source (at most one).
    pub changes: Vec<Change>,
    /// The field after the command: the new source, selection and pending
    /// command.
    pub field: Field,
    /// The change is an undo step of its own.
    pub isolate: bool,
    pub effect: Option<Effect>,
}

impl Outcome {
    /// The field unchanged.
    #[must_use]
    pub fn none(field: &Field) -> Self {
        Self {
            changes: Vec::new(),
            field: field.clone(),
            isolate: false,
            effect: None,
        }
    }

    /// Only the view acts: the field stays as it is.
    #[must_use]
    pub fn effect(field: &Field, effect: Effect) -> Self {
        Self {
            effect: Some(effect),
            ..Self::none(field)
        }
    }

    /// The source is unchanged; the selection or pending command moves.
    #[must_use]
    pub const fn moved(field: Field) -> Self {
        Self {
            changes: Vec::new(),
            field,
            isolate: false,
            effect: None,
        }
    }

    #[must_use]
    pub const fn selection(&self) -> Selection {
        self.field.selection
    }

    #[must_use]
    pub fn pending(&self) -> Option<&str> {
        self.field.pending.as_deref()
    }
}

impl Field {
    /// The formula with the caret at its end. Unparseable source is an
    /// error: the view edits it as TeX.
    pub fn new(source: &str, display: bool) -> Result<Self, ParseError> {
        let stops = stops(source, display)?;
        let last = StopId(stops.stops().len() - 1);
        Ok(Self {
            source: source.to_owned(),
            display,
            renders: renders(source, display).is_ok(),
            stops,
            selection: Selection::caret(last),
            pending: None,
        })
    }

    /// The same field with the caret at `offset` (after an undo, or a
    /// click the view resolved to an offset).
    #[must_use]
    pub fn caret_at(self, offset: usize, affinity: Affinity) -> Self {
        let stop = self.stops.stop_at(offset, affinity);
        self.select(Selection::caret(stop))
    }

    /// The same field with `selection`, widened so both ends share a slot
    /// (see [`crate::widen`]). Out-of-range stops go to the last.
    #[must_use]
    pub fn select(mut self, selection: Selection) -> Self {
        let last = StopId(self.stops.stops().len() - 1);
        let clamp = |id: StopId| if id > last { last } else { id };
        let (anchor, head) = widen(
            &self.stops,
            clamp(selection.anchor),
            clamp(selection.head),
            None,
        );
        self.selection = Selection { anchor, head };
        self
    }

    /// The same field with a pending `\command` (`Some("")` right after
    /// `\`), or none.
    #[must_use]
    pub fn with_pending(mut self, pending: Option<String>) -> Self {
        self.pending = pending;
        self
    }

    /// A field for new source, which must parse; the selection is set by
    /// the caller. Keeps whether the source renders, for the next command.
    pub(crate) fn reparse(&self, source: String) -> Result<Self, ParseError> {
        let stops = stops(&source, self.display)?;
        let renders = renders(&source, self.display).is_ok();
        Ok(Self {
            source,
            display: self.display,
            renders,
            stops,
            selection: Selection::caret(StopId(0)),
            pending: None,
        })
    }

    pub(crate) const fn set_selection(&mut self, selection: Selection) {
        self.selection = selection;
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn display(&self) -> bool {
        self.display
    }

    /// Whether the source renders (some parses and fails to build).
    #[must_use]
    pub const fn renders(&self) -> bool {
        self.renders
    }

    #[must_use]
    pub const fn stops(&self) -> &Stops {
        &self.stops
    }

    #[must_use]
    pub const fn selection(&self) -> Selection {
        self.selection
    }

    #[must_use]
    pub fn pending(&self) -> Option<&str> {
        self.pending.as_deref()
    }

    /// The selection's byte range of the source (empty for a caret): what
    /// copy takes.
    #[must_use]
    pub fn selected(&self) -> Range<usize> {
        let a = self.stops.offset(self.selection.anchor);
        let h = self.stops.offset(self.selection.head);
        a.min(h)..a.max(h)
    }

    /// What typing does at the head.
    #[must_use]
    pub fn mode(&self) -> Mode {
        if self.pending.is_some() {
            Mode::Command
        } else if self.in_text() {
            Mode::Text
        } else {
            Mode::Math
        }
    }

    /// Whether Space is the view's (the quick picks): in maths, with no
    /// `\command` pending, where Space would not end a matrix cell. In a
    /// text run it is a space and in a `\command` it commits. A caret
    /// beside a `\text{}` atom is in maths here, since the run's own end
    /// is a stop of its own.
    #[must_use]
    pub fn space_free(&self) -> bool {
        self.mode() == Mode::Math && !takes_space(self)
    }

    pub(crate) fn in_text(&self) -> bool {
        let slot = self.stops.stop(self.selection.head).slot;
        self.stops.slot(slot).text
    }
}
