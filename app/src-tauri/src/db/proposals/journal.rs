use super::inspect::{raw_rows, snapshot};
use serde_json::{json, Value};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Journal {
    before: BTreeMap<(String, String), Value>,
}
impl Journal {
    pub async fn watch(
        &mut self,
        conn: &mut SqliteConnection,
        entity: &str,
        id: &str,
    ) -> Result<(), String> {
        let key = (entity.to_owned(), id.to_owned());
        if let std::collections::btree_map::Entry::Vacant(entry) = self.before.entry(key) {
            entry.insert(snapshot(conn, entity, id).await?);
        }
        Ok(())
    }
    pub fn created(&mut self, entity: &str, id: impl ToString) {
        self.before
            .entry((entity.into(), id.to_string()))
            .or_insert(Value::Null);
    }
    pub async fn task(
        &mut self,
        conn: &mut SqliteConnection,
        id: i64,
        children: bool,
    ) -> Result<(), String> {
        self.watch(conn, "task", &id.to_string()).await?;
        let snap = snapshot(conn, "task", &id.to_string()).await?;
        if let Some(project) = snap.pointer("/row/project_id").and_then(Value::as_i64) {
            self.watch(conn, "project", &project.to_string()).await?;
        }
        if let Some(parent) = snap.pointer("/row/parent_id").and_then(Value::as_i64) {
            self.watch(conn, "task", &parent.to_string()).await?;
        }
        if children {
            for c in snap["children"].as_array().into_iter().flatten() {
                let child = c["id"].as_i64().ok_or("invalid child id")?;
                self.watch(conn, "task", &child.to_string()).await?;
            }
        }
        Ok(())
    }
    pub async fn tag_dependents(
        &mut self,
        conn: &mut SqliteConnection,
        id: i64,
    ) -> Result<(), String> {
        self.watch(conn, "tag", &id.to_string()).await?;
        let snap = snapshot(conn, "tag", &id.to_string()).await?;
        for t in snap["links"].as_array().into_iter().flatten() {
            self.task(conn, t["task_id"].as_i64().ok_or("invalid task id")?, false)
                .await?;
        }
        for p in snap["projects"].as_array().into_iter().flatten() {
            self.watch(conn, "project", &p["id"].to_string()).await?;
        }
        Ok(())
    }
    pub async fn tag_names(
        &mut self,
        conn: &mut SqliteConnection,
        names: &[String],
    ) -> Result<Vec<(String, Option<i64>)>, String> {
        let mut result = vec![];
        for name in names {
            let normalized = crate::db::projects::normalise_tag_name(name)?;
            let id: Option<i64> =
                sqlx::query_scalar("SELECT id FROM task_tags WHERE name=?1 COLLATE NOCASE")
                    .bind(&normalized)
                    .fetch_optional(&mut *conn)
                    .await
                    .map_err(|e| e.to_string())?;
            if let Some(id) = id {
                self.watch(conn, "tag", &id.to_string()).await?;
            }
            result.push((normalized, id));
        }
        Ok(result)
    }
    pub async fn added_tags(
        &mut self,
        conn: &mut SqliteConnection,
        names: &[(String, Option<i64>)],
    ) -> Result<(), String> {
        for (name, old) in names {
            if old.is_none() {
                let id: Option<i64> =
                    sqlx::query_scalar("SELECT id FROM task_tags WHERE name=?1 COLLATE NOCASE")
                        .bind(name)
                        .fetch_optional(&mut *conn)
                        .await
                        .map_err(|e| e.to_string())?;
                if let Some(id) = id {
                    self.created("tag", id);
                }
            }
        }
        Ok(())
    }
    pub async fn finish(
        self,
        conn: &mut SqliteConnection,
        thread: i64,
        proposal: &str,
    ) -> Result<usize, String> {
        let mut count = 0;
        for ((entity, id), before) in self.before {
            let after = snapshot(conn, &entity, &id).await?;
            if before == after {
                continue;
            }
            let op = if before.is_null() {
                "create"
            } else if after.is_null() {
                "delete"
            } else if before.pointer("/row/project_id") != after.pointer("/row/project_id") {
                "refile"
            } else if before.pointer("/row/column_id") != after.pointer("/row/column_id") {
                "move"
            } else {
                "update"
            };
            record(conn, thread, proposal, (&entity, &id, op), &before, &after).await?;
            count += 1;
        }
        Ok(count)
    }
}
pub(super) async fn record(
    conn: &mut SqliteConnection,
    thread: i64,
    proposal: &str,
    target: (&str, &str, &str),
    before: &Value,
    after: &Value,
) -> Result<(), String> {
    let (entity, id, op) = target;
    sqlx::query("INSERT INTO agent_changes (thread_id,proposal_id,entity,entity_id,op,before,after) VALUES (?1,?2,?3,?4,?5,?6,?7)").bind(thread).bind(proposal).bind(entity).bind(id).bind(op)
        .bind((!before.is_null()).then(||before.to_string())).bind((!after.is_null()).then(||after.to_string())).execute(&mut *conn).await.map_err(|e|e.to_string())?;
    Ok(())
}

