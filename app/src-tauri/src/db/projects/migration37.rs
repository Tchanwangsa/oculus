/// Migration 37 (nullable `project_id`), held here so the tests run the string
/// the app runs. `parent_id` references `project_tasks_new` *itself*: pointed at
/// the old table, `DROP TABLE` fires its `ON DELETE CASCADE` and empties the copy
/// (`defer_foreign_keys` defers the check, not the action). The rename fixes the
/// name up. `ORDER BY id` and the `PRAGMA` are belt and braces.
pub const UNFILED_TASKS_SQL: &str = r#"
PRAGMA defer_foreign_keys = ON;

CREATE TABLE project_tasks_new (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id  INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    -- Itself, not `project_tasks`, until the rename below — see the doc comment.
    parent_id   INTEGER REFERENCES project_tasks_new(id) ON DELETE CASCADE,
    title       TEXT    NOT NULL,
    body        TEXT,
    column_id   TEXT    NOT NULL,
    position    REAL    NOT NULL,
    starts_at   TEXT,
    due_at      TEXT,
    estimate_minutes INTEGER,
    done_at     TEXT,
    source      TEXT    NOT NULL DEFAULT 'manual',
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO project_tasks_new
       (id, project_id, parent_id, title, body, column_id, position, starts_at,
        due_at, estimate_minutes, done_at, source, created_at, updated_at)
SELECT id, project_id, parent_id, title, body, column_id, position, starts_at,
       due_at, estimate_minutes, done_at, source, created_at, updated_at
  FROM project_tasks
 ORDER BY id;

DROP TABLE project_tasks;
ALTER TABLE project_tasks_new RENAME TO project_tasks;

CREATE INDEX IF NOT EXISTS idx_project_tasks_project ON project_tasks(project_id, column_id, position);
CREATE INDEX IF NOT EXISTS idx_project_tasks_due     ON project_tasks(due_at);
"#;
