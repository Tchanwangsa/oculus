use super::*;
use serde_json::json;

fn style(v: Value) -> Result<Style, String> {
    match v {
        Value::Object(m) => Style::from_json(&m),
        _ => Err("not an object".to_owned()),
    }
}

#[test]
fn reads_the_common_keys() {
    let s = style(json!({
        "width": 200, "height": "50%", "padding": [8, 16], "gap": 12,
        "fill": { "linear": { "angle": 90, "stops": [[0, "#000"], [1, "#ffffffcc"]] } },
        "shadow": { "y": 8, "blur": 24, "color": "#00000040" },
        "flexDirection": "column", "clip": true, "maxHeight": 300,
    }))
    .unwrap();
    assert_eq!(s.width, Some(Dim::Px(200.0)));
    assert_eq!(s.height, Some(Dim::Pct(0.5)));
    assert_eq!(s.padding.unwrap().left, 16.0);
    assert_eq!(s.flex_direction, Some(FlexDirection::Column));
    assert_eq!(s.max_height, Some(Dim::Px(300.0)));
    assert!(matches!(s.fill, Some(Fill::Linear { .. })));
}

#[test]
fn unknown_key_names_the_closest() {
    let e = style(json!({ "pading": 4 })).unwrap_err();
    assert!(e.contains("unknown style key `pading`"), "{e}");
    assert!(e.contains("did you mean `padding`"), "{e}");
}

#[test]
fn bad_values_say_what_is_accepted() {
    let e = style(json!({ "padding": "12px" })).unwrap_err();
    assert!(e.contains("style.padding"), "{e}");
    assert!(e.contains("[top, right, bottom, left]"), "{e}");
    let e = style(json!({ "color": "red" })).unwrap_err();
    assert!(e.contains("#rrggbb"), "{e}");
    let e = style(json!({ "width": null })).unwrap_err();
    assert!(e.contains("NaN"), "{e}");
    let e = style(json!({ "flexDirection": "colum" })).unwrap_err();
    assert!(e.contains("column"), "{e}");
    let e = style(json!({ "shadow": { "y": 2, "blurr": 3, "color": "#000" } })).unwrap_err();
    assert!(e.contains("blurr"), "{e}");
}

#[test]
fn every_key_is_listed_once() {
    let mut keys = STYLE_KEYS.to_vec();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), STYLE_KEYS.len());
}
