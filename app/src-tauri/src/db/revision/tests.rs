use super::*;
use serde_json::json;
async fn setup() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all() {
        sqlx::raw_sql(migration.sql).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO subjects(id,code,name) VALUES(1,'A','A'),(2,'B','B')")
        .execute(&pool)
        .await
        .unwrap();
    pool
}
#[tokio::test]
async fn revision_json_roundtrips_and_batch_failure_rolls_back() {
    let db = setup().await;
    let a = json!({"subject_id":1,"kind":"choice","prompt":"Which?","answer":"A","options":["A","B"],"rubric":[],"topic":["Algebra"]});
    let ids = add_batch(&db, Entity::Item, &[a.clone(), a.clone()])
        .await
        .unwrap();
    assert_eq!(ids.len(), 2);
    assert_eq!(
        show(&db, Entity::Item, ids[0]).await.unwrap()["options"],
        json!(["A", "B"])
    );
    let bad = json!({"subject_id":1,"kind":"unknown","prompt":"Bad","answer":"Bad"});
    assert!(add_batch(&db, Entity::Item, &[a, bad]).await.is_err());
    assert_eq!(list(&db, Entity::Item).await.unwrap().len(), 2);
}
#[tokio::test]
async fn quiz_sets_retain_order_and_deletes_clean_relations() {
    let db = setup().await;
    let a = add(
        &db,
        Entity::Item,
        &json!({"subject_id":1,"kind":"card","prompt":"Front","answer":"Back"}),
    )
    .await
    .unwrap();
    let b = add(
        &db,
        Entity::Item,
        &json!({"subject_id":1,"kind":"numeric","prompt":"2+2","answer":"4"}),
    )
    .await
    .unwrap();
    let set = add_set(&db, &json!({"subject_id":1,"title":"Practice"}), &[b, a])
        .await
        .unwrap();
    assert_eq!(set_items(&db, set).await.unwrap(), vec![b, a]);
    remove(&db, Entity::Item, a).await.unwrap();
    assert_eq!(set_items(&db, set).await.unwrap(), vec![b]);
}
#[tokio::test]
async fn notes_and_artifact_state_are_independent() {
    let db = setup().await;
    let note = add(
        &db,
        Entity::Note,
        &json!({"subject_id":1,"title":"Definition","body":"$x$","tags":["Topic"]}),
    )
    .await
    .unwrap();
    let artifact = add(
        &db,
        Entity::Artifact,
        &json!({"subject_id":1,"title":"Slider","program":"root = Stack([])","state":{"$x":7}}),
    )
    .await
    .unwrap();
    update(&db, Entity::Note, note, &json!({"body":"Changed"}))
        .await
        .unwrap();
    assert_eq!(
        show(&db, Entity::Artifact, artifact).await.unwrap()["state"],
        json!({"$x":7})
    );
    assert!(
        update(&db, Entity::Note, note, &json!({"untrusted_column":"no"}))
            .await
            .is_err()
    );
}

