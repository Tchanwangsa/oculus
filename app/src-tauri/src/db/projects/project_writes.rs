use sqlx::SqlitePool;

use super::columns::default_columns;
use super::rows::normalise_tags;

pub struct NewProject {
    pub name: String,
    pub subject_id: Option<i64>,
    pub brief: Option<String>,
    pub starts_at: Option<String>,
    pub due_at: Option<String>,
    pub tags: Vec<String>,
    /// `manual` | `agent`.
    pub source: String,
}

/// Create a project at the end of the list with the default board.
pub async fn create_project(pool: &SqlitePool, input: &NewProject) -> Result<i64, String> {
    let next: f64 =
        sqlx::query_scalar("SELECT CAST(COALESCE(MAX(position), -1) + 1 AS REAL) FROM projects")
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
    /// Replaces the whole set; `Some(vec![])` clears it.
    pub tags: Option<Vec<String>>,
}

pub async fn update_project(
    pool: &SqlitePool,
    id: i64,
    patch: &ProjectPatch,
) -> Result<(), String> {
    // Placeholders numbered as collected: sqlx refuses a statement whose
    // highest parameter is lower than the number of binds.
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
    // Outlives `q`, which borrows it.
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
        tags_json =
            serde_json::to_string(&normalise_tags(v.iter().cloned())).map_err(|e| e.to_string())?;
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
