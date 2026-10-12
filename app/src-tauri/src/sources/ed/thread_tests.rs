//! Ed threads: front matter, category colours, and the shared encoding.

use super::front::front_matter;
use super::palette::category_color;
use super::threads::{thread_md, Board};
use serde_json::{json, Value};

fn categories() -> Vec<String> {
    ["General", "Lectures", "Assignments"]
        .map(String::from)
        .to_vec()
}

fn question(answered: bool) -> Value {
    json!({"thread": {
        "id": 98765, "number": 46, "type": "question", "title": "Late penalty?",
        "category": "Assignments", "subcategory": "A2", "subsubcategory": "",
        "is_answered": answered, "is_staff_answered": false,
        "created_at": "2026-10-01T09:30:00+10:00",
        "document": "Is there one?", "user_id": 1, "answers": [], "comments": []
    }, "users": [{"id": 1, "name": "Ada"}]})
}

#[test]
fn a_thread_opens_with_its_front_matter_and_keeps_its_body() {
    let cats = categories();
    let board = Board {
        region: Some("au"),
        course_id: 1,
        categories: &cats,
    };
    let md = thread_md(&json!({"id": 98765}), &question(false), &board);
    assert!(
        md.starts_with(
            "---\n\
             ed_thread_id: 98765\n\
             number: 46\n\
             type: \"question\"\n\
             answered: false\n\
             category: \"Assignments\"\n\
             subcategory: \"A2\"\n\
             subsubcategory: null\n\
             category_color: \"#f39c12\"\n\
             url: \"https://edstem.org/au/courses/1/discussion/98765\"\n\
             ---\n\
             # Late penalty?\n\n\
             **#46 · question · unresolved · Assignments / A2**  \n"
        ),
        "{md}"
    );
}

#[test]
fn answered_follows_the_meta_line_and_is_null_off_questions() {
    let cats = categories();
    let board = Board {
        region: None,
        course_id: 1,
        categories: &cats,
    };
    // A staff answer on the listing counts, as on the meta line.
    let listing = json!({"id": 98765, "is_staff_answered": true});
    let md = thread_md(&listing, &question(false), &board);
    assert!(md.contains("\nanswered: true\n"), "{md}");
    assert!(md.contains("· resolved ·"), "{md}");
    // No region: no link.
    assert!(md.contains("\nurl: null\n"), "{md}");

    let post = json!({"thread": {"id": 5, "number": 7, "type": "post", "title": "Hi",
                                 "category": "Social"}});
    let md = thread_md(&json!({"id": 5}), &post, &board);
    assert!(md.contains("\nanswered: null\n"), "{md}");
    // A category the course does not list has no colour.
    assert!(md.contains("\ncategory_color: null\n"), "{md}");
    assert!(md.contains("\nsubcategory: null\n"), "{md}");
}

#[test]
fn colours_go_by_position_and_stop_after_the_palette() {
    let many: Vec<String> = (0..20).map(|i| format!("c{i}")).collect();
    assert_eq!(category_color(&many, "c0"), Some("#3498db"));
    assert_eq!(category_color(&many, "c15"), Some("#38eab5"));
    assert_eq!(category_color(&many, "c16"), None);
    assert_eq!(category_color(&[], "c0"), None);
}

#[test]
fn front_matter_is_one_json_literal_per_line() {
    let md = front_matter(&[
        ("title", json!("A \"quoted\"\nline")),
        ("none", Value::Null),
        ("empty", json!([])),
        ("items", json!([{"a": 1}, "b"])),
    ]);
    assert_eq!(
        md,
        "---\ntitle: \"A \\\"quoted\\\"\\nline\"\nnone: null\nempty: []\nitems:\n  - {\"a\":1}\n  - \"b\"\n---\n"
    );
}
