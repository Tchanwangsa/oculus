//! The decision for one file, from the SHA-256 of its bytes here (`local`),
//! at the last sync (`base`, the record) and on Ed (`server`); `None` is
//! absent. One side changed since the base: that side wins, deletes
//! included. Both changed: the same bytes agree, a delete loses to an edit,
//! and two different edits are a conflict.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// In step already.
    Nothing,
    /// Both sides agree but the record does not: record what they hold.
    Record,
    /// Write Ed's bytes here.
    Take,
    /// Upload the local bytes.
    Push,
    /// Deleted here: `fsop remove` it on Ed.
    Remove,
    /// Deleted on Ed: delete it here.
    DeleteLocal,
    /// Both edited, differently: Ed's copy goes beside the local file.
    Conflict,
}

pub fn decide(local: Option<&str>, base: Option<&str>, server: Option<&str>) -> Action {
    use Action::*;
    match (local, server) {
        (l, s) if l == s => {
            if l == base {
                Nothing
            } else {
                Record
            }
        }
        (Some(l), Some(s)) => {
            if base == Some(l) {
                Take
            } else if base == Some(s) {
                Push
            } else {
                Conflict
            }
        }
        (Some(l), None) => match base {
            Some(b) if b == l => DeleteLocal,
            _ => Push,
        },
        (None, Some(s)) => match base {
            Some(b) if b == s => Remove,
            _ => Take,
        },
        // Equal sides are matched first.
        (None, None) => Nothing,
    }
}
