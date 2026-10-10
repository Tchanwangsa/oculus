//! The document model: positions are UTF-16 code units at this API, as in
//! CodeMirror.

pub mod change;
pub mod history;
mod rope;
pub mod selection;
#[cfg(test)]
mod tests;
pub mod transaction;

pub use change::{Change, ChangeDesc, ChangeError, ChangeSet, ChangeSpec, MapMode, Touch};
pub use history::{History, HistoryConfig, HistoryEvent, Popped};
pub use rope::{Chunks, Iter, Line, Lines, PosError, Text};
pub use selection::{Selection, SelectionError, SelectionRange};
pub use transaction::{Isolate, RangeChange, State, StateError, Transaction};
