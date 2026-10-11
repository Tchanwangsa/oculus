use sqlx::{Row, SqliteConnection, SqlitePool};

use crate::db::projects::columns::{board_of, require_column};
use crate::db::projects::read::project_on;
use crate::db::projects::rows::Project;

use super::lookup::{task_on, touch_project};

/// Below this gap a column is renumbered before taking a midpoint.
const MIN_GAP: f64 = 1e-6;

/// Renumber one column to 0, 1, 2, … so midpoints have room again.
async fn renumber_column(
    conn: &mut SqliteConnection,
    project_id: Option<i64>,
    column_id: &str,
) -> Result<(), String> {
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
        sqlx::query(
            "UPDATE project_tasks SET position = ?1, updated_at = datetime('now') WHERE id = ?2",
        )
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

/// Drop a task into a column after `above` and before `below` (either may be
/// absent) at their midpoint, renumbering the column if the gap underflows
/// ({@link MIN_GAP}). A `done`-kind column stamps `done_at`; leaving clears it.
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
            return Err(format!(
                "task {neighbour} does not belong to the same project"
            ));
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

/// A column's kind on its own board, `backlog` for one the board dropped —
/// must agree with `universalColumnOf` in `app/src/components/projects/tasks/universalTasks.ts`.
pub(super) fn kind_of(project: Option<&Project>, column_id: &str) -> String {
    board_of(project)
        .iter()
        .find(|c| c.id == column_id)
        .map(|c| c.kind.clone())
        .unwrap_or_else(|| "backlog".to_string())
}
