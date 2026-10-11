//! Course files for the frontend: reads, and the student's own uploads and
//! notes.

pub(crate) mod commands;
pub(crate) mod documents;
#[cfg(test)]
mod tests;
pub(crate) mod uploads;

pub use uploads::ImportedFile;
