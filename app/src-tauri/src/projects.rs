//! Projects: boards, tasks and one level of subtask, written headlessly.
//!
//! The window side of these tables belongs to the frontend
//! (`app/src/lib/projects.ts`); this is the same SQL without a window, so the
//! `oculus` CLI — and through it the chat agent — can plan work and the board
//! picks it up. Exactly the pair `store.rs` is for the scrape tables: change a
//! table's shape and both writers change.
//!
//! Three rules are why this is more than a few INSERTs, and all three are
//! mirrored from that module rather than reinvented:
//!
//! - **A column id is checked against a board** before anything is filed under
//!   it — the project's own `columns`, or the default board for a task that
//!   belongs to no project at all ({@link board_of}). A task in a column no
//!   view renders is not misfiled, it is invisible — and here `--column` is
//!   free text an agent typed, which is the case the frontend's
//!   `requireColumn` was written for.
//! - **`done_at` is derived from the destination column's `kind`**, never
//!   passed in, on create as well as on move. So "which column is it in" and
//!   "is it finished" cannot disagree whichever door the task came through.
//! - **Subtasks are one level deep**, enforced in code because SQLite cannot
//!   express "the parent has no parent" as a constraint. The board draws a
//!   task and its children, not a tree; a grandchild would never be drawn.
//! - **A task may belong to no project at all** (migration 37). That is the
//!   absence of a project, not an "Inbox" project: `project_id` is NULL, the
//!   board it is checked against is the default one, and there is no
//!   `updated_at` to move.
//!
//! Schema is migration 27 in `lib.rs`, plus 33 (`tags`, `event_id`) and 37
//! (`project_id` nullable, whose SQL is {@link UNFILED_TASKS_SQL} below).
//! Nothing here creates it — same rule as `store.rs`: a fresh machine opens the
//! app once first.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};

// ── Migration 37 ─────────────────────────────────────────────────────────────

/// The table rebuild that lets a task belong to no project — migration 37's
/// whole body, held here so the tests can run the very string the app runs.
///
/// **The why is in the migration's comment block in `lib.rs`**; one line of it
/// is worth having in front of the SQL. `parent_id` deliberately references
/// `project_tasks_new`, *itself*, rather than the `project_tasks` it is about to
/// become: pointed at the old table, the `DROP TABLE` below would fire the
/// self-reference's `ON DELETE CASCADE` and empty the copy it had just made,
/// because `defer_foreign_keys` defers the *check*, not the action — measured,
/// and the case
/// {@link unfiled_tests::the_rebuild_keeps_parents_children_and_their_cascades}
/// fails on. The rename fixes the clause up to the final name.
///
/// `ORDER BY id` and the `PRAGMA` are belt and braces rather than one fix each:
/// SQLite checks this copy's foreign keys at the *end* of the one
/// `INSERT … SELECT` rather than per row, so either alone carries it and
/// dropping both still passes. They are here for the edit that splits the copy
/// in two, or runs it outside a transaction.
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

// ── Columns ──────────────────────────────────────────────────────────────────

/// One board column: `kind` is what the app reasons about, `name` is the
/// user's and changes. Stored as JSON on the project, not as a table.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Column {
    pub id: String,
    pub name: String,
    /// `backlog` | `active` | `done`.
    pub kind: String,
}

/// The board a new project opens with — the same four `DEFAULT_COLUMNS` the
/// frontend serialises, so a project created from the CLI is indistinguishable
/// from one created on the board.
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

/// The board a task's column is checked against: the project's own, or the
/// default one when the task belongs to no project.
///
/// `column_id` is NOT NULL on every task, unfiled ones included, so a task with
/// no project still has to name a column that something can draw. It names one
/// of {@link default_columns}' four ids — which are also the ids a *new*
/// project is born with, so filing an unfiled task into a default board later
/// needs no translation. `boardOf` in `app/src/lib/projects.ts` is the same
/// helper on the window side.
fn board_of(project: Option<&Project>) -> &[Column] {
    match project {
        Some(p) => &p.columns,
        // Built once rather than per write: `default_columns` allocates, and
        // every unfiled create, move and check reads this.
        None => {
            static DEFAULT_BOARD: OnceLock<Vec<Column>> = OnceLock::new();
            DEFAULT_BOARD.get_or_init(default_columns)
        }
    }
}

/// Resolve a column id against a board, or refuse with the ids it does have —
/// the agent's next attempt should not need a second command to find out what
/// the board is called.
fn require_column<'a>(
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
        format!("{whose} has no column \"{column_id}\" (has: {})", known.join(", "))
    })
}

// ── Rows ─────────────────────────────────────────────────────────────────────

