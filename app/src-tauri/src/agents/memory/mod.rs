//! The memory store the agents write back into, and the one writer for it.
//!
//! [`crate::agents`] scaffolds the layer and never writes a memory; this module
//! does, on an agent's behalf, through `oculus memory`.
//!
//! A memory is two writes — the file and its line in `MEMORY.md` — and agents
//! skip the second. So the index is **derived from the files** and rewritten on
//! every write, and routing is a flag, not a path to reason out.
//!
//! Nothing here touches the database: a sandboxed in-app thread cannot write
//! `oculus.db`, but may write files under `agents/`.

mod bucket;
mod frontmatter;
mod maintain;
mod query;
mod slug;
#[cfg(test)]
mod tests;
mod write;

pub use bucket::{bucket_dir, buckets, resolve_subject};
pub use frontmatter::{parse, render, Entry, Front};
pub use maintain::{reindex, relocate, remove};
pub use query::{find, list};
pub use slug::{humanize, slug};
pub use write::{write, WriteSpec, Written};

/// The four kinds a memory can be, the templates' vocabulary. `feedback` and
/// `project` also owe a **Why** and a **How to apply** — see [`WriteSpec`].
pub const TYPES: [&str; 4] = ["user", "feedback", "project", "reference"];

/// The generated half of a `MEMORY.md` starts here; prose above it is kept.
const INDEX_MARK: &str =
    "<!-- Written by `oculus memory` from the files beside this one — edit a memory, not this list. -->";

const INDEX_NAME: &str = "MEMORY.md";
