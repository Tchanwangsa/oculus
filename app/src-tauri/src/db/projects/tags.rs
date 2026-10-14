//! The task tag registry (`task_tags`) and the links from tasks to it
//! (`task_tag_links`) — migration 47, mirrored by `app/src/lib/planning/projects/`.
//!
//! A tag is one shared row, so renaming or recolouring changes every task that
//! carries it and deleting removes it everywhere. Names are unique ignoring
//! case; colours are palette keys, never hex values.

use serde::Serialize;
use sqlx::{Row, SqliteConnection, SqlitePool};

use super::tasks::{task_on, touch_project, touch_task};

/// The only valid colours, in the palette's order.
pub const TAG_COLORS: [&str; 8] = [
    "slate", "indigo", "green", "amber", "rose", "purple", "teal", "orange",
];

/// The colour of a tag made without one, including one an agent names on the fly.
pub const DEFAULT_TAG_COLOR: &str = "slate";

/// The longest a tag name may be, in characters.
pub const MAX_TAG_NAME: usize = 40;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TaskTag {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub position: f64,
    pub created_at: String,
    /// How many tasks carry it.
    pub uses: i64,
}

const TAG_SELECT: &str = r#"SELECT g.id, g.name, g.color, g.position, g.created_at,
       (SELECT COUNT(*) FROM task_tag_links l WHERE l.tag_id = g.id) AS uses
  FROM task_tags g"#;

fn to_tag(r: &sqlx::sqlite::SqliteRow) -> TaskTag {
    TaskTag {
        id: r.get("id"),
        name: r.get("name"),
        color: r.get("color"),
        position: r.get("position"),
        created_at: r.get("created_at"),
        uses: r.get("uses"),
    }
}

/// A palette key, lower-cased; anything else is refused with the palette.
pub fn check_tag_color(color: &str) -> Result<String, String> {
    let key = color.trim().to_lowercase();
    if TAG_COLORS.contains(&key.as_str()) {
        return Ok(key);
    }
    Err(format!(
        "unknown tag colour \"{}\" (has: {})",
        color.trim(),
        TAG_COLORS.join(", ")
    ))
}

/// Whitespace collapsed; empty or over {@link MAX_TAG_NAME} is refused.
pub fn normalise_tag_name(name: &str) -> Result<String, String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err("a tag needs a name".to_string());
    }
    if name.chars().count() > MAX_TAG_NAME {
        return Err(format!(
            "tag names are at most {MAX_TAG_NAME} characters (\"{name}\" has {})",
            name.chars().count()
        ));
    }
    Ok(name)
}

pub async fn list_tags(pool: &SqlitePool) -> Result<Vec<TaskTag>, String> {
    let rows = sqlx::query(&format!("{TAG_SELECT}\n ORDER BY g.position ASC, g.id ASC"))
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows.iter().map(to_tag).collect())
}

/// A tag by name, ignoring case.
pub async fn find_tag(pool: &SqlitePool, name: &str) -> Result<Option<TaskTag>, String> {
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    find_tag_on(&mut conn, name).await
}

async fn find_tag_on(conn: &mut SqliteConnection, name: &str) -> Result<Option<TaskTag>, String> {
    let row = sqlx::query(&format!("{TAG_SELECT}\n WHERE g.name = ?1 COLLATE NOCASE"))
        .bind(name.split_whitespace().collect::<Vec<_>>().join(" "))
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(row.as_ref().map(to_tag))
}

async fn tag_on(conn: &mut SqliteConnection, id: i64) -> Result<TaskTag, String> {
    let row = sqlx::query(&format!("{TAG_SELECT}\n WHERE g.id = ?1"))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    row.as_ref()
        .map(to_tag)
        .ok_or_else(|| format!("tag {id} does not exist"))
}

/// The tag with this name, made at the end of the registry if none matches
/// ignoring case. An existing tag is returned as it is: `color` only applies to
/// a new one, so a repeated `tag add` never recolours.
pub(super) async fn ensure_tag_on(
    conn: &mut SqliteConnection,
    name: &str,
    color: Option<&str>,
) -> Result<TaskTag, String> {
    let name = normalise_tag_name(name)?;
    let color = match color {
        Some(color) => check_tag_color(color)?,
        None => DEFAULT_TAG_COLOR.to_string(),
    };
    if let Some(existing) = find_tag_on(conn, &name).await? {
        return Ok(existing);
    }
    let next: f64 =
        sqlx::query_scalar("SELECT CAST(COALESCE(MAX(position), 0) + 1 AS REAL) FROM task_tags")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    let id = sqlx::query("INSERT INTO task_tags (name, color, position) VALUES (?1, ?2, ?3)")
        .bind(&name)
        .bind(&color)
        .bind(next)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?
        .last_insert_rowid();
    tag_on(conn, id).await
}

