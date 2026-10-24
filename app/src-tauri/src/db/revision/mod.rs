//! Revision rows shared by the headless CLI and the frontend's SQLite writers.
//! JSON columns are decoded at this boundary; inputs reject unknown fields.

use serde_json::{Map, Value};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection, SqlitePool};

pub mod tags;
#[cfg(test)]
mod tests;
pub mod topics;

pub const SCHEMA: &str = include_str!("schema.sql");

#[derive(Clone, Copy, Debug)]
pub enum Entity {
    Note,
    Item,
    Set,
    Artifact,
}

impl Entity {
    fn table(self) -> &'static str {
        match self {
            Self::Note => "revision_notes",
            Self::Item => "revision_items",
            Self::Set => "revision_sets",
            Self::Artifact => "revision_artifacts",
        }
    }

    pub(crate) fn memberships(self) -> (&'static str, &'static str) {
        match self {
            Self::Note => ("revision_note_topics", "note_id"),
            Self::Item => ("revision_item_topics", "item_id"),
            Self::Set => ("revision_set_topics", "set_id"),
            Self::Artifact => ("revision_artifact_topics", "artifact_id"),
        }
    }

    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Note => &[
                "subject_id",
                "title",
                "body",
                "tags",
                "position",
                "source_kind",
                "source_ref",
            ],
            Self::Item => &[
                "subject_id",
                "kind",
                "prompt",
                "answer",
                "options",
                "tolerance",
                "rubric",
                "level",
                "topic",
                "style_ref",
                "status",
                "created_by",
                "due",
                "stability",
                "fsrs_difficulty",
                "reps",
                "lapses",
                "state",
                "learning_steps",
                "last_review",
            ],
            Self::Set => &["subject_id", "title", "description"],
            Self::Artifact => &["subject_id", "title", "program", "state", "source_ref"],
        }
    }

    fn json_fields(self) -> &'static [&'static str] {
        match self {
            Self::Note => &["tags"],
            Self::Item => &["options", "rubric", "topic"],
            Self::Set => &[],
            Self::Artifact => &["state"],
        }
    }

    fn projection(self) -> String {
        let fields = [
            vec!["id"],
            self.fields().to_vec(),
            vec!["created_at", "updated_at"],
        ]
        .concat();
        let pairs = fields
            .iter()
            .map(|name| {
                let expression = if self.json_fields().contains(name) {
                    format!("json({name})")
                } else {
                    name.to_string()
                };
                format!("'{name}',{expression}")
            })
            .collect::<Vec<_>>()
            .join(",");
        let (membership, key) = self.memberships();
        let table = self.table();
        format!("json_object({pairs},'topic_ids',json((SELECT json_group_array(topic_id) FROM (SELECT topic_id FROM {membership} WHERE {key}={table}.id ORDER BY topic_id))))")
    }
}

