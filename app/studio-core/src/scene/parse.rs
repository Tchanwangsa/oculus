//! JSON from the prelude's resolver -> [`Node`]. The prelude already checks
//! element props and `Text` children; this checks styles, `Path` props and
//! the shape.

use super::path::parse_path;
use super::style::Style;
use super::{Kind, Node};
use serde_json::{Map, Value};

/// A bad node: what is wrong, the node's tree path and its TSX position.
#[derive(Debug)]
pub struct NodeError {
    pub message: String,
    pub path: String,
    pub loc: Option<(u32, u32)>,
    /// The error is about an unknown or bad style key.
    pub is_style: bool,
}

pub fn parse_tree(v: &Value) -> Result<Node, NodeError> {
    parse(v, "", None)
}

fn parse(v: &Value, parent: &str, index: Option<usize>) -> Result<Node, NodeError> {
    let fail = |message: String, path: &str, loc, is_style| NodeError {
        message,
        path: path.to_owned(),
        loc,
        is_style,
    };
    let Some(obj) = v.as_object() else {
        return Err(fail(
            "expected an element (Box, Text, Image, Path, Stack or Row)".to_owned(),
            parent,
            None,
            false,
        ));
    };
    let kind = match obj.get("k").and_then(Value::as_str) {
        Some("box") => Kind::Box,
        Some("text") => Kind::Text,
        Some("image") => Kind::Image,
        Some("path") => Kind::Path,
        _ => {
            return Err(fail(
                "expected an element (Box, Text, Image, Path, Stack or Row)".to_owned(),
                parent,
                None,
                false,
            ));
        }
    };
    let path = match index {
        None => kind.name().to_owned(),
        Some(i) => format!("{parent} > {}[{i}]", kind.name()),
    };
    let loc = obj.get("loc").and_then(|l| {
        let a = l.as_array()?;
        Some((a.first()?.as_u64()? as u32, a.get(1)?.as_u64()? as u32))
    });
    let style = match obj.get("style") {
        None => Style::default(),
        Some(Value::Object(m)) => Style::from_json(m).map_err(|e| fail(e, &path, loc, true))?,
        Some(_) => {
            return Err(fail("style must be an object".to_owned(), &path, loc, true));
        }
    };
    let str_field = |m: &Map<String, Value>, k: &str| {
        m.get(k)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_default()
    };
    let path_spec = match kind {
        Kind::Path => Some(Box::new(
            parse_path(obj).map_err(|e| fail(e, &path, loc, false))?,
        )),
        _ => None,
    };
    let mut children = Vec::new();
    if let Some(Value::Array(kids)) = obj.get("children") {
        children.reserve(kids.len());
        for (i, c) in kids.iter().enumerate() {
            children.push(parse(c, &path, Some(i))?);
        }
    }
    Ok(Node {
        kind,
        style,
        text: str_field(obj, "text"),
        src: str_field(obj, "src"),
        path: path_spec,
        children,
        key: obj.get("key").and_then(Value::as_str).map(str::to_owned),
        loc,
    })
}
