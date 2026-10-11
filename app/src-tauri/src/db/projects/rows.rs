use serde::Serialize;
use sqlx::Row;

use super::columns::{default_columns, Column};

/// A project, with the subject's code resolved through a join (never stored).
#[derive(Serialize, Clone, Debug)]
pub struct Project {
    pub id: i64,
    pub subject_id: Option<i64>,
    pub subject_code: Option<String>,
    pub name: String,
    pub brief: Option<String>,
    pub status: String,
    pub starts_at: Option<String>,
    pub due_at: Option<String>,
    pub columns: Vec<Column>,
    /// Parsed from JSON; `[]` when untagged, never null.
    pub tags: Vec<String>,
    /// A `CalEvent.id` (`app/src/lib/planning/calendar/`). Read-only here: pinning is
    /// the app's.
    pub event_id: Option<String>,
    pub position: f64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Task {
    pub id: i64,
    /// NULL on an unfiled task.
    pub project_id: Option<i64>,
    pub parent_id: Option<i64>,
    pub title: String,
    pub body: Option<String>,
    pub column_id: String,
    pub position: f64,
    pub starts_at: Option<String>,
    pub due_at: Option<String>,
    pub estimate_minutes: Option<i64>,
    pub done_at: Option<String>,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

pub(super) const PROJECT_SELECT: &str = r#"SELECT p.id, p.subject_id, s.code AS subject_code, p.name, p.brief,
       p.status, p.starts_at, p.due_at, p.columns, p.tags, p.event_id, p.position,
       p.source, p.created_at, p.updated_at
  FROM projects p
  LEFT JOIN subjects s ON s.id = p.subject_id"#;

/// Trimmed, deduplicated case-insensitively (first spelling wins) and capped —
/// must match `normaliseTags` in `app/src/lib/planning/projects/rows.ts`.
pub fn normalise_tags(tags: impl IntoIterator<Item = String>) -> Vec<String> {
    const MAX_TAGS: usize = 24;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for raw in tags {
        let tag = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if tag.is_empty() {
            continue;
        }
        if !seen.insert(tag.to_lowercase()) {
            continue;
        }
        out.push(tag);
        if out.len() >= MAX_TAGS {
            break;
        }
    }
    out
}

pub(super) fn to_project(r: &sqlx::sqlite::SqliteRow) -> Project {
    // Fall back rather than fail the whole read on one bad row, as `toProject` does.
    let columns = serde_json::from_str::<Vec<Column>>(&r.get::<String, _>("columns"))
        .ok()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(default_columns);
    let tags = r
        .get::<Option<String>, _>("tags")
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default();
    Project {
        id: r.get("id"),
        subject_id: r.get("subject_id"),
        subject_code: r.get("subject_code"),
        name: r.get("name"),
        brief: r.get("brief"),
        status: r.get("status"),
        starts_at: r.get("starts_at"),
        due_at: r.get("due_at"),
        columns,
        tags,
        event_id: r.get("event_id"),
        position: r.get("position"),
        source: r.get("source"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

pub(super) fn to_task(r: &sqlx::sqlite::SqliteRow) -> Task {
    Task {
        id: r.get("id"),
        project_id: r.get("project_id"),
        parent_id: r.get("parent_id"),
        title: r.get("title"),
        body: r.get("body"),
        column_id: r.get("column_id"),
        position: r.get("position"),
        starts_at: r.get("starts_at"),
        due_at: r.get("due_at"),
        estimate_minutes: r.get("estimate_minutes"),
        done_at: r.get("done_at"),
        source: r.get("source"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}