fn input(entity: Entity, value: &Value, creating: bool) -> Result<&Map<String, Value>, String> {
    let object = value.as_object().ok_or("expected a JSON object")?;
    for (key, value) in object {
        if key == "topic_ids" {
            if !value
                .as_array()
                .is_some_and(|a| a.iter().all(|v| v.as_i64().is_some_and(|id| id > 0)))
            {
                return Err("topic_ids must contain positive integer ids".into());
            }
            continue;
        }
        if !entity.fields().contains(&key.as_str()) {
            return Err(format!("unknown revision field: {key}"));
        }
        if entity.json_fields().contains(&key.as_str()) {
            if key != "state" && !value.is_array() {
                return Err(format!("{key} must be a JSON array"));
            }
            if matches!(key.as_str(), "tags" | "topic" | "options")
                && !value
                    .as_array()
                    .is_some_and(|a| a.iter().all(Value::is_string))
            {
                return Err(format!("{key} must contain strings"));
            }
            if key == "tags" {
                let names: Vec<&str> = value
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                tags::check(&names)?;
            }
            if key == "rubric"
                && !value.as_array().is_some_and(|points| {
                    points.iter().all(|p| {
                        p.get("point").is_some_and(Value::is_string)
                            && p.get("marks")
                                .and_then(Value::as_f64)
                                .is_some_and(|m| m > 0.0)
                    })
                })
            {
                return Err("rubric points need point text and positive marks".into());
            }
        } else if value.is_array() || value.is_object() || value.is_boolean() {
            return Err(format!("{key} must be text, a number, or null"));
        }
    }
    for key in [
        "title",
        "body",
        "prompt",
        "answer",
        "style_ref",
        "source_ref",
        "description",
        "program",
    ] {
        if let Some(v) = object.get(key).filter(|v| !v.is_null()) {
            if !v.is_string() {
                return Err(format!("{key} must be text"));
            }
        }
    }
    if matches!(entity, Entity::Artifact) && object.get("state").is_some_and(|v| !v.is_object()) {
        return Err("Artifact state must be a JSON object".into());
    }
    // Revision is scoped to one subject; no writer leaves a row without one.
    match object.get("subject_id") {
        Some(v) if v.as_i64().is_some_and(|id| id > 0) => {}
        None if !creating => {}
        _ => return Err("subject_id must name a subject".into()),
    }
    let required: &[&str] = match entity {
        Entity::Note | Entity::Set => &["title"],
        Entity::Item => &["kind", "prompt", "answer"],
        Entity::Artifact => &["title", "program"],
    };
    for key in required {
        if (creating || object.contains_key(*key))
            && !object
                .get(*key)
                .and_then(Value::as_str)
                .is_some_and(|v| !v.trim().is_empty())
        {
            return Err(format!("{key} must not be empty"));
        }
    }
    for (field, allowed) in [
        (
            "kind",
            &["card", "choice", "numeric", "short", "structured"][..],
        ),
        ("level", &["easy", "medium", "hard"][..]),
        ("status", &["draft", "active", "suspended"][..]),
        ("created_by", &["agent", "user"][..]),
        (
            "source_kind",
            &["chat", "file", "lecture", "agent", "manual"][..],
        ),
    ] {
        if let Some(value) = object.get(field) {
            if !value.as_str().is_some_and(|v| allowed.contains(&v)) {
                return Err(format!("{field} must be {}", allowed.join(", ")));
            }
        }
    }
    for key in ["subject_id", "reps", "lapses", "state", "learning_steps"] {
        if entity.json_fields().contains(&key) {
            continue;
        }
        if let Some(value) = object.get(key).filter(|v| !v.is_null()) {
            if !value.as_i64().is_some_and(|v| v >= 0) {
                return Err(format!("{key} must be a nonnegative integer"));
            }
        }
    }
    for key in ["tolerance", "stability", "fsrs_difficulty", "position"] {
        if let Some(value) = object.get(key).filter(|v| !v.is_null()) {
            if !value
                .as_f64()
                .is_some_and(|v| v.is_finite() && (key == "position" || v >= 0.0))
            {
                return Err(format!("{key} must be a finite nonnegative number"));
            }
        }
    }
    for key in ["due", "last_review"] {
        if let Some(value) = object.get(key).filter(|v| !v.is_null()) {
            crate::db::projects::check_iso8601(
                value.as_str().ok_or("dates must be ISO 8601 text")?,
            )?;
        }
    }
    Ok(object)
}

fn bind(builder: &mut QueryBuilder<'_, Sqlite>, entity: Entity, key: &str, value: &Value) {
    if entity.json_fields().contains(&key) {
        builder.push_bind(value.to_string());
    } else if let Some(v) = value.as_i64() {
        builder.push_bind(v);
    } else if let Some(v) = value.as_f64() {
        builder.push_bind(v);
    } else {
        builder.push_bind(value.as_str().map(str::to_owned));
    }
}

pub async fn add(pool: &SqlitePool, entity: Entity, value: &Value) -> Result<i64, String> {
    let ids = add_batch(pool, entity, std::slice::from_ref(value)).await?;
    Ok(ids[0])
}

