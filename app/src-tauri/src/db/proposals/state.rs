use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Row, SqliteConnection};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Made {
    pub entity: String,
    pub id: String,
    #[serde(default)]
    pub op: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Proposal {
    pub id: String,
    pub item_id: i64,
    pub block: i64,
    pub thread_id: i64,
    pub kind: String,
    pub status: String,
    pub state: Value,
    pub told: Option<String>,
    pub updated_at: String,
    pub made: Vec<Made>,
    pub edited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amended_from: Option<Value>,
}

pub fn key(id: &str) -> Result<(i64, i64), String> {
    let (item, block) = id.split_once('.').ok_or("invalid proposal id")?;
    let item = item.parse::<i64>().map_err(|_| "invalid proposal id")?;
    let block = block.parse::<i64>().map_err(|_| "invalid proposal id")?;
    if item <= 0 || block < 0 || format!("{item}.{block}") != id {
        return Err("invalid proposal id".into());
    }
    Ok((item, block))
}

pub async fn on(conn: &mut SqliteConnection, id: &str) -> Result<Proposal, String> {
    let (item, block) = key(id)?;
    let row = sqlx::query("SELECT * FROM chat_proposals WHERE item_id=?1 AND block=?2")
        .bind(item)
        .bind(block)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("this proposal no longer exists")?;
    let state: Value =
        serde_json::from_str(&row.get::<String, _>("state")).map_err(|e| e.to_string())?;
    let initial: Value =
        serde_json::from_str(&row.get::<String, _>("initial_state")).map_err(|e| e.to_string())?;
    // An undone card lists what it reverted; a live one only its current writes,
    // since an undone cheat card returns to pending and can be added again.
    let changes = sqlx::query("SELECT entity, entity_id, op, before, after FROM agent_changes WHERE proposal_id=?1 AND (reverted_at IS NULL OR ?2='undone') ORDER BY id")
        .bind(id).bind(row.get::<String, _>("status")).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
    let kind: String = row.get("kind");
    let made = changes
        .iter()
        .filter(|r| {
            let entity: String = r.get("entity");
            let op: String = r.get("op");
            match kind.as_str() {
                "NewTasks" => entity == "task" && op == "create",
                "NewProject" => (entity == "project" || entity == "task") && op == "create",
                "NewEvents" => entity == "event" && op == "create",
                "NewNote" => entity == "note",
                "NewCheatNotes" => entity == "cheat",
                "EditItems" => matches!(
                    entity.as_str(),
                    "cheat" | "question" | "quiz" | "artifact" | "topic"
                ),
                "TaskChanges" => entity == "task",
                "ProjectChanges" => entity == "project",
                "TagChanges" => entity == "tag",
                "EventChanges" => entity == "event",
                "DeleteTasks" => entity == "task" && op == "delete",
                "DeleteEvents" => entity == "event" && op == "delete",
                _ => false,
            }
        })
        .map(|r| {
            let after = r
                .get::<Option<String>, _>("after")
                .or_else(|| r.get::<Option<String>, _>("before"))
                .and_then(|s| serde_json::from_str::<Value>(&s).ok());
            let title = after
                .as_ref()
                .and_then(|v| {
                    v.pointer("/row/title")
                        .or_else(|| v.pointer("/row/name"))
                        .or_else(|| v.pointer("/row/prompt"))
                        .or_else(|| v.get("title"))
                })
                .and_then(Value::as_str)
                .map(str::to_owned);
            Made {
                entity: r.get("entity"),
                id: r.get("entity_id"),
                op: r.get("op"),
                title: title.or_else(|| {
                    (r.get::<String, _>("entity") == "note")
                        .then(|| {
                            state
                                .get("title")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                        })
                        .flatten()
                }),
            }
        })
        .collect();
    let amended_from = if let Some(previous) = state.get("amends").and_then(Value::as_str) {
        if let Ok((old, old_block)) = key(previous) {
            let raw:Option<String>=sqlx::query_scalar("SELECT state FROM chat_proposals WHERE item_id=?1 AND block=?2 AND thread_id=?3 AND item_id<?4 AND status='superseded' AND superseded_by=?5")
                .bind(old).bind(old_block).bind(row.get::<i64,_>("thread_id")).bind(item).bind(id).fetch_optional(&mut *conn).await.map_err(|e|e.to_string())?;
            raw.and_then(|s| serde_json::from_str(&s).ok())
        } else {
            None
        }
    } else {
        None
    };
    Ok(Proposal {
        id: id.into(),
        item_id: item,
        block,
        thread_id: row.get("thread_id"),
        kind: row.get("kind"),
        status: row.get("status"),
        edited: state != initial,
        state,
        told: row.get("told"),
        updated_at: row.get("updated_at"),
        made,
        superseded_by: row.get("superseded_by"),
        amended_from,
    })
}

pub fn object<'a>(
    v: &'a Value,
    keys: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, String> {
    let obj = v.as_object().ok_or("proposal fields must be an object")?;
    if let Some(key) = obj
        .keys()
        .find(|k| !keys.contains(&k.as_str()) && k.as_str() != "included")
    {
        return Err(format!("unknown proposal field: {key}"));
    }
    if obj.get("included").is_some_and(|v| !v.is_boolean()) {
        return Err("included must be a boolean".into());
    }
    Ok(obj)
}

pub fn text(v: &Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} needs text"))
}

