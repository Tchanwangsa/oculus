//! The database's side of the move: bringing the schema up first, reading
//! what the plan needs, and rewriting every stored path in one transaction.

mod read;
mod rewrite;
mod schema;

pub use read::{is_video, rows, Rows};
#[cfg(test)]
pub(super) use rewrite::lecture_path;
pub(super) use rewrite::{column, json_column, remap_json, Tx};
pub use rewrite::{rewrite, DbCounts};
pub use schema::ensure;
#[cfg(test)]
pub use schema::ensure_up_to;