async fn topic(db: &SqlitePool, subject: i64, name: &str) -> i64 {
    topics::add(db, &json!({"subject_id":subject,"name":name}))
        .await
        .unwrap()
}
#[tokio::test]
async fn topics_required_scope_checked_and_updates_atomic() {
    let db = setup().await;
    let a = topic(&db, 1, "Algebra").await;
    assert_eq!(a, topic(&db, 1, "algebra").await);
    let b = topic(&db, 1, "Functions").await;
    let wrong = topic(&db, 2, "Algebra").await;
    let note = add(&db, Entity::Note, &json!({"subject_id":1,"title":"Before"}))
        .await
        .unwrap();
    let unsorted = show(&db, Entity::Note, note).await.unwrap()["topic_ids"].clone();
    assert_eq!(unsorted.as_array().unwrap().len(), 1);
    update(&db, Entity::Note, note, &json!({"topic_ids":[a,b]}))
        .await
        .unwrap();
    assert_eq!(
        show(&db, Entity::Note, note).await.unwrap()["topic_ids"],
        json!([a, b])
    );
    assert!(update(
        &db,
        Entity::Note,
        note,
        &json!({"subject_id":1,"title":"Must roll back","topic_ids":[wrong]})
    )
    .await
    .is_err());
    assert_eq!(
        show(&db, Entity::Note, note).await.unwrap()["title"],
        "Before"
    );
    topics::remove(&db, a).await.unwrap();
    assert_eq!(
        show(&db, Entity::Note, note).await.unwrap()["topic_ids"],
        json!([b])
    );
    topics::remove(&db, b).await.unwrap();
    assert_eq!(
        show(&db, Entity::Note, note).await.unwrap()["topic_ids"],
        unsorted
    );
    let builtin = unsorted[0].as_i64().unwrap();
    assert!(topics::remove(&db, builtin).await.is_err());
    assert!(topics::update(&db, builtin, &json!({"name":"Renamed"}))
        .await
        .is_err());
    update(&db, Entity::Note, note, &json!({"subject_id":2}))
        .await
        .unwrap();
    assert_ne!(
        show(&db, Entity::Note, note).await.unwrap()["topic_ids"],
        unsorted
    );
    update(&db, Entity::Note, note, &json!({"topic_ids":[wrong]}))
        .await
        .unwrap();
    update(&db, Entity::Note, note, &json!({"topic_ids":[]}))
        .await
        .unwrap();
    assert_eq!(
        show(&db, Entity::Note, note).await.unwrap()["topic_ids"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn direct_sql_and_subject_deletion_keep_memberships() {
    let db = setup().await;
    for (entity,sql) in [
        (Entity::Note,"INSERT INTO revision_notes(subject_id,title) VALUES(1,'Note')"),
        (Entity::Item,"INSERT INTO revision_items(subject_id,kind,prompt,answer) VALUES(1,'card','Q','A')"),
        (Entity::Set,"INSERT INTO revision_sets(subject_id,title) VALUES(1,'Quiz')"),
        (Entity::Artifact,"INSERT INTO revision_artifacts(subject_id,title,program) VALUES(1,'App','root=Stack([])')"),
    ] {
        let id=sqlx::query(sql).execute(&db).await.unwrap().last_insert_rowid();
        assert_eq!(show(&db,entity,id).await.unwrap()["topic_ids"].as_array().unwrap().len(),1);
    }
    sqlx::query("DELETE FROM subjects WHERE id=1")
        .execute(&db)
        .await
        .unwrap();
    for entity in [Entity::Note, Entity::Item, Entity::Set, Entity::Artifact] {
        let row = show(&db, entity, 1).await.unwrap();
        assert_eq!(row["subject_id"], Value::Null);
        assert_eq!(row["topic_ids"].as_array().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn migration_backfills_labels_and_journals() {
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in crate::db::migrations::all()
        .into_iter()
        .filter(|m| m.version < 51)
    {
        sqlx::raw_sql(migration.sql).execute(&db).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO subjects(id,code,name) VALUES(1,'A','A'); INSERT INTO revision_notes(title,tags) VALUES('N','[\"Algebra\",\" algebra \",\"\",\"Unsorted\"]'); INSERT INTO revision_items(subject_id,kind,prompt,answer,topic) VALUES(1,'card','Q','A','[\"Algebra\"]'); INSERT INTO agent_changes(thread_id,proposal_id,entity,entity_id,op,after) VALUES(1,'1:0','cheat','1','create','{\"row\":{\"id\":1,\"subject_id\":null}}');").execute(&db).await.unwrap();
    sqlx::raw_sql(include_str!("topics.sql"))
        .execute(&db)
        .await
        .unwrap();
    let named: Vec<Value> = topics::list(&db)
        .await
        .unwrap()
        .into_iter()
        .filter(|v| v["is_unsorted"] == 0)
        .collect();
    assert_eq!(named.len(), 2);
    assert_eq!(
        show(&db, Entity::Note, 1).await.unwrap()["tags"],
        json!(["Algebra", " algebra ", "", "Unsorted"])
    );
    let ids = show(&db, Entity::Note, 1).await.unwrap()["topic_ids"].clone();
    assert_eq!(ids.as_array().unwrap().len(), 1);
    let after: String = sqlx::query_scalar("SELECT after FROM agent_changes")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&after).unwrap()["topic_ids"],
        ids
    );
}
#[tokio::test]
async fn invalid_topics_roll_back_batch_and_set() {
    let db = setup().await;
    assert!(add_batch(
        &db,
        Entity::Note,
        &[
            json!({"subject_id":1,"title":"First"}),
            json!({"subject_id":1,"title":"Invalid","topic_ids":[999]})
        ]
    )
    .await
    .is_err());
    assert!(list(&db, Entity::Note).await.unwrap().is_empty());
    let item = add(
        &db,
        Entity::Item,
        &json!({"subject_id":1,"kind":"card","prompt":"Q","answer":"A"}),
    )
    .await
    .unwrap();
    assert!(add_set(
        &db,
        &json!({"subject_id":1,"title":"Invalid","topic_ids":[999]}),
        &[item]
    )
    .await
    .is_err());
    assert!(list(&db, Entity::Set).await.unwrap().is_empty());
}

#[tokio::test]
async fn topic_delete_repairs_links_with_foreign_keys_disabled() {
    let db = setup().await;
    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&db)
        .await
        .unwrap();
    let a = topic(&db, 1, "First").await;
    let b = topic(&db, 1, "Second").await;
    let note = add(
        &db,
        Entity::Note,
        &json!({"subject_id":1,"title":"N","topic_ids":[a]}),
    )
    .await
    .unwrap();
    let item = add(
        &db,
        Entity::Item,
        &json!({"subject_id":1,"kind":"card","prompt":"Q","answer":"A","topic_ids":[a]}),
    )
    .await
    .unwrap();
    let set = add(
        &db,
        Entity::Set,
        &json!({"subject_id":1,"title":"Q","topic_ids":[a]}),
    )
    .await
    .unwrap();
    let artifact = add(
        &db,
        Entity::Artifact,
        &json!({"subject_id":1,"title":"A","program":"root=Stack([])","topic_ids":[a]}),
    )
    .await
    .unwrap();
    topics::remove(&db, a).await.unwrap();
    for (entity, id) in [
        (Entity::Note, note),
        (Entity::Item, item),
        (Entity::Set, set),
        (Entity::Artifact, artifact),
    ] {
        let row = show(&db, entity, id).await.unwrap();
        let ids = row["topic_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(
            topics::show(&db, ids[0].as_i64().unwrap()).await.unwrap()["is_unsorted"],
            1
        );
    }
    update(&db, Entity::Note, note, &json!({"topic_ids":[b]}))
        .await
        .unwrap();
    sqlx::query("DELETE FROM revision_topics WHERE id=?")
        .bind(b)
        .execute(&db)
        .await
        .unwrap();
    assert_eq!(
        show(&db, Entity::Note, note).await.unwrap()["topic_ids"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let dangling:i64=sqlx::query_scalar("SELECT COUNT(*) FROM revision_note_topics m LEFT JOIN revision_topics t ON t.id=m.topic_id WHERE t.id IS NULL").fetch_one(&db).await.unwrap();
    assert_eq!(dangling, 0);
}
#[tokio::test]
async fn first_note_into_an_empty_table_takes_a_real_position() {
    let db = setup().await;
    let first = add(
        &db,
        Entity::Note,
        &json!({"subject_id":1,"title":"First","body":""}),
    )
    .await
    .unwrap();
    let second = add(
        &db,
        Entity::Note,
        &json!({"subject_id":1,"title":"Second","body":""}),
    )
    .await
    .unwrap();
    assert_eq!(
        show(&db, Entity::Note, first).await.unwrap()["position"],
        json!(0.0)
    );
    assert_eq!(
        show(&db, Entity::Note, second).await.unwrap()["position"],
        json!(1.0)
    );
}
#[tokio::test]
async fn every_entry_and_topic_names_a_subject() {
    let db = setup().await;
    for entity in [Entity::Note, Entity::Set] {
        assert!(add(&db, entity, &json!({"title":"Loose"})).await.is_err());
    }
    let note = add(&db, Entity::Note, &json!({"subject_id":1,"title":"Kept"}))
        .await
        .unwrap();
    assert!(update(&db, Entity::Note, note, &json!({"subject_id":null}))
        .await
        .is_err());
    assert!(topics::add(&db, &json!({"name":"Loose"})).await.is_err());
    assert!(topics::add(&db, &json!({"subject_id":null,"name":"Loose"}))
        .await
        .is_err());
}