pub fn nullable_text(v: &Value, key: &str) -> Result<Option<String>, String> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(format!("{key} must be text or null")),
    }
}

pub fn id(v: &Value) -> Result<i64, String> {
    v.as_i64()
        .filter(|i| *i > 0)
        .ok_or_else(|| "an id must be a positive integer".into())
}
pub fn nullable_id(v: Option<&Value>) -> Result<Option<i64>, String> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(v) => id(v).map(Some),
    }
}
pub fn included(v: &Value) -> bool {
    v.get("included").and_then(Value::as_bool) != Some(false)
}
pub fn rows<'a>(v: &'a Value, key: &str) -> Result<&'a [Value], String> {
    let a = v
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} needs a list"))?;
    if a.len() > 500 {
        return Err("a proposal may have at most 500 rows".into());
    }
    Ok(a)
}
pub fn strings(v: &Value, key: &str) -> Result<Vec<String>, String> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Array(a)) if a.len() <= 500 => a
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{key} needs text entries"))
            })
            .collect(),
        _ => Err(format!("{key} needs a list of text")),
    }
}
pub fn date(v: &Value, key: &str) -> Result<Option<String>, String> {
    nullable_text(v, key)?
        .map(|s| crate::db::projects::check_iso8601(&s))
        .transpose()
}

pub fn kind_keys(kind: &str) -> Result<&'static [&'static str], String> {
    Ok(match kind {
        "NewTasks" => &["project", "items", "amends"],
        "NewProject" => &[
            "name", "subject", "tasks", "due", "brief", "starts", "tags", "event", "amends",
        ],
        "NewEvents" => &["events", "amends"],
        "NewCheatNotes" => &["notes", "amends"],
        "NewNote" => &["subject", "title", "body", "amends"],
        "TaskChanges" | "ProjectChanges" | "EventChanges" | "TagChanges" | "EditItems" => {
            &["changes", "amends"]
        }
        "DeleteTasks" => &["tasks", "amends"],
        "DeleteEvents" => &["events", "amends"],
        "Sync" => &["subjects", "lectures"],
        "Reparse" => &["files"],
        "Transcribe" => &["file"],
        "LectureChapters" | "LectureEnd" => &["lecture"],
        "Download" => &["item"],
        _ => return Err(format!("unknown proposal component {kind}")),
    })
}

pub fn validate_shape(kind: &str, state: &Value) -> Result<(), String> {
    object(state, kind_keys(kind)?)?;
    if serde_json::to_vec(state).map_err(|e| e.to_string())?.len() > 2_000_000 {
        return Err("proposal is too large".into());
    }
    Ok(())
}
