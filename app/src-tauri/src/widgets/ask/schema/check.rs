//! Checking a value against a read [`Schema`]. The first failure is the
//! answer, named by its path in the value: `$.steps[2].tex: expected string`.

use serde_json::Value;

use super::{Kind, Schema};

impl Schema {
    pub fn check(&self, value: &Value) -> Result<(), String> {
        self.check_at(value, "$")
    }

    fn check_at(&self, value: &Value, at: &str) -> Result<(), String> {
        let fail = |what: String| Err(format!("{at}: {what}"));
        if let Some(c) = &self.constant {
            if value != c {
                return fail(format!("must be {}", clip(c)));
            }
        }
        if let Some(values) = &self.one_of {
            if !values.contains(value) {
                return fail(format!(
                    "must be one of {}",
                    clip(&Value::Array(values.clone()))
                ));
            }
        }
        if let Some(types) = &self.types {
            if !types.iter().any(|&k| is(value, k)) {
                let names: Vec<&str> = types.iter().map(|k| k.name()).collect();
                return fail(format!("expected {}", names.join(" or ")));
            }
        }
        match value {
            Value::Object(map) => {
                for name in &self.required {
                    if !map.contains_key(name) {
                        return fail(format!("missing property {}", quoted(name)));
                    }
                }
                for (name, v) in map {
                    match self.properties.iter().find(|(n, _)| n == name) {
                        Some((_, sub)) => sub.check_at(v, &format!("{at}{}", key_path(name)))?,
                        None if self.closed => {
                            return fail(format!("unexpected property {}", quoted(name)))
                        }
                        None => {}
                    }
                }
            }
            Value::Array(items) => {
                let n = items.len() as u64;
                if let Some(min) = self.min_items.filter(|&m| n < m) {
                    return fail(format!("expected at least {min} items, got {n}"));
                }
                if let Some(max) = self.max_items.filter(|&m| n > m) {
                    return fail(format!("expected at most {max} items, got {n}"));
                }
                if let Some(sub) = &self.items {
                    for (i, item) in items.iter().enumerate() {
                        sub.check_at(item, &format!("{at}[{i}]"))?;
                    }
                }
            }
            Value::String(text) => {
                let n = text.chars().count() as u64;
                if let Some(min) = self.min_length.filter(|&m| n < m) {
                    return fail(format!("expected at least {min} characters, got {n}"));
                }
                if let Some(max) = self.max_length.filter(|&m| n > m) {
                    return fail(format!("expected at most {max} characters, got {n}"));
                }
            }
            Value::Number(n) => {
                let x = n.as_f64().unwrap_or(f64::NAN);
                if let Some(min) = self.minimum.filter(|&m| x < m) {
                    return fail(format!("expected at least {}", figure(min)));
                }
                if let Some(max) = self.maximum.filter(|&m| x > m) {
                    return fail(format!("expected at most {}", figure(max)));
                }
            }
            Value::Bool(_) | Value::Null => {}
        }
        Ok(())
    }
}

fn is(value: &Value, kind: Kind) -> bool {
    match kind {
        Kind::Object => value.is_object(),
        Kind::Array => value.is_array(),
        Kind::String => value.is_string(),
        Kind::Number => value.is_number(),
        Kind::Integer => {
            value.is_i64()
                || value.is_u64()
                || value
                    .as_f64()
                    .is_some_and(|x| x.is_finite() && x.fract() == 0.0)
        }
        Kind::Boolean => value.is_boolean(),
        Kind::Null => value.is_null(),
    }
}

/// `.name` for a plain identifier, else `["any key"]`.
pub(super) fn key_path(name: &str) -> String {
    let plain = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if plain {
        format!(".{name}")
    } else {
        format!("[{}]", quoted(name))
    }
}

fn quoted(name: &str) -> String {
    serde_json::to_string(name).unwrap_or_else(|_| format!("\"{name}\""))
}

/// A value in an error, cut short so a long `enum` doesn't swamp it.
fn clip(value: &Value) -> String {
    const MAX: usize = 120;
    let text = value.to_string();
    if text.chars().count() <= MAX {
        return text;
    }
    let head: String = text.chars().take(MAX).collect();
    format!("{head}…")
}

/// A bound as written: `3`, not `3.0`.
fn figure(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        x.to_string()
    }
}
