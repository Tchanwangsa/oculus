use super::*;

#[tokio::test]
async fn project_properties_and_multiline_markdown_round_trip_without_losing_tasks() {
    let pool = super::tag_tests::migrated_pool().await;
    sqlx::raw_sql(
        "INSERT INTO subjects (id, code, name) VALUES (1, 'SUBJ1', 'Subject');
        INSERT INTO calendar_events (id, subject_id, kind, title, start_at)
        VALUES ('assignment_123', 1, 'due', 'Assignment', '2026-10-16T07:00:00Z');",
    )
    .execute(&pool)
    .await
    .unwrap();
    let markdown = "# Brief\n\n- First\n- Second: it's $x^2$\n\n```txt\na\\b\n```\n";
    let id = create_project(
        &pool,
        &NewProject {
            name: "Assignment".into(),
            subject_id: Some(1),
            brief: Some(markdown.into()),
            event_id: Some("assignment_123".into()),
            tags: vec!["Assignment".into()],
            starts_at: Some("2026-10-15T09:00:00+11:00".into()),
            due_at: Some("2026-10-16T18:00:00+11:00".into()),
            source: "agent".into(),
        },
    )
    .await
    .unwrap();
    let created = project(&pool, id).await.unwrap().unwrap();
    assert_eq!(created.brief.as_deref(), Some(markdown));
    assert_eq!(created.event_id.as_deref(), Some("assignment_123"));
    assert_eq!(
        created.starts_at.as_deref(),
        Some("2026-10-15T09:00:00+11:00")
    );
    let task_id = create_tasks(
        &pool,
        Some(id),
        &[NewTask {
            title: "Draft".into(),
            body: Some(markdown.into()),
            ..Default::default()
        }],
        "agent",
    )
    .await
    .unwrap()[0];
    update_project(
        &pool,
        id,
        &ProjectPatch {
            subject_id: Some(None),
            brief: Some(Some("Updated\n\nParagraph\n".into())),
            event_id: Some(None),
            status: Some("archived".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let updated = project(&pool, id).await.unwrap().unwrap();
    assert_eq!(updated.subject_id, None);
    assert_eq!(updated.event_id, None);
    assert_eq!(updated.status, "archived");
    assert_eq!(updated.brief.as_deref(), Some("Updated\n\nParagraph\n"));
    assert_eq!(updated.due_at, created.due_at);
    let task = task(&pool, task_id).await.unwrap().unwrap();
    assert_eq!(task.project_id, Some(id));
    assert_eq!(task.body.as_deref(), Some(markdown));
    update_project(
        &pool,
        id,
        &ProjectPatch {
            subject_id: Some(Some(1)),
            event_id: Some(Some("assignment_123".into())),
            brief: Some(None),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let updated = project(&pool, id).await.unwrap().unwrap();
    assert_eq!(updated.subject_id, Some(1));
    assert_eq!(updated.brief, None);
    assert_eq!(updated.event_id.as_deref(), Some("assignment_123"));
    pool.close().await;
}

#[tokio::test]
async fn event_discovery_matches_the_picker_and_invalid_pins_write_nothing() {
    let pool = super::tag_tests::migrated_pool().await;
    sqlx::raw_sql("INSERT INTO subjects (id, code, name) VALUES (1, 'S1', 'S1'), (2, 'S2', 'S2');
        INSERT INTO calendar_events (id, subject_id, kind, title, start_at)
        VALUES ('class_1', 1, 'class', 'Class', '2026-10-16T07:00:00Z');
        INSERT INTO local_events (id, kind, title, start_at) VALUES (1, 'note', 'Personal', '2026-10-15T09:00:00Z');
        INSERT INTO lectures (id, lesson_id, subject_id, title, date) VALUES
        ('one', 'one', 1, 'Hidden recording', '2026-10-16T09:00:00'),
        ('two', 'two', 2, 'Fallback recording', '2026-10-16T09:00:00');")
        .execute(&pool).await.unwrap();
    let events = project_events(&pool, None).await.unwrap();
    let ids: Vec<_> = events.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["local_1", "class_1", "lecture_two"]);
    assert_eq!(project_events(&pool, Some(1)).await.unwrap().len(), 1);
    let id = create_project(
        &pool,
        &NewProject {
            name: "Original".into(),
            event_id: Some("local_1".into()),
            source: "agent".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    for invalid in ["missing", "task_1", "lecture_one"] {
        let error = update_project(
            &pool,
            id,
            &ProjectPatch {
                name: Some("Should not change".into()),
                event_id: Some(Some(invalid.into())),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(error.contains("no linkable calendar event"), "{error}");
        assert_eq!(project(&pool, id).await.unwrap().unwrap().name, "Original");
        assert!(create_project(
            &pool,
            &NewProject {
                name: "Rejected".into(),
                event_id: Some(invalid.into()),
                source: "agent".into(),
                ..Default::default()
            }
        )
        .await
        .is_err());
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    pool.close().await;
}
