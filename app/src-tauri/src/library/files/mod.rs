//! Course files for the frontend: reads, the student's own uploads and notes,
//! and the Canvas cookie for server-side requests.

pub(crate) mod commands;
mod cookies;
pub(crate) mod documents;
#[cfg(test)]
mod tests;
pub(crate) mod uploads;

pub use cookies::*;
pub use uploads::ImportedFile;
