//! The parity fixtures the TypeScript reference also runs
//! (`app/tests/lib/canvas/collabFixtures.test.ts`).

use super::super::error::Code;
use super::super::{format, model, transaction};
use serde_json::Value;
use std::path::PathBuf;

fn fixtures(dir: &str) -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures")
        .join(dir);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&path).unwrap())
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no fixtures in {}", dir.display());
    files
}

fn parse(text: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    model::validate(&value)?;
    Ok(value)
}

#[test]
fn valid_documents_parse_and_serialize_like_typescript() {
    let serialized =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/canvas-model/serialized");
    for (name, text) in fixtures("canvas-model/valid") {
        let value = parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let expected = std::fs::read_to_string(serialized.join(&name)).unwrap_or(text);
        assert_eq!(format::serialize(&value).unwrap(), expected, "{name}");
    }
}

#[test]
fn invalid_documents_are_rejected() {
    for (name, text) in fixtures("canvas-model/invalid") {
        assert!(parse(&text).is_err(), "{name} was accepted");
    }
}

fn code(code: Code) -> &'static str {
    match code {
        Code::ConflictShape => "conflict-shape",
        Code::ConflictOrder => "conflict-order",
        Code::ConflictBackground => "conflict-background",
        Code::Invalid => "invalid",
        other => panic!("unexpected code {other:?}"),
    }
}

#[test]
fn transactions_match_the_reference_implementation() {
    for (name, text) in fixtures("canvas-collab") {
        let fixture: Value = serde_json::from_str(&text).unwrap();
        let document = &fixture["document"];
        model::validate(document).unwrap_or_else(|e| panic!("{name}: {e}"));
        let outcome = transaction::apply(document, &fixture["transaction"]);
        match (
            &fixture["expect"]["ok"],
            fixture["expect"]["error"].as_str(),
        ) {
            (expected, None) => {
                let (result, _) = outcome.unwrap_or_else(|e| panic!("{name}: {e:?}"));
                assert!(
                    transaction::same_value(&result, expected),
                    "{name}: {result:#}"
                );
                let keys = |v: &Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
                assert_eq!(keys(&result), keys(document), "{name}: top-level key order");
            }
            (_, Some(expected)) => {
                let error = outcome.err().unwrap_or_else(|| panic!("{name} succeeded"));
                assert_eq!(code(error.code), expected, "{name}: {}", error.message);
            }
        }
    }
}

#[test]
fn effective_transaction_leaves_out_what_was_already_applied() {
    let shape = |id: &str, x: i64| {
        serde_json::json!({"id": id, "type": "rectangle", "x": x, "y": 0, "width": 1, "height": 1,
            "stroke": "#000", "fill": "#fff", "strokeWidth": 1, "style": "clean", "seed": 1})
    };
    let document = serde_json::json!({"type": "oculus-canvas", "version": 1,
        "elements": [shape("a", 5), shape("b", 0)], "viewport": {"x": 0, "y": 0, "zoom": 1},
        "background": "dots"});
    let tx = serde_json::json!({"edits": [
        {"id": "a", "before": shape("a", 0), "after": shape("a", 5)},
        {"id": "b", "before": shape("b", 0), "after": shape("b", 9)},
    ], "background": {"before": "#fff", "after": "dots"}});
    let (_, effective) = transaction::apply(&document, &tx).unwrap();
    assert_eq!(effective["edits"].as_array().unwrap().len(), 1);
    assert_eq!(effective["edits"][0]["id"], "b");
    assert!(effective.get("background").is_none());
    assert!(transaction::has_changes(&effective));
}

#[test]
fn image_data_urls_follow_the_reference_rules() {
    let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";
    assert!(model::is_image_data_url(png));
    assert!(!model::is_image_data_url(&png.replace("png;", "jpeg;")));
    assert!(!model::is_image_data_url("data:image/png;base64,===="));
    assert!(!model::is_image_data_url(
        "data:image/png;base64,iVBORw0KGgoAAAA=A=="
    ));
    assert!(!model::is_image_data_url(&format!("{png}\n")));
}

#[test]
fn additions_carry_their_placement_through_diff_and_the_ack() {
    let shape = |id: &str| {
        serde_json::json!({"id": id, "type": "rectangle", "x": 0, "y": 0, "width": 1, "height": 1,
            "stroke": "#000", "fill": "#fff", "strokeWidth": 1, "style": "clean", "seed": 1})
    };
    let doc = |ids: &[&str]| {
        serde_json::json!({"type": "oculus-canvas", "version": 1,
            "elements": ids.iter().map(|id| shape(id)).collect::<Vec<_>>(),
            "viewport": {"x": 0, "y": 0, "zoom": 1}, "background": "#fff"})
    };
    let tx = transaction::diff(&doc(&["a", "d"]), &doc(&["z", "a", "b", "c", "d", "top"]));
    let placed: Vec<(&str, Option<&Value>)> = tx["edits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["id"].as_str().unwrap(), e.get("afterId")))
        .collect();
    assert_eq!(
        placed,
        [
            ("z", Some(&Value::Null)),
            ("b", Some(&serde_json::json!("a"))),
            ("c", Some(&serde_json::json!("b"))),
            ("top", None),
        ]
    );
    let (result, effective) = transaction::apply(&doc(&["a", "d", "remote"]), &tx).unwrap();
    let ids: Vec<&str> = result["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["z", "a", "b", "c", "d", "remote", "top"]);
    assert_eq!(effective["edits"][1]["afterId"], "a");
    assert!(effective["edits"][3].get("afterId").is_none());
}
