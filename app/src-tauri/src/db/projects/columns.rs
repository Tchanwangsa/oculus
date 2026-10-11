use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use super::rows::Project;

/// One board column: `kind` is what the app reasons about, `name` is the
/// user's and changes. Stored as JSON on the project, not as a table.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Column {
    pub id: String,
    pub name: String,
    /// `backlog` | `active` | `done`.
    pub kind: String,
}

/// The frontend's `DEFAULT_COLUMNS`; also the board of an unfiled task.
pub fn default_columns() -> Vec<Column> {
    [
        ("backlog", "Backlog", "backlog"),
        ("todo", "Todo", "active"),
        ("doing", "In progress", "active"),
        ("done", "Done", "done"),
    ]
    .iter()
    .map(|(id, name, kind)| Column {
        id: (*id).to_string(),
        name: (*name).to_string(),
        kind: (*kind).to_string(),
    })
    .collect()
}

/// The project's own board, or the default one for an unfiled task
/// (`boardOf` in `app/src/lib/planning/projects/columns.ts`).
pub(super) fn board_of(project: Option<&Project>) -> &[Column] {
    match project {
        Some(p) => &p.columns,
        None => {
            static DEFAULT_BOARD: OnceLock<Vec<Column>> = OnceLock::new();
            DEFAULT_BOARD.get_or_init(default_columns)
        }
    }
}

/// Resolve a column id against a board, or refuse naming the ids it does have.
pub(super) fn require_column<'a>(
    project: Option<&'a Project>,
    column_id: &str,
) -> Result<&'a Column, String> {
    let board = board_of(project);
    board.iter().find(|c| c.id == column_id).ok_or_else(|| {
        let known: Vec<&str> = board.iter().map(|c| c.id.as_str()).collect();
        let whose = match project {
            Some(p) => format!("project {}", p.id),
            None => "an unfiled task".to_string(),
        };
        format!(
            "{whose} has no column \"{column_id}\" (has: {})",
            known.join(", ")
        )
    })
}
