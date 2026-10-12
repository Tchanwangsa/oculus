//! An overlay element as the prelude writes it (`{ k: "overlay", kind, … }`):
//! what it draws, what it is bound to, and how far it has drawn on. Lengths
//! are px; `None` takes a default scaled to the target's maths size.

use super::{Color, describe};
use serde::Deserialize;
use serde_json::{Map, Value};
use vello_cpu::kurbo::Point;

#[derive(Clone, Debug, PartialEq)]
pub struct OverlaySpec {
    pub kind: OverlayKind,
    /// 0..1: how far it has drawn on (Indicate: how far through its pulse).
    pub progress: f32,
    pub color: Option<Color>,
    /// Stroke width (Surround, Underline, Arrow).
    pub width: Option<f64>,
}

/// A laid-out overlay: its spec and, while a morph blends two of them,
/// the A side's spec and how far from it (`p`).
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub now: OverlaySpec,
    pub was: Option<(OverlaySpec, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Below,
    Above,
    Left,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum End {
    Key(String),
    Point(Point),
}

#[derive(Clone, Debug, PartialEq)]
pub enum OverlayKind {
    Brace {
        target: String,
        side: Side,
        /// Between the target and the brace.
        gap: Option<f64>,
        /// The brace's em, which sets its thickness.
        size: Option<f64>,
    },
    Surround {
        target: String,
        pad: Option<f64>,
        radius: Option<f64>,
    },
    Underline {
        target: String,
        gap: Option<f64>,
    },
    Indicate {
        target: String,
        scale: f32,
    },
    Arrow {
        from: End,
        to: End,
        /// Degrees the shaft turns through, an arc; 0 is straight.
        bend: f64,
        head: Option<f64>,
        /// Between a keyed end and its target's box.
        gap: Option<f64>,
    },
}

impl OverlaySpec {
    pub fn name(&self) -> &'static str {
        match self.kind {
            OverlayKind::Brace { .. } => "Brace",
            OverlayKind::Surround { .. } => "Surround",
            OverlayKind::Underline { .. } => "Underline",
            OverlayKind::Indicate { .. } => "Indicate",
            OverlayKind::Arrow { .. } => "Arrow",
        }
    }
}

pub fn parse(obj: &Map<String, Value>) -> Result<OverlaySpec, String> {
    let kind_name = obj.get("kind").and_then(Value::as_str).unwrap_or_default();
    let num = |k: &str| -> Result<Option<f64>, String> {
        match obj.get(k) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_f64()
                .filter(|x| x.is_finite())
                .map(Some)
                .ok_or_else(|| format!("{k} = {}: expected a finite number", describe(v))),
        }
    };
    let target = || -> Result<String, String> {
        obj.get("target")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| {
                "needs target=\"key\" (a key or a key prefix such as \"eq/lhs\")".to_owned()
            })
    };
    let end = |k: &str| -> Result<End, String> {
        match obj.get(k) {
            Some(Value::String(s)) if !s.is_empty() => Ok(End::Key(s.clone())),
            Some(v @ Value::Array(_)) => <[f64; 2]>::deserialize(v)
                .ok()
                .filter(|p| p.iter().all(|x| x.is_finite()))
                .map(|p| End::Point(Point::new(p[0], p[1])))
                .ok_or_else(|| format!("{k} = {}: expected a key or [x, y]", describe(v))),
            v => Err(format!(
                "{k} = {}: expected a key or [x, y]",
                v.map_or("nothing".to_owned(), describe)
            )),
        }
    };
    let kind = match kind_name {
        "brace" => OverlayKind::Brace {
            target: target()?,
            side: match obj.get("side").and_then(Value::as_str).unwrap_or("below") {
                "below" => Side::Below,
                "above" => Side::Above,
                "left" => Side::Left,
                "right" => Side::Right,
                s => return Err(format!("side = {s:?}: use below, above, left or right")),
            },
            gap: num("gap")?,
            size: num("size")?,
        },
        "surround" => OverlayKind::Surround {
            target: target()?,
            pad: num("pad")?,
            radius: num("radius")?,
        },
        "underline" => OverlayKind::Underline {
            target: target()?,
            gap: num("gap")?,
        },
        "indicate" => OverlayKind::Indicate {
            target: target()?,
            scale: num("scale")?.unwrap_or(1.2) as f32,
        },
        "arrow" => OverlayKind::Arrow {
            from: end("from")?,
            to: end("to")?,
            bend: num("bend")?.unwrap_or(0.0),
            head: num("head")?,
            gap: num("gap")?,
        },
        k => return Err(format!("unknown overlay kind {k:?}")),
    };
    let color = match obj.get("color") {
        None | Some(Value::Null) => None,
        Some(v) => {
            Some(Color::deserialize(v).map_err(|e| format!("color = {}: {e}", describe(v)))?)
        }
    };
    Ok(OverlaySpec {
        kind,
        progress: num("progress")?.unwrap_or(1.0) as f32,
        color,
        width: num("width")?,
    })
}
