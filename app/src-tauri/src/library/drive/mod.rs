//! Drive: one folder of the library at a time, as the folder browser shows it.
//!
//! `list_folder` reads the disk, so empty folders show and a file with no
//! `files` row still lists; the frontend joins rows (parse and embed state,
//! recency) onto the entries by `relative_path`. Only the lecture recordings
//! come from the database, because a recording opens the lecture page, not a
//! folder. What the listing leaves out: dot-names, symlinks, hidden app state
//! (`layout::zone_of`), and a file's parse artifacts and converted PDF
//! (`paths::parse_artifacts`), which its row already toggles to.
//! docs/frontend.md has the browser this feeds.

pub(crate) mod commands;
mod entries;
mod listing;
mod recordings;

#[cfg(test)]
mod tests;