/// A project, with the subject's code resolved through a join — the code is
/// never stored, so a renamed subject cannot leave a stale copy behind.
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
    /// The user's own labels, parsed out of JSON at the boundary the way
    /// `columns` is. Always a list — `[]` is untagged, never null.
    pub tags: Vec<String>,
    /// The calendar event this project answers to, as a `CalEvent.id`
    /// (`app/src/lib/calendar.ts`). Read-only here: the CLI has no way to list
    /// the grid, so pinning is the app's, and this is carried so
    /// `oculus project show` can say a project already has one.
    pub event_id: Option<String>,
    pub position: f64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Task {
    pub id: i64,
    /// NULL on an unfiled task — one that belongs to no project at all
    /// (migration 37), rather than to an "Inbox" project.
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

const PROJECT_SELECT: &str = r#"SELECT p.id, p.subject_id, s.code AS subject_code, p.name, p.brief,
       p.status, p.starts_at, p.due_at, p.columns, p.tags, p.event_id, p.position,
       p.source, p.created_at, p.updated_at
  FROM projects p
  LEFT JOIN subjects s ON s.id = p.subject_id"#;

/// Trimmed, emptied out, deduplicated case-insensitively and capped — the same
/// normalisation `normaliseTags` in `app/src/lib/projects.ts` applies, so a tag
/// typed at the CLI and a tag typed on the About page store identically. First
/// spelling wins.
pub fn normalise_tags(tags: impl IntoIterator<Item = String>) -> Vec<String> {
    /// A project is labelled, not catalogued.
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

fn to_project(r: &sqlx::sqlite::SqliteRow) -> Project {
    // A board that will not parse is a board nothing can be drawn on, and the
    // tasks still name their column by id — so fall back rather than fail the
    // whole read on one bad row, the way `toProject` does.
    let columns = serde_json::from_str::<Vec<Column>>(&r.get::<String, _>("columns"))
        .ok()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(default_columns);
    // Same fallback as the board: a tag list that will not parse is not worth
    // failing the whole read over.
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

fn to_task(r: &sqlx::sqlite::SqliteRow) -> Task {
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

// ── Reads ────────────────────────────────────────────────────────────────────

/// Which projects a listing wants. `Personal` is `subject_id IS NULL` — the
/// planning that belongs to no course.
pub enum SubjectFilter {
    Any,
    Personal,
    Subject(i64),
}

/// Every project, in the board's own `position` order. `status` is `active`,
/// `archived`, or `all`.
pub async fn projects(
    pool: &SqlitePool,
    subject: SubjectFilter,
    status: &str,
) -> Result<Vec<Project>, String> {
    let mut where_parts: Vec<String> = Vec::new();
    match subject {
        SubjectFilter::Any => {}
        SubjectFilter::Personal => where_parts.push("p.subject_id IS NULL".into()),
        SubjectFilter::Subject(id) => where_parts.push(format!("p.subject_id = {id}")),
    }
    // One placeholder either way: sqlx refuses a bind the statement does not
    // use, and "all" is a value here rather than a different query.
    where_parts.push("(?1 = 'all' OR p.status = ?1)".into());
    let sql = format!(
        "{PROJECT_SELECT}{}\n ORDER BY p.position ASC, p.id ASC",
        if where_parts.is_empty() {
            String::new()
        } else {
            format!("\n WHERE {}", where_parts.join(" AND "))
        }
    );
    let rows = sqlx::query(&sql)
        .bind(status)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(to_project).collect())
}

pub async fn project(pool: &SqlitePool, id: i64) -> Result<Option<Project>, String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    project_on(&mut *conn, id).await
}

async fn project_on(conn: &mut SqliteConnection, id: i64) -> Result<Option<Project>, String> {
    let row = sqlx::query(&format!("{PROJECT_SELECT}\n WHERE p.id = ?1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.as_ref().map(to_project))
}

/// Every task of one project, parents and subtasks together, in `position`
/// order.
///
/// Deliberately not ordered by `column_id`: that would sort the columns
/// alphabetically, which is not the board's order and never will be. The
/// board's order is the project's `columns` array; the caller groups.
pub async fn tasks(pool: &SqlitePool, project_id: i64) -> Result<Vec<Task>, String> {
    let rows = sqlx::query(
        "SELECT * FROM project_tasks WHERE project_id = ?1 ORDER BY position ASC, id ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(to_task).collect())
}

/// Which tasks a cross-project listing wants: everything, or only the ones
/// filed nowhere.
///
/// The same two scopes the app's universal page offers (`TaskScope` in
/// `app/src/hooks/useTaskList.ts`, backed by `getAllTasks` and
/// `getUnfiledTasks`), so `oculus task list` and that page answer the same two
/// questions rather than two nearly-identical ones.
pub enum TaskScope {
    All,
    Unfiled,
}

/// Every task in the library, across every project and including the ones
/// filed nowhere — what `oculus task list` prints without `-p`.
///
/// Ordered unfiled-first, then by project, then by `position`: the caller
/// prints one board per project, and `position` is the only order a board has.
/// Deliberately *not* the app's `UNIVERSAL_ORDER`, which sorts by due date
/// first — that is the order of a list you read, and this is a set of boards
/// you print. The project's name and columns are not joined in: a caller that
/// groups needs the whole project anyway, and `projects()` hands it over in
/// one more read rather than a row's worth of duplicated columns per task.
pub async fn all_tasks(pool: &SqlitePool, scope: TaskScope) -> Result<Vec<Task>, String> {
    let filter = match scope {
        TaskScope::All => "",
        TaskScope::Unfiled => " WHERE project_id IS NULL",
    };
    let rows = sqlx::query(&format!(
        "SELECT * FROM project_tasks{filter}
          ORDER BY project_id IS NOT NULL, project_id ASC, position ASC, id ASC"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(to_task).collect())
}

/// How many tasks a project has, and how many are finished — what a listing
/// shows instead of reading every row.
pub async fn task_counts(pool: &SqlitePool, project_id: i64) -> Result<(i64, i64), String> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS total, COUNT(done_at) AS done
           FROM project_tasks WHERE project_id = ?1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok((row.get("total"), row.get("done")))
}

// ── Project writes ───────────────────────────────────────────────────────────

pub struct NewProject {
    pub name: String,
    pub subject_id: Option<i64>,
    pub brief: Option<String>,
    pub starts_at: Option<String>,
    pub due_at: Option<String>,
    pub tags: Vec<String>,
    /// `manual` | `agent` — the board says which ones you did not write.
    pub source: String,
}

/// Create a project at the end of the list and return its id. The board is the
/// same default one the app creates.
pub async fn create_project(pool: &SqlitePool, input: &NewProject) -> Result<i64, String> {
    let next: f64 = sqlx::query_scalar("SELECT CAST(COALESCE(MAX(position), -1) + 1 AS REAL) FROM projects")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
    let columns = serde_json::to_string(&default_columns()).map_err(|e| e.to_string())?;
    let tags = serde_json::to_string(&normalise_tags(input.tags.iter().cloned()))
        .map_err(|e| e.to_string())?;
    sqlx::query(
        r#"INSERT INTO projects
             (subject_id, name, brief, status, starts_at, due_at, columns, tags, position, source)
           VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?7, ?8, ?9)"#,
    )
    .bind(input.subject_id)
    .bind(&input.name)
    .bind(&input.brief)
    .bind(&input.starts_at)
    .bind(&input.due_at)
    .bind(columns)
    .bind(tags)
    .bind(next)
    .bind(&input.source)
    .execute(pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

/// A patch: `None` leaves a field alone, `Some(None)` clears it, `Some(v)`
/// writes it — the Rust spelling of the frontend's `undefined` / `null`.
#[derive(Default)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub subject_id: Option<Option<i64>>,
    pub brief: Option<Option<String>>,
    pub status: Option<String>,
    pub starts_at: Option<Option<String>>,
    pub due_at: Option<Option<String>>,
    /// Replaces the whole set, like the frontend's — there is no add/remove
    /// patch. `Some(vec![])` clears it.
    pub tags: Option<Vec<String>>,
}

pub async fn update_project(
    pool: &SqlitePool,
    id: i64,
    patch: &ProjectPatch,
) -> Result<(), String> {
    // Placeholders are numbered as the sets are collected and bound in the
    // same order: sqlx refuses a statement whose highest parameter is lower
    // than the number of binds, so a fixed ?1..?n list with holes in it would
    // fail on every partial patch.
    let mut sets: Vec<String> = Vec::new();
    let mut put = |column: &str| sets.push(format!("{column} = ?{}", sets.len() + 1));
    if patch.name.is_some() {
        put("name");
    }
    if patch.subject_id.is_some() {
        put("subject_id");
    }
    if patch.brief.is_some() {
        put("brief");
    }
    if patch.status.is_some() {
        put("status");
    }
    if patch.starts_at.is_some() {
        put("starts_at");
    }
    if patch.due_at.is_some() {
        put("due_at");
    }
    if patch.tags.is_some() {
        put("tags");
    }
    if sets.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "UPDATE projects SET {}, updated_at = datetime('now') WHERE id = ?{}",
        sets.join(", "),
        sets.len() + 1
    );
    // Held outside the builder: `q.bind(&tags_json)` borrows, so the string has
    // to outlive every `q` reassignment below it.
    let tags_json: String;
    let mut q = sqlx::query(&sql);
    if let Some(v) = &patch.name {
        q = q.bind(v);
    }
    if let Some(v) = &patch.subject_id {
        q = q.bind(*v);
    }
    if let Some(v) = &patch.brief {
        q = q.bind(v);
    }
    if let Some(v) = &patch.status {
        q = q.bind(v);
    }
    if let Some(v) = &patch.starts_at {
        q = q.bind(v);
    }
    if let Some(v) = &patch.due_at {
        q = q.bind(v);
    }
    if let Some(v) = &patch.tags {
        tags_json = serde_json::to_string(&normalise_tags(v.iter().cloned()))
            .map_err(|e| e.to_string())?;
        q = q.bind(&tags_json);
    }
    let affected = q
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?
        .rows_affected();
    if affected == 0 {
        return Err(format!("project {id} does not exist"));
    }
    Ok(())
}

// ── Task writes ──────────────────────────────────────────────────────────────

/// Whose child a new task is: an existing task's id, or the `key` of an
/// earlier item in the same batch, which is how one call can create a parent
/// and its subtasks together.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ParentRef {
    Id(i64),
    Key(String),
}

/// One task to create. Deserialised straight from a `--batch` item, so the
/// field names here are the batch's contract.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct NewTask {
    pub title: String,
    /// Board column id. Omitted means the project's first column.
    #[serde(default, alias = "column_id")]
    pub column: Option<String>,
    #[serde(default, alias = "parent_id")]
    pub parent: Option<ParentRef>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default, alias = "due_at")]
    pub due: Option<String>,
    #[serde(default, alias = "starts_at")]
    pub starts: Option<String>,
    #[serde(default, alias = "estimate_minutes")]
    pub estimate: Option<i64>,
    /// A name this item can be referred to by `parent` later in the same
    /// batch. Never stored.
    #[serde(default)]
    pub key: Option<String>,
}

/// Create one or many tasks and return their ids **in input order**.
///
/// One transaction for the whole batch: a breakdown is a shape, not a pile of
/// rows, and half a breakdown on the board would be worse than none — so a
/// rejected item (an unknown column, a parent that is itself a subtask) rolls
/// the lot back and nothing is written. The single-task path is this function
/// with one item, so both doors behave identically.
pub async fn create_tasks(
    pool: &SqlitePool,
    project_id: Option<i64>,
    items: &[NewTask],
    source: &str,
) -> Result<Vec<i64>, String> {
    if items.is_empty() {
        return Err("no tasks given".to_string());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // `None` is a task filed nowhere, and its board is the default one — so
    // there is nothing to look up and nothing to refuse.
    let project = match project_id {
        Some(id) => Some(
            project_on(&mut *tx, id)
                .await?
                .ok_or_else(|| format!("project {id} does not exist"))?,
        ),
        None => None,
    };

    let mut by_key: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    let mut ids: Vec<i64> = Vec::with_capacity(items.len());

    for (n, item) in items.iter().enumerate() {
        // Which item failed only means something in a batch; on a single add
        // the prefix would be noise in front of the real message.
        let where_ = || {
            if items.len() > 1 {
                format!("task {} ({:?}): ", n + 1, item.title)
            } else {
                String::new()
            }
        };
        if item.title.trim().is_empty() {
            return Err(format!("{}a task needs a title", where_()));
        }
        let parent_id = match &item.parent {
            None => None,
            Some(ParentRef::Id(id)) => Some(*id),
            Some(ParentRef::Key(k)) => Some(
                *by_key
                    .get(k.as_str())
                    .ok_or_else(|| format!("{}no earlier task in this batch has key \"{k}\"", where_()))?,
            ),
        };
        let mut parent_column: Option<String> = None;
        if let Some(parent) = parent_id {
            parent_column = Some(
                assert_can_parent(&mut *tx, parent, project_id)
                    .await
                    .map_err(|e| format!("{}{e}", where_()))?,
            );
        }

        // A subtask with no column of its own inherits its **parent's**, not
        // the board's first column. The first column is Backlog on a default
        // board, and a subtask of an in-progress task filed there is a row the
        // Backlog view never draws — it lists top-level tasks — while the
        // board and the table look right, because both draw a subtask under
        // its parent wherever it claims to be. An explicit `--column` still
        // wins: a subtask can legitimately be done while its parent is not.
        let board = board_of(project.as_ref());
        let column = match &item.column {
            Some(id) => {
                require_column(project.as_ref(), id).map_err(|e| format!("{}{e}", where_()))?
            }
            None => parent_column
                .as_deref()
                // A column the board has since dropped falls back rather than
                // failing: the parent's row is already there either way.
                .and_then(|id| board.iter().find(|c| c.id == id))
                .or_else(|| board.first())
                .ok_or("project has no columns")?,
        };
        let done = column.kind == "done";

        // `IS`, not `=`: an unfiled task's neighbours are the other unfiled
        // tasks in that column, and `project_id = NULL` matches nothing.
        let next: f64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(position), -1) + 1 AS REAL) FROM project_tasks
              WHERE project_id IS ?1 AND column_id = ?2",
        )
        .bind(project_id)
        .bind(&column.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        let id = sqlx::query(
            r#"INSERT INTO project_tasks
                 (project_id, parent_id, title, body, column_id, position, starts_at,
                  due_at, estimate_minutes, done_at, source)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                       CASE WHEN ?10 THEN datetime('now') ELSE NULL END, ?11)"#,
        )
        .bind(project_id)
        .bind(parent_id)
        .bind(item.title.trim())
        .bind(&item.body)
        .bind(&column.id)
        .bind(next)
        .bind(&item.starts)
        .bind(&item.due)
        .bind(item.estimate)
        .bind(i64::from(done))
        .bind(source)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("{}{e}", where_()))?
        .last_insert_rowid();

        if let Some(key) = item.key.as_deref() {
            by_key.insert(key, id);
        }
        ids.push(id);
    }

    // The project's own timestamp moves with its board: a listing sorted by
    // "last touched" should not call a project untouched because the change
    // was a task.
    touch_project(&mut *tx, project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(ids)
}

