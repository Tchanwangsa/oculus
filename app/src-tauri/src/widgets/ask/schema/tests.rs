use serde_json::{json, Value};

use super::{Schema, MAX_DEPTH};

fn schema(v: Value) -> Schema {
    Schema::parse(&v).unwrap_or_else(|e| panic!("{v} should read: {e}"))
}

fn refused(v: Value) -> String {
    Schema::parse(&v).expect_err(&format!("{v} should be refused"))
}

fn steps() -> Schema {
    schema(json!({
        "type": "object",
        "properties": {
            "steps": {
                "type": "array",
                "minItems": 1,
                "maxItems": 4,
                "items": {
                    "type": "object",
                    "properties": {
                        "tex": { "type": "string", "minLength": 1 },
                        "why": { "type": ["string", "null"], "description": "one line" }
                    },
                    "required": ["tex"],
                    "additionalProperties": false
                }
            },
            "score": { "type": "integer", "minimum": 0, "maximum": 10 },
            "verdict": { "enum": ["right", "wrong"] }
        },
        "required": ["steps"]
    }))
}

#[test]
fn a_matching_value_passes() {
    let s = steps();
    s.check(&json!({ "steps": [{ "tex": "x^2" }, { "tex": "2x", "why": null }] }))
        .unwrap();
    s.check(&json!({ "steps": [{ "tex": "a", "why": "b" }], "score": 3, "verdict": "right" }))
        .unwrap();
    // Not closed at the root, so another property is fine there.
    s.check(&json!({ "steps": [{ "tex": "a" }], "extra": true }))
        .unwrap();
}

#[test]
fn errors_name_the_path_in_the_value() {
    let s = steps();
    let cases = [
        (
            json!({ "steps": [{ "tex": "a" }, { "tex": "b" }, { "tex": 3 }] }),
            "$.steps[2].tex: expected string",
        ),
        (json!({}), "$: missing property \"steps\""),
        (json!([]), "$: expected object"),
        (
            json!({ "steps": [{ "tex": "a", "note": 1 }] }),
            "$.steps[0]: unexpected property \"note\"",
        ),
        (
            json!({ "steps": [] }),
            "$.steps: expected at least 1 items, got 0",
        ),
        (
            json!({ "steps": [{"tex":"a"},{"tex":"a"},{"tex":"a"},{"tex":"a"},{"tex":"a"}] }),
            "$.steps: expected at most 4 items, got 5",
        ),
        (
            json!({ "steps": [{ "tex": "" }] }),
            "$.steps[0].tex: expected at least 1 characters, got 0",
        ),
        (
            json!({ "steps": [{ "tex": "a", "why": 2 }] }),
            "$.steps[0].why: expected string or null",
        ),
        (
            json!({ "steps": [{ "tex": "a" }], "score": 2.5 }),
            "$.score: expected integer",
        ),
        (
            json!({ "steps": [{ "tex": "a" }], "score": 11 }),
            "$.score: expected at most 10",
        ),
        (
            json!({ "steps": [{ "tex": "a" }], "score": -1 }),
            "$.score: expected at least 0",
        ),
        (
            json!({ "steps": [{ "tex": "a" }], "verdict": "maybe" }),
            "$.verdict: must be one of [\"right\",\"wrong\"]",
        ),
    ];
    for (value, want) in cases {
        assert_eq!(s.check(&value).unwrap_err(), want, "{value}");
    }
}

#[test]
fn an_awkward_key_is_quoted_in_the_path() {
    let s = schema(
        json!({ "properties": { "a b": { "type": "number" }, "1x": { "type": "number" } } }),
    );
    assert_eq!(
        s.check(&json!({ "a b": "x" })).unwrap_err(),
        "$[\"a b\"]: expected number"
    );
    assert_eq!(
        s.check(&json!({ "1x": "x" })).unwrap_err(),
        "$[\"1x\"]: expected number"
    );
}

