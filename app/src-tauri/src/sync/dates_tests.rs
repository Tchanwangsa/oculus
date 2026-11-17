use serde_json::json;

use super::SourceDates;

fn utc(day: u8) -> String {
    format!("2026-10-{day:02}T00:00:00.000Z")
}

#[test]
fn canvas_dates_use_source_visibility_and_normalize_offsets() {
    let file = json!({
        "created_at": "2026-10-02T00:00:00Z",
        "unlock_at": "2026-10-02T11:00:00+10:00",
        "modified_at": "2026-10-04T10:00:00+10:00",
        "updated_at": "2026-10-05T00:00:00Z"
    });
    let dates = SourceDates::file(&file);
    assert_eq!(
        dates.source_created_at.as_deref(),
        Some("2026-10-02T01:00:00.000Z")
    );
    assert_eq!(dates.source_updated_at, Some(utc(4)));
    let earlier_unlock = json!({"created_at": utc(3), "unlock_at": utc(1), "updated_at": utc(4)});
    assert_eq!(
        SourceDates::file(&earlier_unlock).source_created_at,
        Some(utc(3))
    );
    assert_eq!(
        SourceDates::file(&earlier_unlock).source_updated_at,
        Some(utc(4))
    );
    // Assignments use unlock itself, even if it predates creation.
    assert_eq!(
        SourceDates::assignment(&earlier_unlock).source_created_at,
        Some(utc(1))
    );
    assert_eq!(
        SourceDates::assignment(&json!({"created_at": utc(2)})).source_created_at,
        Some(utc(2))
    );
    assert_eq!(
        SourceDates::page(&earlier_unlock),
        SourceDates {
            source_created_at: Some(utc(3)),
            source_updated_at: Some(utc(4)),
        }
    );
    assert_eq!(
        SourceDates::announcement(
            &json!({"posted_at": utc(2), "created_at": utc(1), "updated_at": utc(5)})
        ),
        SourceDates {
            source_created_at: Some(utc(2)),
            source_updated_at: Some(utc(2)),
        }
    );
    assert_eq!(
        SourceDates::announcement(&json!({"created_at": utc(1)})).source_updated_at,
        Some(utc(1))
    );
}

#[test]
fn missing_and_malformed_dates_do_not_become_local_write_times() {
    for item in [
        json!({}),
        json!({"created_at": "bad", "updated_at": "2026-10-02", "unlock_at": "", "modified_at": 7, "posted_at": null}),
    ] {
        assert_eq!(SourceDates::file(&item), SourceDates::default());
        assert_eq!(SourceDates::page(&item), SourceDates::default());
        assert_eq!(SourceDates::announcement(&item), SourceDates::default());
        assert_eq!(SourceDates::assignment(&item), SourceDates::default());
    }
}

#[test]
fn ed_activity_is_the_latest_nested_reply_not_an_edit_or_roster_date() {
    let listing = json!({"created_at": utc(1)});
    let detail = json!({"thread": {
        "created_at": utc(2), "updated_at": utc(9),
        "answers": [{"created_at": utc(3), "comments": [
            {"created_at": "2026-10-05T10:00:00+10:00", "comments": [
                {"created_at": "2026-10-05T00:00:00.123456Z"}
            ]}
        ]}],
        "comments": [{"created_at": utc(4), "updated_at": utc(8)}, {"created_at": "bad"}],
        "users": [{"created_at": utc(9)}]
    }});
    assert_eq!(
        SourceDates::thread(&listing, &detail),
        SourceDates {
            source_created_at: Some(utc(2)),
            source_updated_at: Some("2026-10-05T00:00:00.123Z".into()),
        }
    );
    assert_eq!(
        SourceDates::thread(&listing, &json!({})),
        SourceDates {
            source_created_at: Some(utc(1)),
            source_updated_at: None,
        }
    );
    let encoded = serde_json::to_value(crate::sync::FileEvent {
        subject_id: 1,
        code: "SUBJ".into(),
        relative_path: "courses/SUBJ/ed/0001.md".into(),
        size_bytes: 1,
        category: "ed".into(),
        canvas_id: None,
        source_url: None,
        source_dates: SourceDates::thread(&listing, &detail),
        action: "unchanged",
    })
    .unwrap();
    assert_eq!(encoded["source_created_at"], utc(2));
    assert_eq!(encoded["source_updated_at"], "2026-10-05T00:00:00.123Z");
    assert!(
        encoded.get("source_dates").is_none(),
        "the TS writer consumes flat fields"
    );
}