/// A task exists, belongs to the same project, and may take children — and if
/// it does, which column it is sitting in.
///
/// Both directions of the one-level rule are checked — here, and in
/// {@link update_task} for the task being reparented. The project check is the
/// one thing stricter than the frontend, which never has to ask: a drag can
/// only land on a board that is already on screen, while an id typed into
/// `--parent` can name anything. The column comes back because a subtask with
/// no column of its own belongs in its parent's — see {@link create_tasks}.
/// "The same project" includes *no* project: a subtask of an unfiled task is
/// unfiled too, since a subtask cannot sit anywhere its parent does not.
async fn assert_can_parent(
    conn: &mut SqliteConnection,
    parent_id: i64,
    project_id: Option<i64>,
) -> Result<String, String> {
    let row = sqlx::query("SELECT parent_id, project_id, column_id FROM project_tasks WHERE id = ?1")
        .bind(parent_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let row = row.ok_or_else(|| format!("parent task {parent_id} does not exist"))?;
    let owner: Option<i64> = row.get("project_id");
    if owner != project_id {
        return Err(match owner {
            Some(owner) => format!("parent task {parent_id} belongs to project {owner}"),
            None => format!("parent task {parent_id} belongs to no project"),
        });
    }
    if row.get::<Option<i64>, _>("parent_id").is_some() {
        return Err("subtasks are one level deep: a subtask cannot have children".to_string());
    }
    Ok(row.get("column_id"))
}

async fn has_children(conn: &mut SqliteConnection, id: i64) -> Result<bool, String> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks WHERE parent_id = ?1")
        .bind(id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(n > 0)
}

/// Move a project's own `updated_at` when its board changes — and do nothing
/// at all for an unfiled task, which has no project to have been touched.
async fn touch_project(conn: &mut SqliteConnection, id: Option<i64>) -> Result<(), String> {
    let Some(id) = id else { return Ok(()) };
    sqlx::query("UPDATE projects SET updated_at = datetime('now') WHERE id = ?1")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn task(pool: &SqlitePool, id: i64) -> Result<Option<Task>, String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    task_on(&mut *conn, id).await
}

async fn task_on(conn: &mut SqliteConnection, id: i64) -> Result<Option<Task>, String> {
    let row = sqlx::query("SELECT * FROM project_tasks WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.as_ref().map(to_task))
}

/// Everything about a task *except* where it sits: `column_id`, `position` and
/// `done_at` are one fact in three columns and {@link move_task} is their only
/// writer, because it is the only thing that reads the board to learn whether
/// the destination is a `done` column.
#[derive(Default)]
pub struct TaskPatch {
    pub title: Option<String>,
    pub body: Option<Option<String>>,
    pub parent_id: Option<Option<i64>>,
    pub starts_at: Option<Option<String>>,
    pub due_at: Option<Option<String>>,
    pub estimate_minutes: Option<Option<i64>>,
}

pub async fn update_task(pool: &SqlitePool, id: i64, patch: &TaskPatch) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let existing = task_on(&mut *tx, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;

    if let Some(Some(parent)) = patch.parent_id {
        if parent == id {
            return Err("a task cannot be its own parent".to_string());
        }
        assert_can_parent(&mut *tx, parent, existing.project_id).await?;
        if has_children(&mut *tx, id).await? {
            return Err(
                "subtasks are one level deep: a task with children cannot have a parent".to_string(),
            );
        }
    }

    let mut sets: Vec<String> = Vec::new();
    let mut put = |column: &str| sets.push(format!("{column} = ?{}", sets.len() + 1));
    if patch.title.is_some() {
        put("title");
    }
    if patch.body.is_some() {
        put("body");
    }
    if patch.parent_id.is_some() {
        put("parent_id");
    }
    if patch.starts_at.is_some() {
        put("starts_at");
    }
    if patch.due_at.is_some() {
        put("due_at");
    }
    if patch.estimate_minutes.is_some() {
        put("estimate_minutes");
    }
    if sets.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "UPDATE project_tasks SET {}, updated_at = datetime('now') WHERE id = ?{}",
        sets.join(", "),
        sets.len() + 1
    );
    let mut q = sqlx::query(&sql);
    if let Some(v) = &patch.title {
        q = q.bind(v);
    }
    if let Some(v) = &patch.body {
        q = q.bind(v);
    }
    if let Some(v) = &patch.parent_id {
        q = q.bind(*v);
    }
    if let Some(v) = &patch.starts_at {
        q = q.bind(v);
    }
    if let Some(v) = &patch.due_at {
        q = q.bind(v);
    }
    if let Some(v) = &patch.estimate_minutes {
        q = q.bind(*v);
    }
    q.bind(id).execute(&mut *tx).await.map_err(|e| e.to_string())?;

    touch_project(&mut *tx, existing.project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// Deletes the task and, by the migration's self-referential cascade, its
/// subtasks. Returns how many rows went.
pub async fn delete_task(pool: &SqlitePool, id: i64) -> Result<u64, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let existing = task_on(&mut *tx, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;
    let children: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks WHERE parent_id = ?1")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM project_tasks WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    touch_project(&mut *tx, existing.project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(children as u64 + 1)
}

/// The gap at which fractional positions have to be given up on: repeated
/// midpoints halve it every time, and well before a double runs out the
/// midpoint stops landing strictly between its neighbours.
const MIN_GAP: f64 = 1e-6;

/// Renumber one column to 0, 1, 2, … so midpoints have room again.
async fn renumber_column(
    conn: &mut SqliteConnection,
    project_id: Option<i64>,
    column_id: &str,
) -> Result<(), String> {
    // `IS`: the unfiled tasks of one column are a group like any other.
    let rows = sqlx::query(
        "SELECT id FROM project_tasks WHERE project_id IS ?1 AND column_id = ?2
          ORDER BY position ASC, id ASC",
    )
    .bind(project_id)
    .bind(column_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;
    for (i, r) in rows.iter().enumerate() {
        sqlx::query("UPDATE project_tasks SET position = ?1, updated_at = datetime('now') WHERE id = ?2")
            .bind(i as f64)
            .bind(r.get::<i64, _>("id"))
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn position_of(conn: &mut SqliteConnection, id: i64) -> Result<Option<f64>, String> {
    sqlx::query_scalar("SELECT position FROM project_tasks WHERE id = ?1")
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())
}

/// Drop a task into a column, between two neighbours.
///
/// `above` is the task it should sit *after* and `below` the one it should sit
/// *before*; either may be absent, and both absent means the end of the
/// column. The new position is their midpoint, which is the whole point of
/// `position REAL`: a move writes one row instead of renumbering a column. The
/// one case that is not arithmetic is the gap underflowing ({@link MIN_GAP}) —
/// the column is renumbered to whole numbers and the midpoint is taken again.
///
/// Landing in a `kind: "done"` column stamps `done_at`; leaving one clears it.
/// The *kind* decides, never the name or the id, both of which are the user's
/// to change.
///
/// An unfiled task moves among the *unfiled* tasks of that column: `project_id
/// IS NULL` is a group like any other here, and its board is the default one.
pub async fn move_task(
    pool: &SqlitePool,
    id: i64,
    column_id: &str,
    above: Option<i64>,
    below: Option<i64>,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let existing = task_on(&mut *tx, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;
    let project = match existing.project_id {
        Some(id) => Some(
            project_on(&mut *tx, id)
                .await?
                .ok_or_else(|| format!("project {id} does not exist"))?,
        ),
        None => None,
    };
    let done = require_column(project.as_ref(), column_id)?.kind == "done";

    for neighbour in [above, below].into_iter().flatten() {
        let row = task_on(&mut *tx, neighbour)
            .await?
            .ok_or_else(|| format!("task {neighbour} does not exist"))?;
        if row.project_id != existing.project_id {
            return Err(format!("task {neighbour} does not belong to the same project"));
        }
        if row.column_id != column_id {
            return Err(format!(
                "task {neighbour} is in column \"{}\", not \"{column_id}\"",
                row.column_id
            ));
        }
    }

    let mut position = midpoint(&mut *tx, above, below).await?;
    if position.is_none() {
        renumber_column(&mut *tx, existing.project_id, column_id).await?;
        position = midpoint(&mut *tx, above, below).await?;
    }
    // Two neighbours still too close after a renumber would mean the column
    // holds more rows than a double can separate, which it cannot.
    let position = position.ok_or("could not find a position for the task")?;

    sqlx::query(
        r#"UPDATE project_tasks
              SET column_id  = ?1,
                  position   = ?2,
                  done_at    = CASE WHEN ?3 THEN COALESCE(done_at, datetime('now')) ELSE NULL END,
                  updated_at = datetime('now')
            WHERE id = ?4"#,
    )
    .bind(column_id)
    .bind(position)
    .bind(i64::from(done))
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    touch_project(&mut *tx, existing.project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// `None` means the neighbours are too close together to fit anything between.
async fn midpoint(
    conn: &mut SqliteConnection,
    above: Option<i64>,
    below: Option<i64>,
) -> Result<Option<f64>, String> {
    let lo = match above {
        Some(id) => position_of(conn, id).await?,
        None => None,
    };
    let hi = match below {
        Some(id) => position_of(conn, id).await?,
        None => None,
    };
    Ok(match (lo, hi) {
        (Some(lo), Some(hi)) => {
            if hi - lo < MIN_GAP {
                None
            } else {
                Some((lo + hi) / 2.0)
            }
        }
        (Some(lo), None) => Some(lo + 1.0),
        (None, Some(hi)) => Some(hi - 1.0),
        (None, None) => Some(0.0),
    })
}

/// The kind a task's column means **on its own board**, and `backlog` for a
/// column that board no longer has.
///
/// The fallback is not a shrug: `kindOf` in
/// `app/src/components/projects/universalTasks.ts` draws such a card in
/// Backlog for the same reason — the leftmost column, where a card's life
/// starts — because a card nothing draws is a card nobody can fix. The two
/// have to agree, or a refile would move a task somewhere other than the
/// column the universal board just showed it in.
fn kind_of(project: Option<&Project>, column_id: &str) -> String {
    board_of(project)
        .iter()
        .find(|c| c.id == column_id)
        .map(|c| c.kind.clone())
        .unwrap_or_else(|| "backlog".to_string())
}

/// File a task under another project, or under none at all — and take its
/// subtasks with it. Returns how many rows moved.
///
/// The one operation that writes `project_id` after a task exists, and the one
/// allowed to move a parent and its children at once: a subtask sits in its
/// parent's project, which {@link assert_can_parent} enforces in both
/// directions, so moving only one side of that pair would write the exact row
/// it refuses to create. A subtask on its own is therefore refused and names
/// its parent — there is no honest half of this move.
///
/// **The column maps across by *kind*, never by id.** A column id only means
/// something against the board it was checked against, and two projects share
/// nothing but what a column *means*: so the destination is the **first**
/// column of that kind on the destination's board — `columnForKind`'s rule in
/// `app/src/components/projects/universalTasks.ts`, which is what the
/// universal board's drag already writes. Entering a kind puts you at its
/// start, so a task filed into a default board lands in Todo rather than
/// skipping to In progress. A board with no column of that kind is refused
/// outright rather than quietly given the nearest one: there is no such thing
/// as the nearest kind.
///
/// `done_at` is still derived from the destination column's kind, exactly as
/// {@link move_task} derives it. The kind is preserved by construction, so
/// this normally changes nothing — except for the one case it has to, a task
/// whose old column its own board had dropped, which reads as `backlog` here
/// and must not arrive in a work column still stamped finished.
///
/// It **appends**, at the end of the destination column. `position` is a
/// fractional slot inside one project's column and nothing else, so there is
/// no slot in the destination to aim at and no order in the source worth
/// preserving; `MAX(position) + 1` over `project_id IS <destination>` is
/// `appendNeighbour`'s neighbour plus one, computed in the one place it can be
/// read atomically. `IS`, not `=`: filing *out* of every project is a group
/// like any other and `project_id = NULL` matches nothing.
pub async fn refile_task(
    pool: &SqlitePool,
    id: i64,
    project_id: Option<i64>,
) -> Result<u64, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let existing = task_on(&mut *tx, id)
        .await?
        .ok_or_else(|| format!("task {id} does not exist"))?;

    if let Some(parent) = existing.parent_id {
        return Err(format!(
            "task {id} is a subtask of task {parent}, and a subtask sits in its parent's \
             project \u{2014} refile task {parent} and this one travels with it"
        ));
    }
    // Already there: nothing to write, and writing anyway would append the
    // task to the end of the column it is already sitting in.
    if existing.project_id == project_id {
        return Ok(0);
    }

    let source = match existing.project_id {
        Some(pid) => Some(
            project_on(&mut *tx, pid)
                .await?
                .ok_or_else(|| format!("project {pid} does not exist"))?,
        ),
        None => None,
    };
    let target = match project_id {
        Some(pid) => Some(
            project_on(&mut *tx, pid)
                .await?
                .ok_or_else(|| format!("project {pid} does not exist"))?,
        ),
        None => None,
    };

    // The parent first, then its children in their own order, so the parent
    // takes the lower position in any column the two end up sharing.
    let rows = sqlx::query(
        "SELECT * FROM project_tasks WHERE parent_id = ?1 ORDER BY position ASC, id ASC",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let mut moving: Vec<Task> = Vec::with_capacity(rows.len() + 1);
    moving.push(existing.clone());
    moving.extend(rows.iter().map(to_task));

    for row in &moving {
        let kind = kind_of(source.as_ref(), &row.column_id);
        let column = board_of(target.as_ref())
            .iter()
            .find(|c| c.kind == kind)
            .ok_or_else(|| {
                let known: Vec<&str> = board_of(target.as_ref())
                    .iter()
                    .map(|c| c.id.as_str())
                    .collect();
                let whose = match &target {
                    Some(p) => format!("project {}", p.id),
                    None => "an unfiled task's board".to_string(),
                };
                format!(
                    "{whose} has no \"{kind}\" column, so task {} ({:?}) has nowhere to land \
                     (has: {})",
                    row.id,
                    row.title,
                    known.join(", ")
                )
            })?;
        let done = column.kind == "done";
        let next: f64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(position), -1) + 1 AS REAL) FROM project_tasks
              WHERE project_id IS ?1 AND column_id = ?2",
        )
        .bind(project_id)
        .bind(&column.id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        sqlx::query(
            r#"UPDATE project_tasks
                  SET project_id = ?1,
                      column_id  = ?2,
                      position   = ?3,
                      done_at    = CASE WHEN ?4 THEN COALESCE(done_at, datetime('now')) ELSE NULL END,
                      updated_at = datetime('now')
                WHERE id = ?5"#,
        )
        .bind(project_id)
        .bind(&column.id)
        .bind(next)
        .bind(i64::from(done))
        .bind(row.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    // Both boards changed: the one that lost the task and the one that gained
    // it. `None` is a no-op, so neither side spells the absence of a project.
    touch_project(&mut *tx, existing.project_id).await?;
    touch_project(&mut *tx, project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(moving.len() as u64)
}

// ── Timestamps ───────────────────────────────────────────────────────────────

/// Check that a date is a date, and hand back exactly the text that came in.
///
/// **Nothing is normalised.** Times are stored as each source gives them (see
/// `docs/calendar.md`): rewriting a zone here would only add a way to be
/// wrong, and the app reads these with `new Date`, which understands all of
/// the shapes below. Accepted: `YYYY-MM-DD`, and that plus `T` or a space and
/// `HH:MM[:SS[.fff]]`, optionally followed by `Z` or `±HH:MM`.
pub fn check_iso8601(value: &str) -> Result<String, String> {
    let text = value.trim();
    let bad = || format!("\"{text}\" is not an ISO 8601 date (want 2026-09-20 or 2026-09-20T23:59:00Z)");
    let bytes = text.as_bytes();
    let digits = |from: usize, n: usize| -> Option<u32> {
        let slice = text.get(from..from + n)?;
        if slice.len() == n && slice.bytes().all(|b| b.is_ascii_digit()) {
            slice.parse().ok()
        } else {
            None
        }
    };
    let in_range = |v: Option<u32>, lo: u32, hi: u32| v.filter(|v| *v >= lo && *v <= hi).is_some();

    if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(bad());
    }
    if digits(0, 4).is_none()
        || !in_range(digits(5, 2), 1, 12)
        || !in_range(digits(8, 2), 1, 31)
    {
        return Err(bad());
    }
    if bytes.len() == 10 {
        return Ok(text.to_string());
    }
    if !matches!(bytes[10], b'T' | b't' | b' ') || bytes.len() < 16 || bytes[13] != b':' {
        return Err(bad());
    }
    if !in_range(digits(11, 2), 0, 23) || !in_range(digits(14, 2), 0, 59) {
        return Err(bad());
    }
    let mut i = 16;
    if bytes.get(i) == Some(&b':') {
        if !in_range(digits(i + 1, 2), 0, 60) {
            return Err(bad());
        }
        i += 3;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while bytes.get(i).is_some_and(|b| b.is_ascii_digit()) {
            i += 1;
        }
        if i == start {
            return Err(bad());
        }
    }
    match bytes.get(i) {
        None => Ok(text.to_string()),
        Some(b'Z') | Some(b'z') if i + 1 == bytes.len() => Ok(text.to_string()),
        Some(b'+') | Some(b'-') => {
            let rest = &text[i + 1..];
            let ok = match rest.len() {
                5 => rest.as_bytes()[2] == b':' && in_range(digits(i + 1, 2), 0, 23)
                    && in_range(digits(i + 4, 2), 0, 59),
                4 => in_range(digits(i + 1, 2), 0, 23) && in_range(digits(i + 3, 2), 0, 59),
                _ => false,
            };
            if ok { Ok(text.to_string()) } else { Err(bad()) }
        }
        _ => Err(bad()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_shapes_the_library_actually_stores() {
        // Canvas UTC, a bare date, SQLite's own datetime('now'), an Echo360
        // wall clock, and an offset — all kept verbatim.
        for good in [
            "2026-09-20",
            "2026-09-20T23:59:00Z",
            "2026-09-20T23:59Z",
            "2026-09-20 13:05:00",
            "2026-09-20T13:05:00.250+10:00",
            "2026-09-20T13:05:00+1000",
        ] {
            assert_eq!(check_iso8601(good).unwrap(), good, "{good}");
        }
    }

    #[test]
    fn refuses_what_is_not_a_date() {
        for bad in ["next friday", "20/09/2026", "2026-13-01", "2026-09-32", "2026-09-20T25:00Z", "2026-09-20T13:05:00 AEST", ""] {
            assert!(check_iso8601(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_new_board_is_the_apps_board() {
        let columns = default_columns();
        assert_eq!(columns.len(), 4);
        assert_eq!(columns[0].id, "backlog");
        assert_eq!(columns.last().unwrap().kind, "done");
    }

    /// The batch's contract, since an agent writes this JSON by hand.
    #[test]
    fn batch_items_parse_from_the_documented_json() {
        let items: Vec<NewTask> = serde_json::from_str(
            r#"[{"title":"Read the brief","column":"todo","due":"2026-09-20T23:59:00Z"},
                {"title":"Draft the intro","parent":1,"estimate":90},
                {"title":"Cite sources","parent":"intro","body":"APA"}]"#,
        )
        .unwrap();
        assert_eq!(items.len(), 3);
        assert!(matches!(items[1].parent, Some(ParentRef::Id(1))));
        assert!(matches!(items[2].parent, Some(ParentRef::Key(ref k)) if k == "intro"));
        assert_eq!(items[1].estimate, Some(90));
        // A typo'd field is refused rather than silently dropped.
        assert!(serde_json::from_str::<Vec<NewTask>>(r#"[{"title":"x","deu":"2026-01-01"}]"#).is_err());
    }
}

/// Migration 37, and the writers over the schema it leaves behind.
#[cfg(test)]
mod unfiled_tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::{Row, SqlitePool};

    use super::*;

    /// The tables migration 37 touches, **as migration 27 wrote them** —
    /// `project_id INTEGER NOT NULL` included, since that constraint is the
    /// thing being rebuilt away and a fixture that omitted it would assert
    /// nothing. `subjects` is here only because `PROJECT_SELECT` joins it.
    ///
    /// Foreign keys are on: sqlx's default, and the app's, which is what makes
    /// the copy's ordering and `defer_foreign_keys` load-bearing rather than
    /// decorative.
    async fn pre_37() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::raw_sql(
            "CREATE TABLE subjects (id INTEGER PRIMARY KEY, code TEXT);
             CREATE TABLE projects (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 subject_id  INTEGER REFERENCES subjects(id) ON DELETE SET NULL,
                 name        TEXT    NOT NULL,
                 brief       TEXT,
                 status      TEXT    NOT NULL DEFAULT 'active',
                 starts_at   TEXT,
                 due_at      TEXT,
                 columns     TEXT    NOT NULL,
                 tags        TEXT    NOT NULL DEFAULT '[]',
                 event_id    TEXT,
                 position    REAL    NOT NULL DEFAULT 0,
                 source      TEXT    NOT NULL DEFAULT 'manual',
                 created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE project_tasks (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                 parent_id   INTEGER REFERENCES project_tasks(id) ON DELETE CASCADE,
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
             CREATE INDEX idx_project_tasks_project ON project_tasks(project_id, column_id, position);
             CREATE INDEX idx_project_tasks_due ON project_tasks(due_at);",
        )
        .execute(&pool)
        .await
        .expect("pre-37 schema");
        pool
    }

    /// The same pool with migration 37 applied — what every writer test runs
    /// against, so the writers are exercised over the schema the app will
    /// actually have rather than one hand-written to suit them.
    async fn migrated() -> SqlitePool {
        let pool = pre_37().await;
        sqlx::raw_sql(UNFILED_TASKS_SQL)
            .execute(&pool)
            .await
            .expect("migration 37");
        pool
    }

    async fn seed_project(pool: &SqlitePool, name: &str) -> i64 {
        create_project(
            pool,
            &NewProject {
                name: name.to_string(),
                subject_id: None,
                brief: None,
                starts_at: None,
                due_at: None,
                tags: vec![],
                source: "manual".to_string(),
            },
        )
        .await
        .expect("project")
    }

    async fn task_row(pool: &SqlitePool, id: i64) -> Option<(Option<i64>, Option<i64>, String)> {
        let row = sqlx::query("SELECT project_id, parent_id, title FROM project_tasks WHERE id = ?1")
            .bind(id)
            .fetch_optional(pool)
            .await
            .expect("read");
        row.map(|r| (r.get("project_id"), r.get("parent_id"), r.get("title")))
    }

    async fn count(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks")
            .fetch_one(pool)
            .await
            .expect("count")
    }

    /// The rebuild's whole risk, as a test.
    ///
    /// A parent and its subtasks go through a `CREATE`/`INSERT`/`DROP`/`RENAME`
    /// with the self-reference live. The way to lose them is the `DROP TABLE`
    /// cascading into the copy, which is exactly what happens if the new
    /// table's `parent_id` names the old one — flip that single word in
    /// {@link UNFILED_TASKS_SQL} and this fails on the first assertion. The
    /// cascades are then asserted *afterwards*, to prove the rename carried the
    /// foreign keys over rather than quietly dropping them.
    #[tokio::test]
    async fn the_rebuild_keeps_parents_children_and_their_cascades() {
        let pool = pre_37().await;
        let project = seed_project(&pool, "Essay").await;
        // Written straight to SQL: `create_tasks` writes the *new* shape, and
        // what is being tested is the copy of rows that predate it.
        sqlx::raw_sql(&format!(
            "INSERT INTO project_tasks (id, project_id, parent_id, title, column_id, position)
             VALUES (1, {project}, NULL, 'Draft', 'todo', 0),
                    (2, {project}, 1, 'Outline', 'todo', 1),
                    (3, {project}, 1, 'Cite', 'done', 0),
                    (4, {project}, NULL, 'Proofread', 'backlog', 0);"
        ))
        .execute(&pool)
        .await
        .expect("seed tasks");

        sqlx::raw_sql(UNFILED_TASKS_SQL)
            .execute(&pool)
            .await
            .expect("migration 37");

        assert_eq!(count(&pool).await, 4, "every row survived the rebuild");
        assert_eq!(task_row(&pool, 2).await.unwrap().1, Some(1), "subtask still names its parent");
        assert_eq!(task_row(&pool, 3).await.unwrap().1, Some(1));
        assert_eq!(task_row(&pool, 4).await.unwrap().0, Some(project), "and its project");

        // Ids continue rather than restarting: AUTOINCREMENT's sequence is
        // re-seeded by the copy, so a new task cannot collide with an old one.
        let fresh = create_tasks(
            &pool,
            Some(project),
            &[NewTask { title: "Submit".into(), ..Default::default() }],
            "manual",
        )
        .await
        .expect("create after the rebuild");
        assert!(fresh[0] > 4, "new ids continue past the copied ones (got {})", fresh[0]);

        // The self-reference survived the rename.
        delete_task(&pool, 1).await.expect("delete the parent");
        assert!(task_row(&pool, 2).await.is_none(), "parent → subtask cascade");
        assert!(task_row(&pool, 3).await.is_none());

        // And so did the project reference.
        sqlx::query("DELETE FROM projects WHERE id = ?1")
            .bind(project)
            .execute(&pool)
            .await
            .expect("delete the project");
        assert_eq!(count(&pool).await, 0, "project → tasks cascade");
    }

    /// A task with no project at all, through the writers rather than through
    /// SQL: created, placed on the default board, moved, patched and read back.
    #[tokio::test]
    async fn an_unfiled_task_round_trips() {
        let pool = migrated().await;

        let ids = create_tasks(
            &pool,
            None,
            &[
                NewTask { title: "Renew Myki".into(), ..Default::default() },
                NewTask {
                    title: "Book a haircut".into(),
                    column: Some("todo".into()),
                    key: Some("hair".into()),
                    ..Default::default()
                },
                NewTask {
                    title: "Ring the salon".into(),
                    parent: Some(ParentRef::Key("hair".into())),
                    ..Default::default()
                },
            ],
            "manual",
        )
        .await
        .expect("unfiled create");

        let first = task(&pool, ids[0]).await.unwrap().unwrap();
        assert_eq!(first.project_id, None, "no project, not a project called Inbox");
        // No `--column` and no parent: the first column of the board it is
        // checked against, which for an unfiled task is the default one.
        assert_eq!(first.column_id, "backlog");

        let child = task(&pool, ids[2]).await.unwrap().unwrap();
        assert_eq!(child.parent_id, Some(ids[1]));
        assert_eq!(child.project_id, None, "a subtask of an unfiled task is unfiled");
        assert_eq!(child.column_id, "todo", "and inherits its parent's column");

        // Positions are taken among the *unfiled* tasks of that column, which
        // `project_id = NULL` would never have matched.
        assert_eq!(first.position, 0.0);

        // The done column of the default board still decides done-ness.
        move_task(&pool, ids[0], "done", None, None).await.expect("move");
        let done = task(&pool, ids[0]).await.unwrap().unwrap();
        assert_eq!(done.column_id, "done");
        assert!(done.done_at.is_some(), "landing in a done column stamps done_at");
        move_task(&pool, ids[0], "doing", None, None).await.expect("move back");
        assert!(task(&pool, ids[0]).await.unwrap().unwrap().done_at.is_none(), "leaving clears it");

        // Everything but where it sits still patches, with no project to touch.
        update_task(
            &pool,
            ids[0],
            &TaskPatch { title: Some("Renew the Myki".into()), ..Default::default() },
        )
        .await
        .expect("patch");
        assert_eq!(task_row(&pool, ids[0]).await.unwrap().2, "Renew the Myki");
    }

    /// The board an unfiled task is checked against is the default one — which
    /// is a real check, not a waiver.
    #[tokio::test]
    async fn an_unfiled_column_is_checked_against_the_default_board() {
        let pool = migrated().await;
        let err = create_tasks(
            &pool,
            None,
            &[NewTask { title: "x".into(), column: Some("inbox".into()), ..Default::default() }],
            "manual",
        )
        .await
        .expect_err("an unknown column is refused");
        assert!(err.contains("an unfiled task"), "{err}");
        assert!(err.contains("backlog, todo, doing, done"), "{err}");
        assert_eq!(count(&pool).await, 0, "and nothing was written");
    }

    /// A subtask cannot cross the line in either direction: its parent's
    /// project is its own, and "no project" is one of the answers.
    #[tokio::test]
    async fn a_subtask_cannot_cross_between_filed_and_unfiled() {
        let pool = migrated().await;
        let project = seed_project(&pool, "Essay").await;
        let filed = create_tasks(
            &pool,
            Some(project),
            &[NewTask { title: "Draft".into(), ..Default::default() }],
            "manual",
        )
        .await
        .expect("filed parent")[0];
        let unfiled = create_tasks(
            &pool,
            None,
            &[NewTask { title: "Errand".into(), ..Default::default() }],
            "manual",
        )
        .await
        .expect("unfiled parent")[0];

        let err = create_tasks(
            &pool,
            None,
            &[NewTask {
                title: "Outline".into(),
                parent: Some(ParentRef::Id(filed)),
                ..Default::default()
            }],
            "manual",
        )
        .await
        .expect_err("an unfiled subtask of a filed parent");
        assert!(err.contains(&format!("belongs to project {project}")), "{err}");

        let err = create_tasks(
            &pool,
            Some(project),
            &[NewTask {
                title: "Outline".into(),
                parent: Some(ParentRef::Id(unfiled)),
                ..Default::default()
            }],
            "manual",
        )
        .await
        .expect_err("a filed subtask of an unfiled parent");
        assert!(err.contains("belongs to no project"), "{err}");
    }

    /// Filing a task out of a project: the column maps across by kind, and
    /// nothing is left behind on the board it came from.
    #[tokio::test]
    async fn refiling_a_task_out_of_a_project_maps_the_column_by_kind() {
        let pool = migrated().await;
        let project = seed_project(&pool, "Essay").await;
        let id = create_tasks(
            &pool,
            Some(project),
            &[NewTask { title: "Draft".into(), column: Some("doing".into()), ..Default::default() }],
            "manual",
        )
        .await
        .expect("filed")[0];

        assert_eq!(refile_task(&pool, id, None).await.expect("unfile"), 1);
        let row = task(&pool, id).await.unwrap().unwrap();
        assert_eq!(row.project_id, None);
        // `doing` is `active`, and the **first** active column of the board it
        // arrives on is `todo` — entering a kind puts you at its start.
        assert_eq!(row.column_id, "todo");
        assert!(row.done_at.is_none());
        // And nothing of it stayed on the project.
        assert!(tasks(&pool, project).await.unwrap().is_empty());

        // Filing it back the other way is the same rule read backwards.
        assert_eq!(refile_task(&pool, id, Some(project)).await.expect("file"), 1);
        let row = task(&pool, id).await.unwrap().unwrap();
        assert_eq!(row.project_id, Some(project));
        assert_eq!(row.column_id, "todo");
        // A second refile to where it already is writes nothing at all.
        assert_eq!(refile_task(&pool, id, Some(project)).await.unwrap(), 0);
    }

    /// A subtask sits in its parent's project, so a refile moves both — each
    /// by its own column's kind, and the parent is refused nothing on the way.
    #[tokio::test]
    async fn refiling_carries_the_subtasks_with_it() {
        let pool = migrated().await;
        let project = seed_project(&pool, "Essay").await;
        let ids = create_tasks(
            &pool,
            None,
            &[
                NewTask { title: "Essay".into(), column: Some("doing".into()), key: Some("p".into()), ..Default::default() },
                NewTask {
                    title: "Outline".into(),
                    parent: Some(ParentRef::Key("p".into())),
                    column: Some("done".into()),
                    ..Default::default()
                },
                NewTask {
                    title: "Draft".into(),
                    parent: Some(ParentRef::Key("p".into())),
                    ..Default::default()
                },
            ],
            "manual",
        )
        .await
        .expect("unfiled breakdown");

        assert_eq!(refile_task(&pool, ids[0], Some(project)).await.expect("refile"), 3);
        for id in &ids {
            let row = task(&pool, *id).await.unwrap().unwrap();
            assert_eq!(row.project_id, Some(project), "task {id} came along");
        }
        // Each row kept its own kind: the parent was in flight, one subtask
        // was finished, the other inherited the parent's column.
        assert_eq!(task(&pool, ids[1]).await.unwrap().unwrap().column_id, "done");
        assert!(task(&pool, ids[1]).await.unwrap().unwrap().done_at.is_some());
        assert_eq!(task(&pool, ids[2]).await.unwrap().unwrap().column_id, "todo");
        // Two rows landing in the same column do not land on top of each other.
        let parent = task(&pool, ids[0]).await.unwrap().unwrap();
        let sibling = task(&pool, ids[2]).await.unwrap().unwrap();
        assert_ne!(parent.position, sibling.position);
        assert!(parent.position < sibling.position, "the parent goes in first");

        // And a subtask cannot be refiled on its own: that is the one row
        // `assert_can_parent` exists to refuse.
        let err = refile_task(&pool, ids[1], None).await.expect_err("a lone subtask");
        assert!(err.contains(&format!("subtask of task {}", ids[0])), "{err}");
        assert_eq!(
            task(&pool, ids[1]).await.unwrap().unwrap().project_id,
            Some(project),
            "and it did not move"
        );
    }

    /// A destination with no column of the source's kind is refused, whole —
    /// there is no nearest kind to fall back to.
    #[tokio::test]
    async fn refiling_refuses_a_board_with_no_column_of_that_kind() {
        let pool = migrated().await;
        let project = seed_project(&pool, "Essay").await;
        // A board of one active column: nothing on it means "finished".
        sqlx::query("UPDATE projects SET columns = ?1 WHERE id = ?2")
            .bind(r#"[{"id":"now","name":"Now","kind":"active"}]"#)
            .bind(project)
            .execute(&pool)
            .await
            .expect("narrow board");

        let id = create_tasks(
            &pool,
            None,
            &[NewTask { title: "Submit".into(), column: Some("done".into()), ..Default::default() }],
            "manual",
        )
        .await
        .expect("unfiled")[0];

        let err = refile_task(&pool, id, Some(project))
            .await
            .expect_err("no done column to land in");
        assert!(err.contains("has no \"done\" column"), "{err}");
        assert_eq!(task(&pool, id).await.unwrap().unwrap().project_id, None, "nothing moved");
    }

    /// The cross-project read `oculus task list` prints: unfiled first, then a
    /// project's own rows in `position` order.
    #[tokio::test]
    async fn all_tasks_spans_projects_and_can_ask_for_the_unfiled_alone() {
        let pool = migrated().await;
        let project = seed_project(&pool, "Essay").await;
        create_tasks(
            &pool,
            Some(project),
            &[NewTask { title: "Draft".into(), ..Default::default() }],
            "manual",
        )
        .await
        .expect("filed");
        create_tasks(
            &pool,
            None,
            &[NewTask { title: "Errand".into(), ..Default::default() }],
            "manual",
        )
        .await
        .expect("unfiled");

        let all = all_tasks(&pool, TaskScope::All).await.expect("all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].title, "Errand", "unfiled first");
        assert_eq!(all[1].project_id, Some(project));

        let unfiled = all_tasks(&pool, TaskScope::Unfiled).await.expect("unfiled");
        assert_eq!(unfiled.len(), 1);
        assert_eq!(unfiled[0].title, "Errand");
    }

    /// Two unfiled tasks and a filed one in the same column id are two separate
    /// runs of positions, and a move only ever looks at its own.
    #[tokio::test]
    async fn a_move_will_not_take_a_neighbour_from_the_other_side() {
        let pool = migrated().await;
        let project = seed_project(&pool, "Essay").await;
        let filed = create_tasks(
            &pool,
            Some(project),
            &[NewTask { title: "Draft".into(), column: Some("todo".into()), ..Default::default() }],
            "manual",
        )
        .await
        .expect("filed")[0];
        let unfiled = create_tasks(
            &pool,
            None,
            &[NewTask { title: "Errand".into(), column: Some("todo".into()), ..Default::default() }],
            "manual",
        )
        .await
        .expect("unfiled")[0];

        let err = move_task(&pool, unfiled, "todo", Some(filed), None)
            .await
            .expect_err("a neighbour from another project");
        assert!(err.contains("does not belong to the same project"), "{err}");
    }
}
