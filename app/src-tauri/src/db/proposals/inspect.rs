use serde_json::{json, Map, Value};
use sqlx::{Column, Row, SqliteConnection, SqlitePool, TypeInfo, ValueRef};

pub(super) fn table(entity: &str) -> Result<&'static str, String> {
    Ok(match entity {
        "task" => "project_tasks",
        "project" => "projects",
        "tag" => "task_tags",
        "event" => "local_events",
        _ => return Err("unknown proposal entity".into()),
    })
}
pub(super) fn numeric(entity: &str, id: &str) -> Result<i64, String> {
    let id = if entity == "event" {
        id.strip_prefix("local_")
            .ok_or("only your local_ events may be changed")?
    } else {
        id
    };
    id.parse::<i64>()
        .ok()
        .filter(|i| *i > 0)
        .ok_or_else(|| "invalid entity id".into())
}
pub(super) fn raw(row: &sqlx::sqlite::SqliteRow) -> Result<Value, String> {
    let mut out = Map::new();
    for c in row.columns() {
        let value = row.try_get_raw(c.ordinal()).map_err(|e| e.to_string())?;
        let v = if value.is_null() {
            Value::Null
        } else {
            match value.type_info().name() {
                "INTEGER" => json!(row
                    .try_get::<i64, _>(c.ordinal())
                    .map_err(|e| e.to_string())?),
                "REAL" => json!(row
                    .try_get::<f64, _>(c.ordinal())
                    .map_err(|e| e.to_string())?),
                _ => json!(row
                    .try_get::<String, _>(c.ordinal())
                    .map_err(|e| e.to_string())?),
            }
        };
        out.insert(c.name().into(), v);
    }
    Ok(Value::Object(out))
}
pub(super) async fn raw_rows(
    conn: &mut SqliteConnection,
    sql: &str,
    id: i64,
) -> Result<Value, String> {
    let rows = sqlx::query(sql)
        .bind(id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    rows.iter()
        .map(raw)
        .collect::<Result<Vec<_>, _>>()
        .map(Value::Array)
}
pub(super) async fn snapshot(
    conn: &mut SqliteConnection,
    entity: &str,
    id: &str,
) -> Result<Value, String> {
    let n = numeric(entity, id)?;
    let row = sqlx::query(&format!("SELECT * FROM {} WHERE id=?1", table(entity)?))
        .bind(n)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let Some(row) = row else {
        return Ok(Value::Null);
    };
    let mut result = json!({"row":raw(&row)?});
    if entity == "task" {
        result["tags"] = raw_rows(
            conn,
            "SELECT task_id,tag_id FROM task_tag_links WHERE task_id=?1 ORDER BY tag_id",
            n,
        )
        .await?;
        result["files"] = raw_rows(
            conn,
            "SELECT * FROM task_files WHERE task_id=?1 ORDER BY file_id",
            n,
        )
        .await?;
        result["children"] = raw_rows(
            conn,
            "SELECT id FROM project_tasks WHERE parent_id=?1 ORDER BY id",
            n,
        )
        .await?;
    } else if entity == "project" {
        result["children"] = raw_rows(
            conn,
            "SELECT id FROM project_tasks WHERE project_id=?1 ORDER BY id",
            n,
        )
        .await?;
    } else if entity == "tag" {
        result["links"] = raw_rows(
            conn,
            "SELECT task_id FROM task_tag_links WHERE tag_id=?1 ORDER BY task_id",
            n,
        )
        .await?;
        result["projects"]=raw_rows(conn,"SELECT p.id FROM projects p WHERE EXISTS (SELECT 1 FROM json_each(p.tags) j JOIN task_tags g ON j.value=g.name COLLATE NOCASE WHERE g.id=?1) ORDER BY p.id",n).await?;
    }
    Ok(result)
}

pub async fn subject(
    conn: &mut SqliteConnection,
    code: Option<&str>,
) -> Result<Option<(i64, String)>, String> {
    let Some(code) = code else { return Ok(None) };
    let folded = crate::library::paths::fold_code(code);
    let rows = sqlx::query("SELECT id,code,is_current FROM subjects ORDER BY id")
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    let matched: Vec<_> = rows
        .iter()
        .filter(|r| {
            crate::library::paths::fold_code(&r.get::<String, _>("code")).starts_with(&folded)
        })
        .collect();
    let current: Vec<_> = matched
        .iter()
        .filter(|r| r.get::<i64, _>("is_current") != 0)
        .collect();
    let row = if matched.len() == 1 {
        matched[0]
    } else if current.len() == 1 {
        current[0]
    } else {
        return Err(format!(
            "{code} does not name one subject — pass its full code"
        ));
    };
    Ok(Some((row.get("id"), row.get("code"))))
}

pub async fn inspect(
    pool: &SqlitePool,
    entity: &str,
    ids: &[Value],
) -> Result<Map<String, Value>, String> {
    if ids.len() > 500 {
        return Err("too many ids".into());
    }
    let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
    let mut out = Map::new();
    for id in ids {
        let key = match id {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return Err("invalid entity id".into()),
        };
        out.insert(key.clone(), current(&mut conn, entity, &key).await?);
    }
    Ok(out)
}

pub(super) async fn current(
    conn: &mut SqliteConnection,
    entity: &str,
    id: &str,
) -> Result<Value, String> {
    let snap = snapshot(conn, entity, id).await?;
    if snap.is_null() {
        return Ok(Value::Null);
    }
    let mut row = snap["row"].clone();
    let n = numeric(entity, id)?;
    if entity == "task" {
        for (name, column) in [
            ("due", "due_at"),
            ("starts", "starts_at"),
            ("estimate", "estimate_minutes"),
            ("column", "column_id"),
            ("project", "project_id"),
            ("parent", "parent_id"),
        ] {
            row[name] = row[column].clone();
        }
        let names:Vec<String>=sqlx::query_scalar("SELECT g.name FROM task_tags g WHERE g.id IN (SELECT tag_id FROM task_tag_links WHERE task_id=?1) OR (NOT EXISTS (SELECT 1 FROM task_tag_links WHERE task_id=?1) AND g.name COLLATE NOCASE IN (SELECT j.value FROM projects p,json_each(p.tags) j JOIN project_tasks t ON t.project_id=p.id WHERE t.id=?1)) ORDER BY g.position,g.id").bind(n).fetch_all(&mut *conn).await.map_err(|e|e.to_string())?;
        row["tags"] = json!(names);
        let files:Vec<String>=sqlx::query_scalar("SELECT f.relative_path FROM files f JOIN task_files tf ON tf.file_id=f.id WHERE tf.task_id=?1 ORDER BY f.relative_path").bind(n).fetch_all(&mut *conn).await.map_err(|e|e.to_string())?;
        let columns:Option<String>=sqlx::query_scalar("SELECT columns FROM projects WHERE id=(SELECT project_id FROM project_tasks WHERE id=?1)").bind(n).fetch_optional(&mut *conn).await.map_err(|e|e.to_string())?;
        row["columns"] = columns
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or_else(|| json!(crate::db::projects::default_columns()));
        row["files"] = json!(files);
        row["children"] = raw_rows(
            conn,
            "SELECT id,title FROM project_tasks WHERE parent_id=?1 ORDER BY position,id",
            n,
        )
        .await?;
    } else if entity == "project" {
        row["due"] = row["due_at"].clone();
        row["starts"] = row["starts_at"].clone();
        row["event"] = row["event_id"].clone();
        row["tags"] = serde_json::from_str(row["tags"].as_str().unwrap_or("[]"))
            .map_err(|e| e.to_string())?;
        row["columns"] = serde_json::from_str(row["columns"].as_str().unwrap_or("[]"))
            .map_err(|e| e.to_string())?;
    } else if entity == "event" {
        row["id"] = json!(id);
        row["start"] = row["start_at"].clone();
        row["end"] = row["end_at"].clone();
    }
    if entity == "tag" {
        let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM project_tasks t WHERE EXISTS (SELECT 1 FROM task_tag_links l WHERE l.task_id=t.id AND l.tag_id=?1) OR (NOT EXISTS (SELECT 1 FROM task_tag_links l WHERE l.task_id=t.id) AND EXISTS (SELECT 1 FROM projects p,json_each(p.tags) j JOIN task_tags g ON g.name=j.value COLLATE NOCASE WHERE p.id=t.project_id AND g.id=?1))").bind(n).fetch_one(&mut *conn).await.map_err(|e|e.to_string())?;
        row["task_count"] = json!(count);
    }
    if entity == "project" || entity == "event" {
        row["subject"] = if let Some(s) = row["subject_id"].as_i64() {
            let code: Option<String> = sqlx::query_scalar("SELECT code FROM subjects WHERE id=?1")
                .bind(s)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            json!(code)
        } else {
            Value::Null
        };
    }
    Ok(row)
}
