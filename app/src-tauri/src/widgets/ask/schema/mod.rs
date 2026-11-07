//! The subset of JSON Schema a widget's `ask` may use (docs/widgets.md):
//! `type` (one of [`Kind`] or a list of them), `properties`, `required`,
//! `additionalProperties` (a boolean), `items` (one schema), `enum`, `const`,
//! `minItems`, `maxItems`, `minimum`, `maximum`, `minLength`, `maxLength`,
//! and the notes `description` and `title`. Any other keyword is refused when
//! the schema is read, before a model is asked, so a widget can't rely on a
//! rule that is never checked. As in JSON Schema, a keyword about one type
//! (`minLength`, `properties`, …) applies only to a value of that type.

mod check;
#[cfg(test)]
mod tests;

use serde_json::Value;

/// How deep `properties` and `items` may nest, the root counting as one.
pub const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Object,
    Array,
    String,
    Number,
    Integer,
    Boolean,
    Null,
}

impl Kind {
    fn parse(name: &str) -> Option<Kind> {
        Some(match name {
            "object" => Kind::Object,
            "array" => Kind::Array,
            "string" => Kind::String,
            "number" => Kind::Number,
            "integer" => Kind::Integer,
            "boolean" => Kind::Boolean,
            "null" => Kind::Null,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Object => "object",
            Kind::Array => "array",
            Kind::String => "string",
            Kind::Number => "number",
            Kind::Integer => "integer",
            Kind::Boolean => "boolean",
            Kind::Null => "null",
        }
    }
}

/// One schema, read. Absent keywords are `None` or empty.
#[derive(Debug, Clone, Default)]
pub struct Schema {
    types: Option<Vec<Kind>>,
    properties: Vec<(String, Schema)>,
    required: Vec<String>,
    /// `additionalProperties`; true when absent.
    closed: bool,
    items: Option<Box<Schema>>,
    one_of: Option<Vec<Value>>,
    constant: Option<Value>,
    min_items: Option<u64>,
    max_items: Option<u64>,
    minimum: Option<f64>,
    maximum: Option<f64>,
    min_length: Option<u64>,
    max_length: Option<u64>,
}

impl Schema {
    /// Reads `value` as a schema of the subset. Errors name the schema's own
    /// path: `schema $.properties.steps: "oneOf" is not supported`.
    pub fn parse(value: &Value) -> Result<Schema, String> {
        read(value, "$", 1)
    }
}

fn read(value: &Value, at: &str, depth: usize) -> Result<Schema, String> {
    let fail = |what: String| format!("schema {at}: {what}");
    if depth > MAX_DEPTH {
        return Err(fail(format!("nests deeper than {MAX_DEPTH} levels")));
    }
    let Value::Object(map) = value else {
        return Err(fail("a schema is an object".into()));
    };
    let mut s = Schema::default();
    for (key, v) in map {
        let count = || {
            v.as_u64()
                .ok_or_else(|| fail(format!("\"{key}\" takes a whole number of 0 or more")))
        };
        let number = || {
            v.as_f64()
                .ok_or_else(|| fail(format!("\"{key}\" takes a number")))
        };
        match key.as_str() {
            "type" => s.types = Some(types(v).ok_or_else(|| fail(type_help()))?),
            "properties" => {
                let Value::Object(props) = v else {
                    return Err(fail("\"properties\" takes an object of schemas".into()));
                };
                for (name, sub) in props {
                    let path = format!("{at}.properties{}", check::key_path(name));
                    s.properties
                        .push((name.clone(), read(sub, &path, depth + 1)?));
                }
            }
            "required" => {
                s.required = v
                    .as_array()
                    .and_then(|a| a.iter().map(|n| n.as_str().map(String::from)).collect())
                    .ok_or_else(|| fail("\"required\" takes a list of property names".into()))?;
            }
            "additionalProperties" => {
                let open = v.as_bool().ok_or_else(|| {
                    fail(
                        "\"additionalProperties\" takes true or false (a schema is not supported)"
                            .into(),
                    )
                })?;
                s.closed = !open;
            }
            "items" => {
                if !v.is_object() {
                    return Err(fail("\"items\" takes one schema".into()));
                }
                s.items = Some(Box::new(read(v, &format!("{at}.items"), depth + 1)?));
            }
            "enum" => match v {
                Value::Array(values) if !values.is_empty() => s.one_of = Some(values.clone()),
                _ => return Err(fail("\"enum\" takes a list of one or more values".into())),
            },
            "const" => s.constant = Some(v.clone()),
            "minItems" => s.min_items = Some(count()?),
            "maxItems" => s.max_items = Some(count()?),
            "minLength" => s.min_length = Some(count()?),
            "maxLength" => s.max_length = Some(count()?),
            "minimum" => s.minimum = Some(number()?),
            "maximum" => s.maximum = Some(number()?),
            "description" | "title" => {
                if !v.is_string() {
                    return Err(fail(format!("\"{key}\" takes a string")));
                }
            }
            other => return Err(fail(format!("\"{other}\" is not supported"))),
        }
    }
    Ok(s)
}

/// `type`'s value: one kind or a non-empty list of them.
fn types(v: &Value) -> Option<Vec<Kind>> {
    match v {
        Value::String(name) => Kind::parse(name).map(|k| vec![k]),
        Value::Array(names) if !names.is_empty() => names
            .iter()
            .map(|n| n.as_str().and_then(Kind::parse))
            .collect(),
        _ => None,
    }
}

fn type_help() -> String {
    "\"type\" takes object, array, string, number, integer, boolean or null, or a list of them"
        .into()
}
