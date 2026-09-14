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
//! - **A column id is checked against the project's own `columns`** before
//!   anything is filed under it. A task in a column no view renders is not
//!   misfiled, it is invisible — and here `--column` is free text an agent
//!   typed, which is the case the frontend's `requireColumn` was written for.
//! - **`done_at` is derived from the destination column's `kind`**, never
//!   passed in, on create as well as on move. So "which column is it in" and
//!   "is it finished" cannot disagree whichever door the task came through.
//! - **Subtasks are one level deep**, enforced in code because SQLite cannot
//!   express "the parent has no parent" as a constraint. The board draws a
//!   task and its children, not a tree; a grandchild would never be drawn.
//!
//! Schema is migration 27 in `lib.rs`. Nothing here creates it — same rule as
//! `store.rs`: a fresh machine opens the app once first.

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection, SqlitePool};

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

/// Resolve a column id against a project's board, or refuse with the ids it
/// does have — the agent's next attempt should not need a second command to
/// find out what the board is called.
fn require_column<'a>(project: &'a Project, column_id: &str) -> Result<&'a Column, String> {
    project.columns.iter().find(|c| c.id == column_id).ok_or_else(|| {
        let known: Vec<&str> = project.columns.iter().map(|c| c.id.as_str()).collect();
        format!(
            "project {} has no column \"{column_id}\" (has: {})",
            project.id,
            known.join(", ")
        )
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
    pub position: f64,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Task {
    pub id: i64,
    pub project_id: i64,
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
       p.status, p.starts_at, p.due_at, p.columns, p.position, p.source,
       p.created_at, p.updated_at
  FROM projects p
  LEFT JOIN subjects s ON s.id = p.subject_id"#;

fn to_project(r: &sqlx::sqlite::SqliteRow) -> Project {
    // A board that will not parse is a board nothing can be drawn on, and the
    // tasks still name their column by id — so fall back rather than fail the
    // whole read on one bad row, the way `toProject` does.
    let columns = serde_json::from_str::<Vec<Column>>(&r.get::<String, _>("columns"))
        .ok()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(default_columns);
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
    sqlx::query(
        r#"INSERT INTO projects
             (subject_id, name, brief, status, starts_at, due_at, columns, position, source)
           VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?7, ?8)"#,
    )
    .bind(input.subject_id)
    .bind(&input.name)
    .bind(&input.brief)
    .bind(&input.starts_at)
    .bind(&input.due_at)
    .bind(columns)
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
    if sets.is_empty() {
        return Ok(());
    }
    let sql = format!(
        "UPDATE projects SET {}, updated_at = datetime('now') WHERE id = ?{}",
        sets.join(", "),
        sets.len() + 1
    );
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
    project_id: i64,
    items: &[NewTask],
    source: &str,
) -> Result<Vec<i64>, String> {
    if items.is_empty() {
        return Err("no tasks given".to_string());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let project = project_on(&mut *tx, project_id)
        .await?
        .ok_or_else(|| format!("project {project_id} does not exist"))?;

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
        let column = match &item.column {
            Some(id) => require_column(&project, id).map_err(|e| format!("{}{e}", where_()))?,
            None => parent_column
                .as_deref()
                // A column the board has since dropped falls back rather than
                // failing: the parent's row is already there either way.
                .and_then(|id| project.columns.iter().find(|c| c.id == id))
                .or_else(|| project.columns.first())
                .ok_or("project has no columns")?,
        };
        let done = column.kind == "done";

        let next: f64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(position), -1) + 1 AS REAL) FROM project_tasks
              WHERE project_id = ?1 AND column_id = ?2",
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
async fn assert_can_parent(
    conn: &mut SqliteConnection,
    parent_id: i64,
    project_id: i64,
) -> Result<String, String> {
    let row = sqlx::query("SELECT parent_id, project_id, column_id FROM project_tasks WHERE id = ?1")
        .bind(parent_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let row = row.ok_or_else(|| format!("parent task {parent_id} does not exist"))?;
    let owner: i64 = row.get("project_id");
    if owner != project_id {
        return Err(format!("parent task {parent_id} belongs to project {owner}"));
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

async fn touch_project(conn: &mut SqliteConnection, id: i64) -> Result<(), String> {
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
    project_id: i64,
    column_id: &str,
) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT id FROM project_tasks WHERE project_id = ?1 AND column_id = ?2
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
    let project = project_on(&mut *tx, existing.project_id)
        .await?
        .ok_or_else(|| format!("project {} does not exist", existing.project_id))?;
    let done = require_column(&project, column_id)?.kind == "done";

    for neighbour in [above, below].into_iter().flatten() {
        let row = task_on(&mut *tx, neighbour)
            .await?
            .ok_or_else(|| format!("task {neighbour} does not exist"))?;
        if row.project_id != existing.project_id {
            return Err(format!("task {neighbour} is in another project"));
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
