use serde::Deserialize;
use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::db::projects::columns::{board_of, require_column};
use crate::db::projects::read::project_on;

use super::lookup::touch_project;

/// An existing task's id, or the `key` of an earlier item in the same batch.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ParentRef {
    Id(i64),
    Key(String),
}

/// One `--batch` item: the field names are the batch's contract.
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
    /// Referred to by a later item's `parent`. Never stored.
    #[serde(default)]
    pub key: Option<String>,
}

/// Create tasks and return their ids in input order. One transaction: any
/// rejected item rolls the whole batch back.
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
        // Prefix errors with the item only in a batch.
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
            Some(ParentRef::Key(k)) => Some(*by_key.get(k.as_str()).ok_or_else(|| {
                format!("{}no earlier task in this batch has key \"{k}\"", where_())
            })?),
        };
        let mut parent_column: Option<String> = None;
        if let Some(parent) = parent_id {
            parent_column = Some(
                assert_can_parent(&mut *tx, parent, project_id)
                    .await
                    .map_err(|e| format!("{}{e}", where_()))?,
            );
        }

        // A subtask with no `--column` inherits its parent's, not the first
        // column: the Backlog view lists only top-level tasks, so it would vanish.
        let board = board_of(project.as_ref());
        let column = match &item.column {
            Some(id) => {
                require_column(project.as_ref(), id).map_err(|e| format!("{}{e}", where_()))?
            }
            None => parent_column
                .as_deref()
                .and_then(|id| board.iter().find(|c| c.id == id))
                .or_else(|| board.first())
                .ok_or("project has no columns")?,
        };
        let done = column.kind == "done";

        // `IS`, not `=`: `project_id = NULL` matches nothing.
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

    touch_project(&mut *tx, project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(ids)
}

/// Check a would-be parent exists, is in the same project (NULL included) and
/// is not itself a subtask; returns its column. {@link update_task} checks the
/// other direction.
pub(super) async fn assert_can_parent(
    conn: &mut SqliteConnection,
    parent_id: i64,
    project_id: Option<i64>,
) -> Result<String, String> {
    let row =
        sqlx::query("SELECT parent_id, project_id, column_id FROM project_tasks WHERE id = ?1")
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
