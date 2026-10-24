//! A review and its FSRS transition commit together on one connection.
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewInput {
    item_id: i64,
    expected_reps: i64,
    answer: String,
    rating: i64,
    score: Option<f64>,
    max: Option<f64>,
    feedback: Option<String>,
    judge_model: Option<String>,
    #[serde(default)]
    overridden: bool,
    seconds: Option<f64>,
    set_id: Option<i64>,
    session_id: Option<String>,
    scheduled: bool,
    working: Option<String>,
    card: Option<Value>,
}
#[tauri::command]
pub async fn revision_record_review(input: ReviewInput) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    record(&pool, input).await
}
async fn record(pool: &sqlx::SqlitePool, input: ReviewInput) -> Result<(), String> {
    if !(1..=4).contains(&input.rating)
        || input.seconds.is_some_and(|s| !s.is_finite() || s < 0.0)
        || input.score.is_some() != input.max.is_some()
        || input
            .score
            .zip(input.max)
            .is_some_and(|(s, m)| !s.is_finite() || !m.is_finite() || m <= 0.0 || s < 0.0 || s > m)
    {
        return Err("Invalid review score, rating or time.".into());
    }
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM revision_items WHERE id=?1")
            .bind(input.item_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    if status.as_deref() != Some("active") {
        return Err("Only accepted, active questions can be reviewed.".into());
    }
    if let Some(set) = input.set_id {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revision_set_items WHERE set_id=?1 AND item_id=?2",
        )
        .bind(set)
        .bind(input.item_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if count != 1 {
            return Err("This question is not in that quiz.".into());
        }
    }
    if input.scheduled {
        let reps: i64 = sqlx::query_scalar("SELECT reps FROM revision_items WHERE id=?1")
            .bind(input.item_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        if reps != input.expected_reps {
            return Err(
                "This question was reviewed in another view. Refresh the queue before continuing."
                    .into(),
            );
        }

        let card = input.card.as_ref().ok_or("Missing FSRS transition")?;
        let number = |k: &str| -> Result<f64, String> {
            card[k]
                .as_f64()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| format!("Invalid FSRS {k}"))
        };
        let integer = |k: &str| -> Result<i64, String> {
            card[k]
                .as_i64()
                .filter(|v| *v >= 0)
                .ok_or_else(|| format!("Invalid FSRS {k}"))
        };
        let due = card["due"].as_str().ok_or("Missing due date")?;
        crate::db::projects::check_iso8601(due)?;
        let last = card["last_review"].as_str().ok_or("Missing last review")?;
        crate::db::projects::check_iso8601(last)?;
        if integer("reps")? != input.expected_reps + 1 {
            return Err("Invalid FSRS repetition transition".into());
        }
        let state = integer("state")?;
        if state > 3 {
            return Err("Invalid FSRS state".into());
        }
        sqlx::query("UPDATE revision_items SET due=?1,stability=?2,fsrs_difficulty=?3,reps=?4,lapses=?5,state=?6,last_review=?7,learning_steps=?8,updated_at=datetime('now') WHERE id=?9")
            .bind(due).bind(number("stability")?).bind(number("fsrs_difficulty")?).bind(integer("reps")?).bind(integer("lapses")?).bind(state).bind(last).bind(integer("learning_steps")?).bind(input.item_id).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    }
    if let Some(session) = &input.session_id {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revision_reviews WHERE session_id=?1 AND item_id=?2",
        )
        .bind(session)
        .bind(input.item_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        if exists > 0 {
            return Err("This answer has already been recorded.".into());
        }
    }
    sqlx::query("INSERT INTO revision_reviews (item_id,set_id,session_id,scheduled,answer,rating,score,max,feedback,judge_model,overridden,seconds,working) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)")
        .bind(input.item_id).bind(input.set_id).bind(input.session_id).bind(input.scheduled).bind(input.answer).bind(input.rating).bind(input.score).bind(input.max).bind(input.feedback).bind(input.judge_model).bind(input.overridden).bind(input.seconds).bind(input.working).execute(&mut *tx).await.map_err(|e|e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn revision_save_items(mut items: Vec<Value>) -> Result<Vec<i64>, String> {
    if items.is_empty() || items.len() > 500 {
        return Err("Supply 1–500 questions".into());
    }
    for item in &mut items {
        let object = item.as_object_mut().ok_or("Question must be an object")?;
        object.insert("status".into(), Value::String("draft".into()));
        object.insert("created_by".into(), Value::String("agent".into()));
    }
    let pool = crate::db::store::open_pool().await?;
    crate::db::revision::add_batch(&pool, crate::db::revision::Entity::Item, &items).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    async fn setup() -> (sqlx::SqlitePool, i64) {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        for m in crate::db::migrations::all() {
            sqlx::raw_sql(m.sql).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO subjects(id,code,name) VALUES(1,'A','A')")
            .execute(&pool)
            .await
            .unwrap();
        let id = crate::db::revision::add(
            &pool,
            crate::db::revision::Entity::Item,
            &json!({"subject_id":1,"kind":"card","prompt":"Front","answer":"Back","status":"active"}),
        )
        .await
        .unwrap();
        (pool, id)
    }
    fn input(id: i64, scheduled: bool) -> ReviewInput {
        serde_json::from_value(json!({"item_id":id,"expected_reps":0,"answer":"Student","rating":3,"score":1,"max":1,"overridden":false,"scheduled":scheduled,"card":{"due":"2026-10-25T00:00:00Z","last_review":"2026-10-24T00:00:00Z","stability":2,"fsrs_difficulty":5,"reps":1,"lapses":0,"state":2,"learning_steps":0}})).unwrap()
    }
    #[tokio::test]
    async fn scheduled_attempt_is_atomic_and_rejects_a_stale_card() {
        let (db, id) = setup().await;
        record(&db, input(id, true)).await.unwrap();
        assert!(record(&db, input(id, true)).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM revision_reviews")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }
    #[tokio::test]
    async fn quiz_attempt_leaves_the_schedule_untouched_and_cannot_duplicate() {
        let (db, id) = setup().await;
        let set = crate::db::revision::add_set(&db, &json!({"subject_id":1,"title":"Quiz"}), &[id])
            .await
            .unwrap();
        let mut first = input(id, false);
        first.set_id = Some(set);
        first.session_id = Some("session".into());
        record(&db, first).await.unwrap();
        let mut second = input(id, false);
        second.set_id = Some(set);
        second.session_id = Some("session".into());
        assert!(record(&db, second).await.is_err());
        let reps: i64 = sqlx::query_scalar("SELECT reps FROM revision_items WHERE id=?1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(reps, 0);
    }
    #[tokio::test]
    async fn invalid_transition_or_draft_cannot_create_an_attempt() {
        let (db, id) = setup().await;
        let mut bad = input(id, true);
        bad.card.as_mut().unwrap()["state"] = json!(7);
        assert!(record(&db, bad).await.is_err());
        sqlx::query("UPDATE revision_items SET status='draft'")
            .execute(&db)
            .await
            .unwrap();
        assert!(record(&db, input(id, false)).await.is_err());
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM revision_reviews")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}

#[tauri::command]
pub async fn revision_create_set(
    title: String,
    subject_id: i64,
    items: Vec<i64>,
    topic_ids: Option<Vec<i64>>,
) -> Result<i64, String> {
    // A quiz may start empty; questions are added to it later.
    if items.len() > 500 {
        return Err("A quiz holds at most 500 questions".into());
    }
    let pool = crate::db::store::open_pool().await?;
    crate::db::revision::add_set(
        &pool,
        &match topic_ids {
            Some(ids) => serde_json::json!({"title":title,"subject_id":subject_id,"topic_ids":ids}),
            None => serde_json::json!({"title":title,"subject_id":subject_id}),
        },
        &items,
    )
    .await
}

fn entity_name(entity: &str) -> Result<crate::db::revision::Entity, String> {
    use crate::db::revision::Entity;
    match entity {
        "note" => Ok(Entity::Note),
        "item" => Ok(Entity::Item),
        "set" => Ok(Entity::Set),
        "artifact" => Ok(Entity::Artifact),
        _ => Err("Unknown revision entity".into()),
    }
}
#[tauri::command]
pub async fn revision_create_entity(entity: String, mut input: Value) -> Result<i64, String> {
    let pool = crate::db::store::open_pool().await?;
    let object = input.as_object_mut().ok_or("Expected a revision object")?;
    if entity == "item" {
        object
            .entry("created_by")
            .or_insert_with(|| Value::String("user".into()));
    }
    crate::db::revision::add(&pool, entity_name(&entity)?, &input).await
}
#[tauri::command]
pub async fn revision_update_entity(entity: String, id: i64, patch: Value) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    crate::db::revision::update(&pool, entity_name(&entity)?, id, &patch).await
}
#[tauri::command]
pub async fn revision_create_topic(subject_id: i64, name: String) -> Result<i64, String> {
    let pool = crate::db::store::open_pool().await?;
    crate::db::revision::topics::add(
        &pool,
        &serde_json::json!({"subject_id":subject_id,"name":name}),
    )
    .await
}
#[tauri::command]
pub async fn revision_update_topic(id: i64, patch: Value) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    crate::db::revision::topics::update(&pool, id, &patch).await
}
#[tauri::command]
pub async fn revision_delete_topic(id: i64) -> Result<(), String> {
    let pool = crate::db::store::open_pool().await?;
    crate::db::revision::topics::remove(&pool, id).await
}
