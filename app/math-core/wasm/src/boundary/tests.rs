#![allow(clippy::unwrap_used, clippy::non_ascii_literal)]

use oculus_math_edit::{Command, Field};
use serde_json::{Value, json};

use super::{caret_at, command, selected, shortcuts, slots, step, stop_slots, stops};

/// The UTF-16 offset of byte `byte`, counted independently of `Units`.
fn units(source: &str, byte: usize) -> u32 {
    source[..byte].encode_utf16().count() as u32
}

fn open(source: &str) -> Field {
    Field::new(source, false).unwrap()
}

/// Runs `json` on `field` and checks the step's changes, then its
/// rewrite, applied in reverse order to the UTF-16 source, give the new
/// field's source. Returns the step and the new field.
fn run(field: &Field, json: &str) -> (Value, Field) {
    let outcome = field.run(&command(json).unwrap());
    let step: Value = serde_json::from_str(&step(field, &outcome)).unwrap();
    let mut text: Vec<u16> = field.source().encode_utf16().collect();
    for key in ["changes", "rewrite"] {
        let Some(changes) = step.get(key) else {
            continue;
        };
        for change in changes.as_array().unwrap().iter().rev() {
            let from = change["from"].as_u64().unwrap() as usize;
            let to = change["to"].as_u64().unwrap() as usize;
            let insert: Vec<u16> = change["insert"].as_str().unwrap().encode_utf16().collect();
            text.splice(from..to, insert);
        }
    }
    assert_eq!(String::from_utf16(&text).unwrap(), outcome.field.source());
    (step, outcome.field)
}

#[test]
fn thai_in_text_has_utf16_stops() {
    let source = r"\text{สวัสดี}";
    let field = open(source);
    let expected: Vec<u32> = field
        .stops()
        .stops()
        .iter()
        .map(|stop| units(source, stop.offset))
        .collect();
    assert_eq!(stops(&field), expected);
    // The row's ends, and the text run's stops between clusters (none
    // before a combining vowel: สวั|ส is a stop, สว|ั is not).
    assert_eq!(expected, [0, 6, 7, 9, 10, 12, 13]);
    // Each stop's offset places the caret back on it.
    for unit in expected {
        let field = caret_at(field.clone(), unit, true).unwrap();
        assert_eq!(selected(&field), [unit, unit]);
    }
}