/// Make a tag, or return the existing one that matches ignoring case.
pub async fn create_tag(
    pool: &SqlitePool,
    name: &str,
    color: Option<&str>,
) -> Result<TaskTag, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let tag = ensure_tag_on(&mut tx, name, color).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(tag)
}

/// The ids for a list of names, making any that do not exist yet, in order and
/// without repeats (ignoring case).
pub(super) async fn ensure_tags_on(
    conn: &mut SqliteConnection,
    names: &[String],
) -> Result<Vec<i64>, String> {
    let mut ids: Vec<i64> = Vec::with_capacity(names.len());
    for name in names {
        if name.trim().is_empty() {
            continue;
        }
        let id = ensure_tag_on(conn, name, None).await?.id;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// Rename and/or recolour. A rename that matches another tag ignoring case is
/// refused; changing only the capitalisation of its own name is fine.
pub async fn update_tag(
    pool: &SqlitePool,
    id: i64,
    name: Option<&str>,
    color: Option<&str>,
) -> Result<TaskTag, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    tag_on(&mut tx, id).await?;
    if let Some(name) = name {
        let name = normalise_tag_name(name)?;
        if let Some(other) = find_tag_on(&mut tx, &name).await? {
            if other.id != id {
                return Err(format!(
                    "a tag called \"{}\" already exists (tag {})",
                    other.name, other.id
                ));
            }
        }
        sqlx::query("UPDATE task_tags SET name = ?1 WHERE id = ?2")
            .bind(&name)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(color) = color {
        let color = check_tag_color(color)?;
        sqlx::query("UPDATE task_tags SET color = ?1 WHERE id = ?2")
            .bind(&color)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    let tag = tag_on(&mut tx, id).await?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(tag)
}

/// Delete a tag everywhere; returns how many tasks carried it. Those tasks'
/// `updated_at` moves (and their projects'), as for any other link change.
pub async fn delete_tag(pool: &SqlitePool, id: i64) -> Result<i64, String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let tag = tag_on(&mut tx, id).await?;
    sqlx::query(
        "UPDATE projects SET updated_at = datetime('now') WHERE id IN
           (SELECT t.project_id FROM project_tasks t
              JOIN task_tag_links l ON l.task_id = t.id WHERE l.tag_id = ?1)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE project_tasks SET updated_at = datetime('now') WHERE id IN
           (SELECT task_id FROM task_tag_links WHERE tag_id = ?1)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    // The foreign key would cascade these; deleting them first keeps the
    // delete correct on a connection that has foreign keys off.
    sqlx::query("DELETE FROM task_tag_links WHERE tag_id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM task_tags WHERE id = ?1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(tag.uses)
}

/// Replace a task's tag set with `tag_ids`, atomically. An id the registry
/// lacks refuses the whole call and names it.
pub async fn set_task_tags(pool: &SqlitePool, task_id: i64, tag_ids: &[i64]) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let existing = task_on(&mut tx, task_id)
        .await?
        .ok_or_else(|| format!("task {task_id} does not exist"))?;
    replace_task_tags_on(&mut tx, task_id, tag_ids).await?;
    touch_task(&mut tx, task_id).await?;
    touch_project(&mut tx, existing.project_id).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

/// The link rewrite alone, for a caller that owns the transaction and the
/// `updated_at` bumps.
pub(super) async fn replace_task_tags_on(
    conn: &mut SqliteConnection,
    task_id: i64,
    tag_ids: &[i64],
) -> Result<(), String> {
    let mut unknown: Vec<i64> = Vec::new();
    for id in tag_ids {
        let known: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM task_tags WHERE id = ?1")
            .bind(id)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        if known == 0 && !unknown.contains(id) {
            unknown.push(*id);
        }
    }
    if !unknown.is_empty() {
        let ids: Vec<String> = unknown.iter().map(i64::to_string).collect();
        return Err(format!("no such tag id: {}", ids.join(", ")));
    }
    sqlx::query("DELETE FROM task_tag_links WHERE task_id = ?1")
        .bind(task_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    for id in tag_ids {
        sqlx::query("INSERT OR IGNORE INTO task_tag_links (task_id, tag_id) VALUES (?1, ?2)")
            .bind(task_id)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
