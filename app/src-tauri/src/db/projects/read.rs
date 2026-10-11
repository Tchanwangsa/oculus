use sqlx::{Row, SqliteConnection, SqlitePool};

use super::rows::{to_project, to_task, Project, Task, PROJECT_SELECT};

/// `Personal` is `subject_id IS NULL`.
pub enum SubjectFilter {
    Any,
    Personal,
    Subject(i64),
}

/// `status` is `active`, `archived`, or `all`.
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
    // Always bound: sqlx refuses a bind the statement does not use.
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

pub(super) async fn project_on(
    conn: &mut SqliteConnection,
    id: i64,
) -> Result<Option<Project>, String> {
    let row = sqlx::query(&format!("{PROJECT_SELECT}\n WHERE p.id = ?1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.as_ref().map(to_project))
}

/// Every task of one project in `position` order. Not ordered by `column_id`
/// (that is alphabetical); the board's order is `columns` and the caller groups.
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

/// The app's `TaskScope` (`app/src/hooks/data/useTaskList.ts`).
pub enum TaskScope {
    All,
    Unfiled,
}

/// Every task across projects — `oculus task list` without `-p`. Ordered
/// unfiled-first, then by project, then `position`, because the caller prints
/// one board per project (not the app's due-date `UNIVERSAL_ORDER`).
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

/// `(total, done)` for one project.
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
