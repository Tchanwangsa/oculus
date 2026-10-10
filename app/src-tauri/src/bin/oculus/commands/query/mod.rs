//! The read-only library commands — search, grep, read, files, calendar —
//! and the file lookup and subject filtering they share.

use crate::*;

mod calendar;
mod files;
mod grep;
mod library;
mod loaders;
mod read;
mod search;
mod subjects;
#[cfg(test)]
mod tests;

pub(crate) use library::*;
pub(crate) use subjects::*;

/// One row of `files` with its subject code and how much of it is searchable.
#[derive(Serialize)]
pub(crate) struct LibFile {
    #[serde(skip)]
    id: i64,
    #[serde(rename = "subject")]
    code: String,
    #[serde(rename = "path")]
    relative_path: String,
    filename: String,
    file_type: String,
    category: Option<String>,
    size_bytes: Option<i64>,
    parse_status: Option<String>,
    indexed_pages: i64,
}
