use super::*;
use crate::theme::Theme;
use serde_json::{Map, Value, json};

fn schema() -> PropsSchema {
    parse_schema(&json!({
        "title": { "kind": "str", "optional": false },
        "bullets": { "kind": "list", "optional": false, "of": { "kind": "str", "optional": false }, "default": [] },
        "size": { "kind": "oneOf", "optional": false, "values": ["s", "m"], "default": "m" },
        "accent": { "kind": "token", "optional": false, "token": "color", "default": "accent" },
        "note": { "kind": "str", "optional": true },
    }))
    .unwrap()
}

fn run(props: serde_json::Value) -> Result<Map<String, Value>, String> {
    let theme = Theme::bundled("default").unwrap();
    validate_props("Card", &schema(), props.as_object().unwrap(), &theme).map_err(|e| e.to_string())
}

#[test]
fn fills_defaults_and_resolves_tokens() {
    let out = run(json!({ "title": "Hi" })).unwrap();
    assert_eq!(out["bullets"], json!([]));
    assert_eq!(out["size"], json!("m"));
    assert_eq!(out["accent"], json!("#5e6ad2"));
    assert!(!out.contains_key("note"));
}

#[test]
fn errors_name_component_path_expected_and_got() {
    let e = run(json!({})).unwrap_err();
    assert_eq!(
        e,
        "Card prop `title`: expected a string (required), got nothing (missing)"
    );
    let e = run(json!({ "title": "x", "bullets": ["a", 3] })).unwrap_err();
    assert_eq!(e, "Card prop `bullets[1]`: expected a string, got 3");
    let e = run(json!({ "titel": "x" })).unwrap_err();
    assert!(e.contains("did you mean `title`"), "{e}");
    let e = run(json!({ "title": "x", "accent": "acent" })).unwrap_err();
    assert!(e.contains("did you mean `accent`"), "{e}");
    let e = run(json!({ "title": "x", "size": "xl" })).unwrap_err();
    assert!(e.contains("one of \"s\", \"m\""), "{e}");
}

#[test]
fn rejects_non_builders() {
    let e = parse_schema(&json!({ "title": "string" })).unwrap_err();
    assert!(e.contains("props.title is not a prop builder"), "{e}");
}

#[test]
fn a_spec_serialises_to_the_json_its_builder_wrote() {
    let written = json!({
        "title": { "kind": "str", "optional": false },
        "bullets": { "kind": "list", "optional": false, "of": { "kind": "num", "optional": true }, "default": [] },
        "size": { "kind": "oneOf", "optional": false, "values": ["s", "m"], "default": "m" },
        "accent": { "kind": "token", "optional": false, "token": "color", "default": "accent" },
        "math": { "kind": "latex", "optional": true },
        "on": { "kind": "bool", "optional": false, "default": true },
    });
    let schema = parse_schema(&written).unwrap();
    for (name, spec) in &schema {
        assert_eq!(
            &serde_json::to_value(spec).unwrap(),
            &written[name],
            "{name}"
        );
    }
}
