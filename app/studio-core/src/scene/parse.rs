//! JSON from the prelude's resolver -> [`Node`]. The prelude already checks
//! element props and `Text` children; this checks styles, `Path` props and
//! the shape.

use super::overlay;
use super::path::parse_path;
use super::style::Style;
use super::{Curve, FADES, Kind, MathSpec, Node, Timing, WriteSpec};
use serde_json::{Map, Value};
use std::sync::Arc;

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
            "expected an element (Box, Text, Image, Path, Math, Stack or Row)".to_owned(),
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
        Some("morph") => Kind::Morph,
        Some("math") => Kind::Math,
        Some("overlay") => Kind::Overlay,
        _ => {
            return Err(fail(
                "expected an element (Box, Text, Image, Path, Math, Stack or Row)".to_owned(),
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
    let overlay = match kind {
        Kind::Overlay => Some(Box::new(overlay::parse(obj).map_err(|e| {
            let name = obj.get("kind").and_then(Value::as_str).unwrap_or("overlay");
            fail(format!("<{}>: {e}", overlay_name(name)), &path, loc, false)
        })?)),
        _ => None,
    };
    let timing = match kind {
        Kind::Morph => Some(Box::new(
            timing(obj).map_err(|e| fail(e, &path, loc, false))?,
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
        math: match kind {
            Kind::Math => Some(Box::new(MathSpec {
                tex: str_field(obj, "tex"),
                display: obj.get("display").and_then(Value::as_bool).unwrap_or(true),
                write: obj
                    .get("write")
                    .map(write)
                    .transpose()
                    .map_err(|e| fail(e, &path, loc, false))?,
            })),
            _ => None,
        },
        overlay,
        children,
        key: obj.get("key").and_then(Value::as_str).map(str::to_owned),
        loc,
        p: obj
            .get("p")
            .and_then(Value::as_f64)
            .filter(|p| p.is_finite())
            .unwrap_or(0.0) as f32,
        timing,
        scope: None,
    })
}

/// A Morph's `lag`, `fade` and `ease` (a table the prelude sampled).
fn timing(obj: &Map<String, Value>) -> Result<Timing, String> {
    let mut t = Timing::default();
    if let Some(v) = obj.get("lag") {
        t.lag = v
            .as_f64()
            .filter(|l| (0.0..=1.0).contains(l))
            .ok_or_else(|| format!("lag = {v}: expected a number from 0 to 1"))?
            as f32;
    }
    if let Some(v) = obj.get("fade") {
        let word = v.as_str().unwrap_or_default();
        t.fade = FADES
            .iter()
            .find(|(n, _)| *n == word)
            .map(|(_, f)| *f)
            .ok_or_else(|| format!("fade = {v}: expected \"overlap\" or \"sequential\""))?;
    }
    if let Some(v) = obj.get("ease") {
        let table: Option<Vec<f32>> = v.as_array().and_then(|a| {
            a.iter()
                .map(|x| x.as_f64().filter(|x| x.is_finite()).map(|x| x as f32))
                .collect()
        });
        match table {
            Some(t2) if t2.len() >= 2 => t.ease = Curve::Table(Arc::from(t2)),
            _ => return Err(format!("ease = {v}: expected a sampled easing")),
        }
    }
    Ok(t)
}

/// The prelude's `<Write>` spec: `{ groups: [[name | null, p], …], lag?,
/// stroke }`.
fn write(v: &Value) -> Result<WriteSpec, String> {
    let bad = || format!("<Write> on a Math: bad spec {v}");
    let groups = v
        .get("groups")
        .and_then(Value::as_array)
        .ok_or_else(bad)?
        .iter()
        .map(|g| {
            let pair = g.as_array().filter(|a| a.len() == 2).ok_or_else(bad)?;
            let name = match &pair[0] {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                _ => return Err(bad()),
            };
            let p = pair[1].as_f64().filter(|p| p.is_finite()).ok_or_else(bad)?;
            Ok((name, p as f32))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let lag = match v.get("lag") {
        None | Some(Value::Null) => None,
        Some(l) => Some(
            l.as_f64()
                .filter(|l| (0.0..=1.0).contains(l))
                .ok_or_else(|| format!("<Write> lag = {l}: expected a number from 0 to 1"))?
                as f32,
        ),
    };
    let stroke = v
        .get("stroke")
        .and_then(Value::as_f64)
        .filter(|s| s.is_finite() && *s >= 0.0)
        .ok_or_else(bad)?;
    Ok(WriteSpec {
        groups,
        lag,
        stroke,
    })
}

/// `brace` -> `Brace`, as the prelude names the element.
fn overlay_name(kind: &str) -> String {
    let mut c = kind.chars();
    c.next()
        .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
        .unwrap_or_default()
}
