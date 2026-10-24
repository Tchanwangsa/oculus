//! Subject-scoped buckets shared by every Revision content type.
use serde_json::{json, Value};
use sqlx::{SqliteConnection, SqlitePool};

const PROJECTION: &str = "json_object('id',id,'subject_id',subject_id,'name',name,'description',description,'position',position,'is_unsorted',is_unsorted)";

pub async fn list(pool: &SqlitePool) -> Result<Vec<Value>, String> {
    let rows: Vec<String> = sqlx::query_scalar(&format!("SELECT {PROJECTION} FROM revision_topics ORDER BY subject_id,is_unsorted DESC,position,name,id"))
        .fetch_all(pool).await.map_err(|e|e.to_string())?;
    rows.into_iter()
        .map(|v| serde_json::from_str(&v).map_err(|e| e.to_string()))
        .collect()
}
pub async fn show(pool: &SqlitePool, id: i64) -> Result<Value, String> {
    show_on(&mut *pool.acquire().await.map_err(|e| e.to_string())?, id).await
}
pub async fn show_on(conn: &mut SqliteConnection, id: i64) -> Result<Value, String> {
    let row: String = sqlx::query_scalar(&format!(
        "SELECT {PROJECTION} FROM revision_topics WHERE id=?"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("No topic {id}"))?;
    serde_json::from_str(&row).map_err(|e| e.to_string())
}
fn validate(value: &Value, creating: bool) -> Result<(), String> {
    let obj = value.as_object().ok_or("Expected a topic object")?;
    for (key, val) in obj {
        match key.as_str() {
            "name" if val.as_str().is_some_and(|s| !s.trim().is_empty()) => {}
            "description" if val.is_string() => {}
            "position" if val.as_f64().is_some_and(f64::is_finite) => {}
            "subject_id" if creating && val.as_i64().is_some_and(|v| v > 0) => {}
            _ => return Err(format!("Invalid topic field: {key}")),
        }
    }
    if creating && !obj.contains_key("name") {
        return Err("Topic name is required".into());
    }
    if creating && !obj.contains_key("subject_id") {
        return Err("A topic belongs to a subject; subject_id is required".into());
    }
    if !creating && obj.is_empty() {
        return Err("No changes supplied".into());
    }
    Ok(())
}
pub async fn add(pool: &SqlitePool, value: &Value) -> Result<i64, String> {
    validate(value, true)?;
    let name = value["name"].as_str().unwrap().trim();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    sqlx::query("INSERT OR IGNORE INTO revision_topics(subject_id,name,description,position) VALUES(?,?,?,?)")
        .bind(value["subject_id"].as_i64()).bind(name).bind(value["description"].as_str().unwrap_or(""))
        .bind(value["position"].as_f64().unwrap_or(0.0)).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    let id = sqlx::query_scalar(
        "SELECT id FROM revision_topics WHERE subject_id IS ? AND name=? COLLATE NOCASE",
    )
    .bind(value["subject_id"].as_i64())
    .bind(name)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(id)
}
pub async fn update(pool: &SqlitePool, id: i64, value: &Value) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    update_on(&mut tx, id, value).await?;
    tx.commit().await.map_err(|e| e.to_string())
}
pub async fn update_on(conn: &mut SqliteConnection, id: i64, value: &Value) -> Result<(), String> {
    validate(value, false)?;
    let mut next = show_on(conn, id).await?;
    if next["is_unsorted"] == 1
        && value
            .get("name")
            .is_some_and(|v| v.as_str() != Some("Unsorted"))
    {
        return Err("Unsorted cannot be renamed".into());
    }
    for (key, val) in value.as_object().unwrap() {
        next[key] = val.clone();
    }
    sqlx::query("UPDATE revision_topics SET name=?,description=?,position=? WHERE id=?")
        .bind(next["name"].as_str().unwrap().trim())
        .bind(next["description"].as_str().unwrap())
        .bind(next["position"].as_f64().unwrap())
        .bind(id)
        .execute(conn)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub async fn remove(pool: &SqlitePool, id: i64) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let row = show_on(&mut tx, id).await?;
    if row["is_unsorted"] == json!(1) {
        return Err("Unsorted cannot be deleted".into());
    }
    sqlx::query("DELETE FROM revision_topics WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    for entity in [
        super::Entity::Note,
        super::Entity::Item,
        super::Entity::Set,
        super::Entity::Artifact,
    ] {
        let (table, _) = entity.memberships();
        sqlx::query(&format!("DELETE FROM {table} WHERE topic_id=?"))
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn ids_on(
    conn: &mut SqliteConnection,
    entity: super::Entity,
    id: i64,
) -> Result<Vec<i64>, String> {
    let (table, key) = entity.memberships();
    sqlx::query_scalar(&format!(
        "SELECT topic_id FROM {table} WHERE {key}=? ORDER BY topic_id"
    ))
    .bind(id)
    .fetch_all(conn)
    .await
    .map_err(|e| e.to_string())
}
pub async fn replace_on(
    conn: &mut SqliteConnection,
    entity: super::Entity,
    id: i64,
    ids: &[i64],
) -> Result<(), String> {
    let subject: Option<i64> = sqlx::query_scalar(&format!(
        "SELECT subject_id FROM {} WHERE id=?",
        entity.table()
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("No revision row {id}"))?;
    let mut named = Vec::new();
    for &topic in ids {
        if topic <= 0 {
            return Err("topic_ids must contain positive integers".into());
        }
        let valid: Option<i64> = sqlx::query_scalar(
            "SELECT is_unsorted FROM revision_topics WHERE id=? AND subject_id IS ?",
        )
        .bind(topic)
        .bind(subject)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
        match valid {
            None => return Err(format!("Topic {topic} must belong to the same subject")),
            Some(0) if !named.contains(&topic) => {
                named.push(topic);
            }
            _ => {}
        }
    }
    let (table, key) = entity.memberships();
    sqlx::query(&format!("DELETE FROM {table} WHERE {key}=?"))
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(&format!("INSERT OR IGNORE INTO {table}({key},topic_id) SELECT ?,id FROM revision_topics WHERE subject_id IS ? AND is_unsorted=1"))
        .bind(id).bind(subject).execute(&mut *conn).await.map_err(|e|e.to_string())?;
    for topic in named {
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO {table}({key},topic_id) VALUES(?,?)"
        ))
        .bind(id)
        .bind(topic)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
