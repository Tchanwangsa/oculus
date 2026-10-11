//! The database access every module shares. In the app the frontend writes most
//! rows through tauri-plugin-sql; headless runs write the same rows with the
//! same SQL here. Schema ownership stays with the migrations: a missing
//! database is reported, never created.

mod calendar;
mod content_end;
mod files;
mod lectures;
mod pool;
mod runs;
mod subjects;
#[cfg(test)]
mod tests;

pub use calendar::*;
pub use content_end::*;
pub use files::*;
pub use lectures::*;
pub use pool::*;
pub use runs::*;
pub use subjects::*;
