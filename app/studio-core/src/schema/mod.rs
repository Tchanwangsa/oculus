//! A component's `props` export, read into Rust. The prelude's builders
//! (`str()`, `num()`, `bool()`, `list(x)`, `oneOf([...])`, `token(kind)`,
//! `latex()`, each with `.default(v)` and `.optional()`) serialise to the
//! JSON this reads. The inspector UI and lint are generated from it.

mod validate;

pub use validate::validate_props;

use serde::Deserialize;
use serde::ser::{Serialize, SerializeMap, Serializer};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum PropKind {
    Str,
    Num,
    Bool,
    List(Box<PropSpec>),
    OneOf(Vec<Value>),
    /// A token name from a theme section (`color`, `space`, `motion.spring`…);
    /// the component receives the token's value.
    Token(String),
    /// LaTeX source, for `<Math tex>`; checked by typesetting it, so a
    /// bad formula is a props error with KaTeX's message and position.
    Latex,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropSpec {
    pub kind: PropKind,
    pub optional: bool,
    pub default: Option<Value>,
}

/// Serialises to the JSON the builder wrote (`kind`, `optional`, `default`,
/// and `of`, `values` or `token` by kind), so a client reads the schema in
/// the shape the component exported it.
impl Serialize for PropSpec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        let kind = match &self.kind {
            PropKind::Str => "str",
            PropKind::Num => "num",
            PropKind::Bool => "bool",
            PropKind::List(_) => "list",
            PropKind::OneOf(_) => "oneOf",
            PropKind::Token(_) => "token",
            PropKind::Latex => "latex",
        };
        map.serialize_entry("kind", kind)?;
        map.serialize_entry("optional", &self.optional)?;
        if let Some(default) = &self.default {
            map.serialize_entry("default", default)?;
        }
        match &self.kind {
            PropKind::List(of) => map.serialize_entry("of", of)?,
            PropKind::OneOf(values) => map.serialize_entry("values", values)?,
            PropKind::Token(token) => map.serialize_entry("token", token)?,
            PropKind::Str | PropKind::Num | PropKind::Bool | PropKind::Latex => {}
        }
        map.end()
    }
}

/// The JSON one builder serialises to.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpec {
    kind: String,
    #[serde(default)]
    optional: bool,
    default: Option<Value>,
    of: Option<Box<RawSpec>>,
    values: Option<Vec<Value>>,
    token: Option<String>,
}

/// Props in declaration order.
pub type PropsSchema = Vec<(String, PropSpec)>;

/// Reads the serialised `props` export. Errors name the prop.
pub fn parse_schema(v: &Value) -> Result<PropsSchema, String> {
    let Some(obj) = v.as_object() else {
        return Err(
            "`export const props` must be an object of prop builders, e.g. { title: str() }"
                .to_owned(),
        );
    };
    obj.iter()
        .map(|(name, spec)| {
            let raw = RawSpec::deserialize(spec).map_err(|_| {
                format!(
                    "props.{name} is not a prop builder; use str(), num(), bool(), list(x), oneOf([...]), token(kind) or latex()"
                )
            })?;
            Ok((name.clone(), convert(raw).map_err(|e| format!("props.{name}: {e}"))?))
        })
        .collect()
}

fn convert(raw: RawSpec) -> Result<PropSpec, String> {
    let kind = match raw.kind.as_str() {
        "str" => PropKind::Str,
        "num" => PropKind::Num,
        "bool" => PropKind::Bool,
        "latex" => PropKind::Latex,
        "list" => PropKind::List(Box::new(convert(
            *raw.of.ok_or("list(x) needs an item builder")?,
        )?)),
        "oneOf" => {
            let values = raw.values.ok_or("oneOf needs an array of values")?;
            if values.is_empty() {
                return Err("oneOf([...]) needs at least one value".to_owned());
            }
            PropKind::OneOf(values)
        }
        "token" => PropKind::Token(raw.token.ok_or("token(kind) needs a kind")?),
        other => return Err(format!("unknown prop kind `{other}`")),
    };
    Ok(PropSpec {
        kind,
        optional: raw.optional,
        default: raw.default,
    })
}

impl PropKind {
    /// How the kind reads in an error, e.g. "a list of strings".
    pub fn describe(&self) -> String {
        match self {
            PropKind::Str => "a string".to_owned(),
            PropKind::Num => "a finite number".to_owned(),
            PropKind::Bool => "true or false".to_owned(),
            PropKind::Latex => "a LaTeX string".to_owned(),
            PropKind::List(of) => format!("a list of ({})", of.kind.describe()),
            PropKind::OneOf(vs) => format!(
                "one of {}",
                vs.iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            PropKind::Token(kind) => format!("a theme.{kind} token name"),
        }
    }
}

#[cfg(test)]
mod tests;