/// One transaction: a malformed item or failed relation leaves no partial batch.
pub async fn add_batch(
    pool: &SqlitePool,
    entity: Entity,
    values: &[Value],
) -> Result<Vec<i64>, String> {
    for value in values {
        input(entity, value, true)?;
        if matches!(entity, Entity::Item) {
            validate_question(value)?;
        }
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    for value in values {
        ids.push(insert_on(&mut tx, entity, value).await?);
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(ids)
}

pub async fn update(
    pool: &SqlitePool,
    entity: Entity,
    id: i64,
    value: &Value,
) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    update_on(&mut tx, entity, id, value).await?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn update_on(
    conn: &mut SqliteConnection,
    entity: Entity,
    id: i64,
    value: &Value,
) -> Result<(), String> {
    let object = input(entity, value, false)?;
    let mut next = show_on(conn, entity, id).await?;
    for (k, v) in object {
        next[k] = v.clone();
    }
    if matches!(entity, Entity::Item) {
        validate_question(&next)?;
    }
    if object.is_empty() {
        return Err("no changes supplied".into());
    }
    let mut query = QueryBuilder::<Sqlite>::new(format!("UPDATE {} SET ", entity.table()));
    let mut first = true;
    for (key, value) in object.iter().filter(|(k, _)| k.as_str() != "topic_ids") {
        if !first {
            query.push(",");
        }
        first = false;
        query.push(format!("{key}="));
        bind(&mut query, entity, key, value);
    }
    if !first {
        query.push(",");
    }
    query
        .push("updated_at=datetime('now') WHERE id=")
        .push_bind(id);
    query
        .build()
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(ids) = object.get("topic_ids") {
        topics::replace_on(
            conn,
            entity,
            id,
            &ids.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    Ok(())
}

pub async fn list(pool: &SqlitePool, entity: Entity) -> Result<Vec<Value>, String> {
    let order = if matches!(entity, Entity::Note) {
        "position,id"
    } else {
        "id"
    };
    let query = format!(
        "SELECT {} FROM {} ORDER BY {order}",
        entity.projection(),
        entity.table()
    );
    sqlx::query_scalar::<_, String>(&query)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
        .collect()
}

pub async fn show(pool: &SqlitePool, entity: Entity, id: i64) -> Result<Value, String> {
    show_on(
        &mut *pool.acquire().await.map_err(|e| e.to_string())?,
        entity,
        id,
    )
    .await
}
pub async fn show_on(
    conn: &mut SqliteConnection,
    entity: Entity,
    id: i64,
) -> Result<Value, String> {
    let query = format!(
        "SELECT {} FROM {} WHERE id=?",
        entity.projection(),
        entity.table()
    );
    let row = sqlx::query_scalar::<_, String>(&query)
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no revision row {id}"))?;
    serde_json::from_str(&row).map_err(|e| e.to_string())
}

pub async fn remove(pool: &SqlitePool, entity: Entity, id: i64) -> Result<(), String> {
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // Explicit dependent deletes also work when the caller's connection lacks foreign_keys.
    if matches!(entity, Entity::Item) {
        sqlx::query("DELETE FROM revision_reviews WHERE item_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("DELETE FROM revision_set_items WHERE item_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    if matches!(entity, Entity::Set) {
        sqlx::query("DELETE FROM revision_set_items WHERE set_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        sqlx::query("UPDATE revision_reviews SET set_id=NULL WHERE set_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    let (membership, key) = entity.memberships();
    // Delete the parent first so fallback triggers do not recreate its memberships.
    let result = sqlx::query(&format!("DELETE FROM {} WHERE id=?", entity.table()))
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    if result.rows_affected() == 0 {
        return Err(format!("no revision row {id}"));
    }
    sqlx::query(&format!("DELETE FROM {membership} WHERE {key}=?"))
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

pub async fn set_items(pool: &SqlitePool, set_id: i64) -> Result<Vec<i64>, String> {
    show(pool, Entity::Set, set_id).await?;
    sqlx::query_scalar(
        "SELECT item_id FROM revision_set_items WHERE set_id=? ORDER BY position,item_id",
    )
    .bind(set_id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())
}

pub async fn add_set(pool: &SqlitePool, value: &Value, item_ids: &[i64]) -> Result<i64, String> {
    input(Entity::Set, value, true)?;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for item in item_ids {
        let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM revision_items WHERE id=?1")
            .bind(item)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        if exists != 1 {
            return Err(format!("No question {item}"));
        }
    }
    let id = insert_on(&mut tx, Entity::Set, value).await?;
    for (position, item_id) in item_ids.iter().enumerate() {
        sqlx::query(
            "INSERT OR IGNORE INTO revision_set_items (set_id,item_id,position) VALUES (?,?,?)",
        )
        .bind(id)
        .bind(item_id)
        .bind(position as f64)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(id)
}

fn validate_question(value: &Value) -> Result<(), String> {
    if value["kind"] == "numeric"
        && !value["answer"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .is_some_and(f64::is_finite)
    {
        return Err("Numeric answers must be finite numbers written as text".into());
    }
    if value["kind"] == "choice" {
        let options = value["options"]
            .as_array()
            .ok_or("Choice questions need options")?;
        if options.len() < 2
            || options
                .iter()
                .any(|v| v.as_str().is_none_or(|s| s.trim().is_empty()))
            || options.iter().filter(|v| **v == value["answer"]).count() != 1
        {
            return Err(
                "Choice questions need at least two options and exactly one matching answer".into(),
            );
        }
    }
    Ok(())
}

async fn insert_on(
    conn: &mut SqliteConnection,
    entity: Entity,
    value: &Value,
) -> Result<i64, String> {
    let mut object = value.as_object().unwrap().clone();
    if matches!(entity, Entity::Note) && !object.contains_key("position") {
        // A new entry goes last. The cast keeps an empty table's 0 a REAL.
        let next: f64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(MAX(position),-1)+1 AS REAL) FROM revision_notes",
        )
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
        object.insert("position".into(), serde_json::json!(next));
    }
    let mut query = QueryBuilder::<Sqlite>::new(format!("INSERT INTO {} (", entity.table()));
    query.push(
        object
            .keys()
            .filter(|k| k.as_str() != "topic_ids")
            .cloned()
            .collect::<Vec<_>>()
            .join(","),
    );
    query.push(") VALUES (");
    for (index, (key, value)) in object
        .iter()
        .filter(|(k, _)| k.as_str() != "topic_ids")
        .enumerate()
    {
        if index > 0 {
            query.push(",");
        }
        bind(&mut query, entity, key, value);
    }
    query.push(")");
    let result = query
        .build()
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let id = result.last_insert_rowid();
    if let Some(ids) = object.get("topic_ids") {
        topics::replace_on(
            conn,
            entity,
            id,
            &ids.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect::<Vec<_>>(),
        )
        .await?;
    }
    Ok(id)
}
