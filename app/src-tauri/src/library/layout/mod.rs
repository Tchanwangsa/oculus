//! Every path shape in the library, in one place.
//!
//! The library is `courses/<code>/…` plus each lecture's folder. Inside a
//! subject, Oculus's sync writes the synced kinds (`SyncedKind`, `home.md`,
//! `syllabus.md`), the student owns their own files (`OwnKind`), and app state
//! sits in hidden folders. `zone_of` is the one question a guard asks of a
//! data-dir-relative path; `category_of` labels a row; `derived` names a
//! PDF's parse artifacts. Folder names are data in `shape::CURRENT`, so a new
//! layout changes that table, not its callers. docs/architecture.md has the
//! tree.

mod category;
mod derived;
mod hidden;
mod own;
mod recordings;
mod roots;
mod shape;
mod synced;
mod zone;

#[cfg(test)]
mod tests;

pub use category::*;
pub use derived::*;
pub use hidden::*;
pub use own::*;
pub use recordings::*;
pub use roots::*;
pub use synced::*;
pub use zone::*;
