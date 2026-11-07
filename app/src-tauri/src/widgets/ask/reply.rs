//! The JSON value out of a model's reply, checked against the schema, with
//! one more turn when it doesn't match.

use std::cell::RefCell;

use serde_json::Value;

use super::schema::Schema;

/// The first fragment of `reply` that is JSON and matches `schema`, through
/// the lenient extractor the lecture jobs use (`reply_json::parse_reply`:
/// the whole reply, fenced blocks, then balanced brackets). When fragments parse
/// but none matches, the error is the longest one's mismatch: an inner list
/// of a wrong object is not what the model meant to send.
pub fn extract(reply: &str, schema: &Schema) -> Result<Value, String> {
    let mismatch: RefCell<Option<(usize, String)>> = RefCell::new(None);
    let found = crate::harness::reply_json::parse_reply(reply, "JSON value", |text| {
        let value: Value = serde_json::from_str(text).ok()?;
        match schema.check(&value) {
            Ok(()) => Some(value),
            Err(e) => {
                let mut m = mismatch.borrow_mut();
                if m.as_ref().is_none_or(|(len, _)| text.len() > *len) {
                    *m = Some((text.len(), e));
                }
                None
            }
        }
    });
    found.map_err(|none| mismatch.into_inner().map_or(none, |(_, e)| e))
}

/// One turn, and once more with the reason when its reply doesn't match.
/// `turn` answers None when the request was stopped, which ends it here.
pub fn answer(
    mut turn: impl FnMut(&str) -> Result<Option<String>, String>,
    request: &str,
    schema: &Schema,
) -> Result<Option<Value>, String> {
    let mut failure = String::new();
    for attempt in 0..2 {
        let text = if attempt == 0 {
            request.to_string()
        } else {
            format!(
                "{request}\n\nYour previous reply was rejected: {failure}\n\
                 Reply again with only the JSON value."
            )
        };
        let Some(reply) = turn(&text)? else {
            return Ok(None);
        };
        match extract(&reply, schema) {
            Ok(value) => return Ok(Some(value)),
            Err(e) => failure = e,
        }
    }
    Err(format!(
        "The model's reply did not match the schema: {failure}"
    ))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn schema() -> Schema {
        Schema::parse(&json!({
            "type": "object",
            "properties": { "steps": { "type": "array", "items": { "type": "string" } } },
            "required": ["steps"]
        }))
        .unwrap()
    }

    #[test]
    fn json_is_found_in_fences_and_prose() {
        let s = schema();
        let want = json!({ "steps": ["a", "b"] });
        for reply in [
            r#"{"steps":["a","b"]}"#,
            "```json\n{\"steps\": [\"a\", \"b\"]}\n```",
            "Here you go: {\"steps\": [\"a\", \"b\"]} — hope that helps.",
        ] {
            assert_eq!(extract(reply, &s).unwrap(), want, "{reply}");
        }
    }

    #[test]
    fn a_fragment_that_parses_but_mismatches_says_why() {
        let e = extract(r#"{"steps": ["a", 2]}"#, &schema()).unwrap_err();
        assert_eq!(e, "$.steps[1]: expected string");
        let wrapped = extract(r#"Sure: {"steps": [1], "n": 2} done"#, &schema()).unwrap_err();
        assert_eq!(wrapped, "$.steps[0]: expected string");
        let none = extract("no json here", &schema()).unwrap_err();
        assert!(none.starts_with("no JSON value in the reply"), "{none}");
    }

    #[test]
    fn a_scalar_schema_takes_a_bare_value() {
        let s = Schema::parse(&json!({ "type": "integer" })).unwrap();
        assert_eq!(extract(" 42 ", &s).unwrap(), json!(42));
    }

    #[test]
    fn a_mismatch_is_asked_again_once_with_the_reason() {
        let mut sent: Vec<String> = Vec::new();
        let got = answer(
            |text| {
                sent.push(text.to_string());
                Ok(Some(if sent.len() == 1 {
                    r#"{"steps": "a"}"#.into()
                } else {
                    r#"{"steps": ["a"]}"#.into()
                }))
            },
            "REQ",
            &schema(),
        );
        assert_eq!(got.unwrap(), Some(json!({ "steps": ["a"] })));
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], "REQ");
        assert!(
            sent[1].starts_with("REQ\n\nYour previous reply was rejected: $.steps: expected array")
        );
        assert!(sent[1].ends_with("Reply again with only the JSON value."));
    }

    #[test]
    fn two_mismatches_fail_with_the_last_reason() {
        let mut turns = 0;
        let got = answer(
            |_| {
                turns += 1;
                Ok(Some(r#"{"nope": 1}"#.into()))
            },
            "REQ",
            &schema(),
        );
        assert_eq!(turns, 2);
        assert_eq!(
            got.unwrap_err(),
            "The model's reply did not match the schema: $: missing property \"steps\""
        );
    }

    #[test]
    fn a_provider_error_is_not_retried_and_a_stop_ends_quietly() {
        let mut turns = 0;
        let got = answer(
            |_| {
                turns += 1;
                Err("the CLI is missing".into())
            },
            "REQ",
            &schema(),
        );
        assert_eq!(
            (got.unwrap_err().as_str(), turns),
            ("the CLI is missing", 1)
        );
        assert_eq!(answer(|_| Ok(None), "REQ", &schema()).unwrap(), None);
    }
}
