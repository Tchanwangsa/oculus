//! A `Path` element's geometry and paint: SVG path data, the viewBox it is
//! fitted into its box with (`xMidYMid meet`), fill, stroke and trim. Stroke
//! widths are in viewBox units, as in SVG.

use super::color::Color;
use super::values::{Fill, describe};
use serde::Deserialize;
use serde_json::{Map, Value};
use vello_cpu::kurbo::{BezPath, Rect, Shape};

#[derive(Clone, Debug)]
pub struct PathSpec {
    pub d: BezPath,
    /// The path-unit rect fitted into the content box.
    pub view_box: Rect,
    pub fill: Option<Fill>,
    pub stroke: Option<StrokeSpec>,
    /// Start and end as fractions of the total arc length, across subpaths
    /// in order; `None` draws the whole path.
    pub trim: Option<(f64, f64)>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StrokeSpec {
    pub width: f64,
    pub color: Color,
    #[serde(default)]
    pub cap: LineCap,
    #[serde(default)]
    pub join: LineJoin,
    /// Alternating on/off lengths in viewBox units, as SVG `stroke-dasharray`.
    #[serde(default)]
    pub dash: Vec<f64>,
    #[serde(default)]
    pub dash_offset: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// Reads a `path` node's props; the message names the bad prop.
pub fn parse_path(obj: &Map<String, Value>) -> Result<PathSpec, String> {
    let d_src = obj.get("d").and_then(Value::as_str).unwrap_or_default();
    let d = BezPath::from_svg(d_src).map_err(|e| {
        let shown: String = d_src.chars().take(60).collect();
        let more = if d_src.chars().count() > 60 {
            "…"
        } else {
            ""
        };
        format!("d is not SVG path data ({e}): {shown:?}{more}")
    })?;
    let view_box = match obj.get("viewBox") {
        None => d.bounding_box(),
        Some(v) => {
            let b = <[f64; 4]>::deserialize(v)
                .ok()
                .filter(|b| b.iter().all(|x| x.is_finite()) && b[2] >= 0.0 && b[3] >= 0.0)
                .ok_or_else(|| {
                    format!(
                        "viewBox = {}: expected [x, y, width, height] with width and height 0 or more",
                        describe(v)
                    )
                })?;
            Rect::new(b[0], b[1], b[0] + b[2], b[1] + b[3])
        }
    };
    let fill = obj
        .get("fill")
        .map(|v| Fill::deserialize(v).map_err(|e| format!("fill = {}: {e}", describe(v))))
        .transpose()?;
    let stroke = obj
        .get("stroke")
        .map(|v| {
            let s = StrokeSpec::deserialize(v).map_err(|e| {
                format!(
                    "stroke = {}: {e}; a stroke is {{ width, color, cap?, join?, dash?, dashOffset? }}",
                    describe(v)
                )
            })?;
            let ok = |x: f64| x.is_finite() && x >= 0.0;
            if !ok(s.width) || !s.dash.iter().all(|x| ok(*x)) || !s.dash_offset.is_finite() {
                return Err(
                    "stroke width and dash lengths must be finite and 0 or more".to_owned(),
                );
            }
            Ok(s)
        })
        .transpose()?;
    let trim = obj
        .get("trim")
        .map(|v| {
            <[f64; 2]>::deserialize(v)
                .ok()
                .filter(|t| t.iter().all(|x| x.is_finite()))
                .map(|t| (t[0], t[1]))
                .ok_or_else(|| {
                    format!(
                        "trim = {}: expected [start, end], fractions 0 to 1 of the length",
                        describe(v)
                    )
                })
        })
        .transpose()?;
    Ok(PathSpec {
        d,
        view_box,
        fill,
        stroke,
        trim,
    })
}