pub(super) async fn restore(
    conn: &mut SqliteConnection,
    entity: &str,
    id: &str,
    before: &Value,
) -> Result<(), String> {
    let table = super::inspect::table(entity)?;
    let n = super::inspect::numeric(entity, id)?;
    if before.is_null() {
        if entity == "task" {
            for table in ["task_tag_links", "task_files"] {
                sqlx::query(&format!("DELETE FROM {table} WHERE task_id=?1"))
                    .bind(n)
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        sqlx::query(&format!("DELETE FROM {table} WHERE id=?1"))
            .bind(n)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    let row = before["row"].as_object().ok_or("invalid journal row")?;
    let keys: Vec<_> = row.keys().collect();
    let columns = keys
        .iter()
        .map(|k| format!("\"{k}\""))
        .collect::<Vec<_>>()
        .join(",");
    let placeholders = (1..=keys.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let exists: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE id=?1"))
        .bind(n)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    // A plain UPDATE preserves the conflict policy inside the shared-tag
    // triggers; an outer UPSERT can turn their INSERT OR IGNORE into a failure.
    let sql = if exists > 0 {
        let sets = keys
            .iter()
            .enumerate()
            .map(|(i, k)| format!("\"{k}\"=?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");
        format!("UPDATE {table} SET {sets} WHERE id={n}")
    } else {
        format!("INSERT INTO {table} ({columns}) VALUES ({placeholders})")
    };
    let mut query = sqlx::query(&sql);
    for key in &keys {
        query = match &row[*key] {
            Value::Null => query.bind(Option::<String>::None),
            Value::Number(n) if n.is_i64() => query.bind(n.as_i64()),
            Value::Number(n) => query.bind(n.as_f64()),
            Value::String(s) => query.bind(s),
            _ => return Err("invalid journal column".into()),
        };
    }
    query.execute(&mut *conn).await.map_err(|e| e.to_string())?;
    if entity == "task" {
        for (field, table, key) in [
            ("tags", "task_tag_links", "tag_id"),
            ("files", "task_files", "file_id"),
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE task_id=?1"))
                .bind(n)
                .execute(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
            for link in before[field].as_array().into_iter().flatten() {
                if field == "files" {
                    sqlx::query(
                        "INSERT INTO task_files (task_id,file_id,created_at) VALUES (?1,?2,?3)",
                    )
                    .bind(n)
                    .bind(link[key].as_i64())
                    .bind(link["created_at"].as_str())
                    .execute(&mut *conn)
                    .await
                    .map_err(|e| e.to_string())?;
                } else {
                    sqlx::query("INSERT INTO task_tag_links (task_id,tag_id) VALUES (?1,?2)")
                        .bind(n)
                        .bind(link[key].as_i64())
                        .execute(&mut *conn)
                        .await
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    Ok(())
}

pub(super) async fn changes(conn: &mut SqliteConnection, id: &str) -> Result<Vec<Value>, String> {
    let rows=sqlx::query("SELECT entity,entity_id,before,after FROM agent_changes WHERE proposal_id=?1 AND reverted_at IS NULL ORDER BY id DESC").bind(id).fetch_all(&mut *conn).await.map_err(|e|e.to_string())?;
    rows.into_iter().map(|r|Ok(json!({"entity":r.get::<String,_>("entity"),"id":r.get::<String,_>("entity_id"),"before":parse(r.get("before"))?,"after":parse(r.get("after"))?}))).collect()
}
fn parse(v: Option<String>) -> Result<Value, String> {
    v.map(|v| serde_json::from_str(&v).map_err(|e| e.to_string()))
        .unwrap_or(Ok(Value::Null))
}

pub(super) async fn children_ids(conn: &mut SqliteConnection, id: i64) -> Result<Vec<i64>, String> {
    let rows = raw_rows(
        conn,
        "SELECT id FROM project_tasks WHERE parent_id=?1 ORDER BY id",
        id,
    )
    .await?;
    Ok(rows
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["id"].as_i64())
        .collect())
}