#[test]
fn astral_characters_are_two_units() {
    let source = r"\text{𝒜b}";
    let field = open(source);
    assert_eq!(stops(&field), [0, 6, 8, 9, 10]);
    // Between the surrogates is no offset of the source.
    assert!(caret_at(field.clone(), 7, false).is_err());
    assert!(caret_at(field.clone(), 11, false).is_err());
    let field = caret_at(field, 8, false).unwrap();
    assert_eq!(selected(&field), [8, 8]);
    let (step, field) = run(&field, r#"{"insert": "𝒜"}"#);
    assert_eq!(
        step["changes"],
        json!([{"from": 8, "to": 8, "insert": "𝒜"}])
    );
    assert_eq!(selected(&field), [10, 10]);
}

#[test]
fn a_shortcut_rewrites_in_the_source_after_its_key() {
    let mut field = open(r"\text{ไทย}");
    let mut last = Value::Null;
    for key in ["s", "i", "n"] {
        (last, field) = run(&field, &format!(r#"{{"insert": "{key}"}}"#));
    }
    assert_eq!(field.source(), r"\text{ไทย}\sin");
    // `n` lands at UTF-16 offset 12 (byte 18), then a `\` before `sin`
    // (offset 10 of `\text{ไทย}sin`) makes it `\sin`.
    assert_eq!(
        last,
        json!({
            "changes": [{"from": 12, "to": 12, "insert": "n"}],
            "isolate": false,
            "rewrite": [{"from": 10, "to": 10, "insert": "\\"}],
        })
    );
    let (step, field) = run(&field, r#""escape""#);
    assert_eq!(field.source(), r"\text{ไทย}sin");
    assert_eq!(step["isolate"], json!(true));
    assert_eq!(selected(&field), [13, 13]);
}

#[test]
fn vertical_moves_read_xs_by_stop_id() {
    // Stops: 0 |\frac{ 6 |a 7 |b 8 |}{ 10 |c 11 |} 12 |.
    let field = open(r"\frac{ab}{c}");
    assert_eq!(stops(&field), [0, 6, 7, 8, 10, 11, 12]);
    let field = caret_at(field, 10, false).unwrap();
    // The caret's x is xs[4]; the numerator's nearest stop is id 3.
    let (step, up) = run(&field, r#"{"up": [null, 0, 5, 10, 9, 14, null]}"#);
    assert_eq!(step, json!({"changes": [], "isolate": false}));
    assert_eq!(selected(&up), [8, 8]);
    // Unmeasured, the caret goes to the slot's first stop.
    let (_, up) = run(&field, r#"{"up": []}"#);
    assert_eq!(selected(&up), [6, 6]);
    let (step, _) = run(
        &field,
        r#"{"down": [null, null, null, null, null, null, null]}"#,
    );
    assert_eq!(step["effect"], json!("leaveDown"));
}

#[test]
fn commands_decode_by_name() {
    assert_eq!(
        command(r#"{"left": {"extend": true}}"#),
        Ok(Command::Left { extend: true })
    );
    assert_eq!(
        command(r#"{"end": {}}"#),
        Ok(Command::End { extend: false })
    );
    assert_eq!(command(r#""deleteLine""#), Ok(Command::DeleteLine));
    assert_eq!(command(r#""shiftTab""#), Ok(Command::ShiftTab));
    assert_eq!(
        command(r#"{"template": "\\frac{#0}{#?}"}"#),
        Ok(Command::Template(r"\frac{#0}{#?}".to_owned()))
    );
    for bad in [
        r#""left""#,
        r#"{"insert": 1}"#,
        r#"{"jump": 2}"#,
        "",
        r#"{"left": {"by": 1}}"#,
    ] {
        assert!(command(bad).is_err(), "{bad}");
    }
}

#[test]
fn effects_and_selections() {
    let field = open("x");
    let (step, _) = run(&field, r#"{"right": {"extend": false}}"#);
    assert_eq!(step["effect"], json!("leaveRight"));
    let (_, all) = run(&field, r#""selectAll""#);
    assert_eq!(selected(&all), [0, 1]);
    let empty = open("");
    let (step, _) = run(&empty, r#""backspace""#);
    assert_eq!(step["effect"], json!("removeMaths"));
}

#[test]
fn slots_carry_kind_interior_and_parent() {
    let field = open(r"\text{ไทย}^{\frac{a}{b}}");
    let slots: Value = serde_json::from_str(&slots(&field)).unwrap();
    let ids = stop_slots(&field);
    assert_eq!(ids.len(), stops(&field).len());
    let kinds: Vec<&str> = slots
        .as_array()
        .unwrap()
        .iter()
        .map(|slot| slot["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["row", "text", "sup", "numer", "denom"]);
    assert_eq!(
        slots[0],
        json!({"kind": "row", "row": 0, "bounds": "open", "text": false, "from": 0, "to": 24, "parent": null})
    );
    // The text run's interior in UTF-16: ไทย is three units.
    assert_eq!(slots[1]["from"], json!(6));
    assert_eq!(slots[1]["to"], json!(9));
    assert_eq!(slots[1]["text"], json!(true));
    assert_eq!(slots[3]["parent"], json!(2));
}

#[test]
fn the_shortcut_table_is_pairs() {
    let table: Vec<(String, String)> = serde_json::from_str(&shortcuts()).unwrap();
    assert!(table.contains(&("sin".to_owned(), r"\sin".to_owned())));
}
