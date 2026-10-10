use sqlx::SqlitePool;

use crate::db::projects::columns::board_of;
use crate::db::projects::read::project_on;
use crate::db::projects::rows::{to_task, Task};

use super::lookup::{task_on, touch_project};
use super::mover::kind_of;

/// File a task under another project (or none), taking its subtasks with it;
/// returns how many rows moved. A lone subtask is refused — it sits in its
/// parent's project ({@link assert_can_parent}).
///
/// The column maps across by *kind*, never id: the first column of that kind on
/// the destination board (`columnForUniversal` in
/// `app/src/components/projects/tasks/universalTasks.ts`); none of that kind is an
/// error. `done_at` is re-derived as in {@link move_task}, and rows append at
/// the end of the column. `IS`, not `=`, for the NULL project.
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

    // Parent first, so it takes the lower position in a shared column.
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

    touch_project(&mut *tx, existing.project_id).await?;
    touch_project(&mut *tx, project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(moving.len() as u64)
}
