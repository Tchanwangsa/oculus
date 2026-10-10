use super::parse::{clean_location, clean_title, context_id, event_row, flatten_events};
use super::KIND_CLASS;
use std::collections::HashSet;

fn ev(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

#[test]
fn numeric_and_prefixed_ids_both_normalise() {
    assert_eq!(
        context_id(&ev(r#"{"id":12}"#), "event").unwrap(),
        "event_12"
    );
    assert_eq!(
        context_id(&ev(r#"{"id":"assignment_9"}"#), "assignment").unwrap(),
        "assignment_9"
    );
    assert!(context_id(&ev(r#"{}"#), "event").is_none());
}

#[test]
fn timetabled_titles_lose_the_course_prefix_and_keep_the_group() {
    assert_eq!(
        clean_title(
            "IT Project (COMP30022_2026_SM2) (Tutorial 1 (16))",
            "IT Project (COMP30022_2026_SM2)"
        ),
        "Tutorial 1 (16)"
    );
    assert_eq!(
        clean_title(
            "Models of Computation (COMP30026_2026_SM2) (Lecture 2 (1))",
            "Models of Computation (COMP30026_2026_SM2)"
        ),
        "Lecture 2 (1)"
    );
    assert_eq!(
        clean_title(
            "Week 9 Tutorial Wireshark Monday 12pm",
            "Information Security"
        ),
        "Week 9 Tutorial Wireshark Monday 12pm"
    );
    assert_eq!(clean_title("(a) then (b)", ""), "(a) then (b)");
    assert_eq!(clean_title("Course X", "Course X"), "Course X");
}

#[test]
fn venues_lose_the_capacity_and_keep_the_room() {
    assert_eq!(
        clean_location("PAR-192-L2-L108-Laby Theatre (210)"),
        "PAR-192-L2-L108-Laby Theatre"
    );
    assert_eq!(
        clean_location("X-Departmentally Organised Venue"),
        "X-Departmentally Organised Venue"
    );
    assert_eq!(clean_location("Room 4 (online)"), "Room 4 (online)");
}

#[test]
fn undated_and_deleted_events_are_dropped() {
    assert!(event_row(&ev(r#"{"id":1,"title":"x"}"#)).is_none());
    assert!(event_row(&ev(
        r#"{"id":1,"start_at":"2026-08-10T01:00:00Z","workflow_state":"deleted"}"#
    ))
    .is_none());
}

#[test]
fn children_replace_their_parent_filtered_to_my_sections() {
    let raw = ev(r#"[{
            "id": 1, "title": "Tutorial", "start_at": "2026-08-10T01:00:00Z",
            "child_events": [
              {"id": 2, "title": "Tutorial 01", "start_at": "2026-08-10T01:00:00Z",
               "context_code": "course_section_11"},
              {"id": 3, "title": "Tutorial 02", "start_at": "2026-08-10T03:00:00Z",
               "context_code": "course_section_22"}
            ]
        }]"#);
    let mine: HashSet<String> = ["course_section_22".to_string()].into_iter().collect();

    let kept = flatten_events(raw.as_array().unwrap(), &mine);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].title, "Tutorial 02");
    assert_eq!(kept[0].id, "event_3");
}

#[test]
fn unknown_sections_keep_every_child_not_none() {
    let raw = ev(r#"[{
            "id": 1, "start_at": "2026-08-10T01:00:00Z",
            "child_events": [
              {"id": 2, "title": "A", "start_at": "2026-08-10T01:00:00Z",
               "context_code": "course_section_11"},
              {"id": 3, "title": "B", "start_at": "2026-08-10T03:00:00Z",
               "context_code": "course_section_22"}
            ]
        }]"#);
    let kept = flatten_events(raw.as_array().unwrap(), &HashSet::new());
    assert_eq!(kept.len(), 2);
}

#[test]
fn the_same_occurrence_arriving_twice_is_kept_once() {
    let raw = ev(r#"[
          {"id": 1, "start_at": "2026-08-10T01:00:00Z", "hidden": true,
           "child_events": [
             {"id": 2, "title": "Tutorial 02", "start_at": "2026-08-10T01:00:00Z",
              "context_code": "course_section_22"}]},
          {"id": 2, "title": "Tutorial 02", "start_at": "2026-08-10T01:00:00Z",
           "context_code": "course_section_22", "parent_event_id": 1}
        ]"#);
    let mine: HashSet<String> = ["course_section_22".to_string()].into_iter().collect();
    let kept = flatten_events(raw.as_array().unwrap(), &mine);
    assert_eq!(kept.len(), 1);
}

#[test]
fn a_hidden_parent_with_no_children_in_hand_is_skipped() {
    let raw = ev(r#"[{"id": 1, "title": "Tutorial", "hidden": true,
                          "start_at": "2026-08-10T01:00:00Z"}]"#);
    assert!(flatten_events(raw.as_array().unwrap(), &HashSet::new()).is_empty());
}

#[test]
fn a_childless_event_is_kept_as_itself() {
    let raw = ev(
        r#"[{"id": 7, "title": "Lecture", "start_at": "2026-08-10T01:00:00Z",
                          "end_at": "2026-08-10T02:00:00Z", "location_name": "  Alice Hoy 210 "}]"#,
    );
    let kept = flatten_events(raw.as_array().unwrap(), &HashSet::new());
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].location.as_deref(), Some("Alice Hoy 210"));
    assert_eq!(kept[0].kind, KIND_CLASS);
}
