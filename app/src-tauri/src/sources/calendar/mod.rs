//! Canvas calendar: timetabled classes (`type=event`) and dated coursework
//! (`type=assignment`), flattened into one row shape. See `docs/calendar.md`.
//!
//! - Canvas expands a repeating class server-side into one event per
//!   occurrence; `all_events=true` fetches the whole semester.
//! - A sectioned class is a parent spanning every section plus per-section
//!   `child_events`: children replace the parent, filtered to this user's
//!   sections when known (`fetch::my_section_codes`).

pub(crate) mod command;
mod fetch;
mod parse;

#[cfg(test)]
mod tests;

pub use fetch::fetch;

/// One dated item. `start_at`/`end_at` are Canvas's UTC strings, verbatim.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CalendarEvent {
    /// `event_123` / `assignment_456` — stable across syncs, the upsert key.
    pub id: String,
    /// `class` for a scheduled event, `due` for a deadline.
    pub kind: String,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
    pub location: Option<String>,
    pub url: Option<String>,
    /// Markdown, converted from Canvas HTML.
    pub description: Option<String>,
}

pub const KIND_CLASS: &str = "class";
pub const KIND_DUE: &str = "due";