#[test]
fn integers_and_numbers() {
    let int = schema(json!({ "type": "integer" }));
    int.check(&json!(3)).unwrap();
    int.check(&json!(3.0)).unwrap();
    int.check(&json!(-4)).unwrap();
    assert!(int.check(&json!(3.5)).is_err());
    assert!(int.check(&json!("3")).is_err());
    let num = schema(json!({ "type": "number", "minimum": 0.5 }));
    num.check(&json!(2)).unwrap();
    num.check(&json!(0.5)).unwrap();
    assert_eq!(
        num.check(&json!(0.25)).unwrap_err(),
        "$: expected at least 0.5"
    );
}

#[test]
fn const_enum_and_scalars() {
    let c = schema(json!({ "const": { "kind": "x" } }));
    c.check(&json!({ "kind": "x" })).unwrap();
    assert_eq!(
        c.check(&json!({ "kind": "y" })).unwrap_err(),
        "$: must be {\"kind\":\"x\"}"
    );
    let b = schema(json!({ "type": "boolean" }));
    b.check(&json!(false)).unwrap();
    assert_eq!(b.check(&json!(null)).unwrap_err(), "$: expected boolean");
    let n = schema(json!({ "type": "null" }));
    n.check(&json!(null)).unwrap();
    let s = schema(json!({ "type": "string", "maxLength": 3 }));
    // Characters, not bytes.
    s.check(&json!("θφψ")).unwrap();
    assert_eq!(
        s.check(&json!("abcd")).unwrap_err(),
        "$: expected at most 3 characters, got 4"
    );
}

#[test]
fn a_keyword_applies_only_to_its_own_type() {
    let s = schema(json!({ "type": ["string", "array"], "minLength": 2, "minItems": 1 }));
    s.check(&json!("ab")).unwrap();
    s.check(&json!(["a"])).unwrap();
    assert!(s.check(&json!("a")).is_err());
    assert!(s.check(&json!([])).is_err());
}

#[test]
fn an_empty_schema_takes_anything() {
    let s = schema(json!({}));
    for v in [
        json!(null),
        json!(1),
        json!("x"),
        json!([1, "a"]),
        json!({ "a": [] }),
    ] {
        s.check(&v).unwrap();
    }
}

#[test]
fn keywords_outside_the_subset_are_refused_with_their_place() {
    assert_eq!(
        refused(json!({ "type": "object", "oneOf": [] })),
        "schema $: \"oneOf\" is not supported"
    );
    assert_eq!(
        refused(json!({ "properties": { "steps": { "items": { "pattern": "x" } } } })),
        "schema $.properties.steps.items: \"pattern\" is not supported"
    );
    for kw in [
        "$schema",
        "$ref",
        "anyOf",
        "allOf",
        "not",
        "format",
        "patternProperties",
        "prefixItems",
        "uniqueItems",
        "default",
    ] {
        let e = refused(json!({ kw: true }));
        assert!(e.contains("is not supported"), "{kw}: {e}");
    }
}

#[test]
fn malformed_keywords_are_refused() {
    for bad in [
        json!("string"),
        json!(true),
        json!({ "type": "text" }),
        json!({ "type": [] }),
        json!({ "type": ["string", 3] }),
        json!({ "properties": [] }),
        json!({ "properties": { "a": "string" } }),
        json!({ "required": "a" }),
        json!({ "required": ["a", 1] }),
        json!({ "additionalProperties": { "type": "string" } }),
        json!({ "items": [{ "type": "string" }] }),
        json!({ "enum": [] }),
        json!({ "enum": "a" }),
        json!({ "minItems": -1 }),
        json!({ "maxLength": 1.5 }),
        json!({ "minimum": "0" }),
        json!({ "description": 3 }),
    ] {
        refused(bad);
    }
}

#[test]
fn nesting_is_capped() {
    let mut v = json!({ "type": "string" });
    for _ in 1..MAX_DEPTH {
        v = json!({ "type": "array", "items": v });
    }
    schema(v.clone());
    let deeper = json!({ "type": "array", "items": v });
    assert!(refused(deeper).contains(&format!("nests deeper than {MAX_DEPTH} levels")));
}
