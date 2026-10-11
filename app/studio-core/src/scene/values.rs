//! The value types style keys take: lengths, sides, fills, shadows, borders
//! and the keyword enums. Each rejects what it cannot read, with the forms it
//! accepts in the message.

use super::color::Color;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// A length: px number, `"50%"` or `"auto"`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Dim {
    #[default]
    Auto,
    Px(f32),
    /// Fraction of the parent (0.5 for "50%").
    Pct(f32),
}

impl<'de> Deserialize<'de> for Dim {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let bad = |v: &Value| {
            D::Error::custom(format!(
                "expected a number of px, \"N%\" or \"auto\", got {}",
                describe(v)
            ))
        };
        let v = Value::deserialize(d)?;
        match &v {
            Value::Number(n) => n.as_f64().map(|n| Dim::Px(n as f32)).ok_or_else(|| bad(&v)),
            Value::String(s) if s == "auto" => Ok(Dim::Auto),
            Value::String(s) => s
                .strip_suffix('%')
                .and_then(|p| p.trim().parse::<f32>().ok())
                .map(|p| Dim::Pct(p / 100.0))
                .ok_or_else(|| bad(&v)),
            _ => Err(bad(&v)),
        }
    }
}

/// Padding or margin: `n`, `[all]`, `[vertical, horizontal]` or
/// `[top, right, bottom, left]`, in px.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sides {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl<'de> Deserialize<'de> for Sides {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        let nums: Option<Vec<f32>> = match &v {
            Value::Number(n) => n.as_f64().map(|n| vec![n as f32]),
            Value::Array(a) => a.iter().map(|x| x.as_f64().map(|n| n as f32)).collect(),
            _ => None,
        };
        let s = |t, r, b, l| Sides {
            top: t,
            right: r,
            bottom: b,
            left: l,
        };
        match nums.as_deref() {
            Some([a]) => Ok(s(*a, *a, *a, *a)),
            Some([v, h]) => Ok(s(*v, *h, *v, *h)),
            Some([t, r, b, l]) => Ok(s(*t, *r, *b, *l)),
            _ => Err(D::Error::custom(format!(
                "expected px as n, [all], [vertical, horizontal] or [top, right, bottom, left], got {}",
                describe(&v)
            ))),
        }
    }
}

/// A fill: a colour, or `{ linear: { angle, stops: [[offset, colour], …] } }`
/// with a CSS angle (0 = to top, 90 = to right) and offsets in 0..=1.
#[derive(Clone, Debug, PartialEq)]
pub enum Fill {
    Solid(Color),
    Linear {
        angle_deg: f32,
        stops: Vec<(f32, Color)>,
    },
}

impl<'de> Deserialize<'de> for Fill {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Linear {
            angle: f32,
            stops: Vec<(f32, Color)>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Gradient {
            linear: Linear,
        }
        let v = Value::deserialize(d)?;
        match v {
            Value::String(s) => Color::parse(&s).map(Fill::Solid).map_err(D::Error::custom),
            Value::Object(_) => {
                let g = Gradient::deserialize(v).map_err(|e| {
                    D::Error::custom(format!(
                        "a gradient is {{ linear: {{ angle, stops: [[offset, colour], ...] }} }}: {e}"
                    ))
                })?;
                let stops = g.linear.stops;
                if stops.len() < 2 {
                    return Err(D::Error::custom("a gradient needs at least two stops"));
                }
                let mut last = 0.0;
                for (o, _) in &stops {
                    if !(0.0..=1.0).contains(o) || *o < last {
                        return Err(D::Error::custom(
                            "gradient stop offsets must rise from 0 to 1",
                        ));
                    }
                    last = *o;
                }
                Ok(Fill::Linear {
                    angle_deg: g.linear.angle,
                    stops,
                })
            }
            other => Err(D::Error::custom(format!(
                "expected a colour or {{ linear: {{ angle, stops }} }}, got {}",
                describe(&other)
            ))),
        }
    }
}

/// CSS `box-shadow`: `blur` is the CSS blur radius (gaussian sigma = blur / 2).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Shadow {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub blur: f32,
    #[serde(default)]
    pub spread: f32,
    pub color: Color,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Border {
    pub width: f32,
    pub color: Color,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FlexWrap {
    Nowrap,
    Wrap,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Justify {
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    Relative,
    Absolute,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// `fontVariantNumeric`: with tabular digits every digit has one advance, so
/// a changing number does not jitter.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NumericVariant {
    #[default]
    Normal,
    TabularNums,
}

/// How an image fills its box, as CSS `object-fit`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ObjectFit {
    #[default]
    Fill,
    Cover,
    Contain,
}

/// A short description of a JSON value for error messages.
pub fn describe(v: &Value) -> String {
    match v {
        Value::Null => "null (NaN, Infinity or null in JS)".to_owned(),
        Value::Bool(b) => format!("{b}"),
        Value::Number(n) => format!("{n}"),
        Value::String(s) => format!("{s:?}"),
        Value::Array(_) => "an array".to_owned(),
        Value::Object(_) => "an object".to_owned(),
    }
}
