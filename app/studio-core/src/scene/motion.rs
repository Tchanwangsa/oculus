//! Value types of the motion style keys: `transformOrigin`, `clipInset` and
//! Text's `reveal`. Keywords that are misspelt name the closest valid one.

use super::values::describe;
use crate::suggest::did_you_mean;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// The point transforms apply about, as fractions of the border box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Origin {
    pub x: f32,
    pub y: f32,
}

const ORIGIN_WORDS: &[&str] = &["center", "top", "bottom", "left", "right"];

impl<'de> Deserialize<'de> for Origin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        let bad = || {
            D::Error::custom(format!(
                "expected [x, y] fractions of the box or keywords like \"center\" or \"top left\", got {}",
                describe(&v)
            ))
        };
        match &v {
            Value::Array(a) => match a.as_slice() {
                [x, y] => Ok(Origin {
                    x: x.as_f64().ok_or_else(bad)? as f32,
                    y: y.as_f64().ok_or_else(bad)? as f32,
                }),
                _ => Err(bad()),
            },
            Value::String(s) => {
                let mut o = Origin { x: 0.5, y: 0.5 };
                let words: Vec<&str> = s.split_whitespace().collect();
                if words.is_empty() || words.len() > 2 {
                    return Err(bad());
                }
                for w in words {
                    match w {
                        "center" => {}
                        "left" => o.x = 0.0,
                        "right" => o.x = 1.0,
                        "top" => o.y = 0.0,
                        "bottom" => o.y = 1.0,
                        _ => {
                            return Err(D::Error::custom(format!(
                                "unknown transformOrigin keyword `{w}`{}; use {}",
                                did_you_mean(w, ORIGIN_WORDS.iter().copied()),
                                ORIGIN_WORDS.join(", ")
                            )));
                        }
                    }
                }
                Ok(o)
            }
            _ => Err(bad()),
        }
    }
}

/// `clipInset: [top, right, bottom, left]`, fractions 0..1 of the border box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inset(pub [f32; 4]);

impl<'de> Deserialize<'de> for Inset {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(d)?;
        let nums: Option<Vec<f32>> = v
            .as_array()
            .and_then(|a| a.iter().map(|x| x.as_f64().map(|n| n as f32)).collect());
        match nums.as_deref() {
            Some(&[t, r, b, l]) => Ok(Inset([t, r, b, l])),
            _ => Err(D::Error::custom(format!(
                "expected [top, right, bottom, left] fractions 0 to 1 of the box, got {}",
                describe(&v)
            ))),
        }
    }
}

/// Text's `reveal`: units appear one after another as `progress` goes 0 to
/// 1. Paint only; layout and wrapping are those of the whole text.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Reveal {
    #[serde(default)]
    pub by: RevealBy,
    pub progress: f32,
    /// 0 reveals every unit together, 1 strictly one after another.
    #[serde(default = "half")]
    pub stagger: f32,
    #[serde(default)]
    pub from: RevealFrom,
}

fn half() -> f32 {
    0.5
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RevealBy {
    Char,
    #[default]
    Word,
    Line,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RevealFrom {
    #[default]
    Fade,
    Rise,
    Blur,
}

/// A keyword from a fixed list, with a did-you-mean on a miss.
fn keyword<'de, D: Deserializer<'de>, T: Copy>(
    d: D,
    what: &str,
    options: &[(&'static str, T)],
) -> Result<T, D::Error> {
    let s = String::deserialize(d)?;
    let names = || options.iter().map(|(n, _)| *n);
    options
        .iter()
        .find(|(n, _)| *n == s)
        .map(|(_, v)| *v)
        .ok_or_else(|| {
            D::Error::custom(format!(
                "unknown {what} `{s}`{}; use {}",
                did_you_mean(&s, names()),
                names().collect::<Vec<_>>().join(", ")
            ))
        })
}

impl<'de> Deserialize<'de> for RevealBy {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        keyword(
            d,
            "reveal.by",
            &[
                ("char", RevealBy::Char),
                ("word", RevealBy::Word),
                ("line", RevealBy::Line),
            ],
        )
    }
}

impl<'de> Deserialize<'de> for RevealFrom {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        keyword(
            d,
            "reveal.from",
            &[
                ("fade", RevealFrom::Fade),
                ("rise", RevealFrom::Rise),
                ("blur", RevealFrom::Blur),
            ],
        )
    }
}
