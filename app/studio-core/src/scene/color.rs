//! Colours: `#rgb`, `#rrggbb` or `#rrggbbaa`, nothing else. Theme tokens are
//! plain values from `theme`, resolved in JS before they get here.

use serde::{Deserialize, Deserializer};
use vello_cpu::color::{AlphaColor, Srgb};

/// Straight (not premultiplied) sRGB colour, components in 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub fn parse(s: &str) -> Result<Color, String> {
        let bad = || {
            format!(
                "`{s}` is not a colour; use #rgb, #rrggbb or #rrggbbaa (or a theme.color token)"
            )
        };
        let hex = s.strip_prefix('#').ok_or_else(bad)?;
        let digits: Vec<u8> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|d| d as u8))
            .collect::<Option<_>>()
            .ok_or_else(bad)?;
        let pair = |i: usize| digits[i] * 16 + digits[i + 1];
        let (r, g, b, a) = match digits.len() {
            3 => (digits[0] * 17, digits[1] * 17, digits[2] * 17, 255),
            6 => (pair(0), pair(2), pair(4), 255),
            8 => (pair(0), pair(2), pair(4), pair(6)),
            _ => return Err(bad()),
        };
        Ok(Color {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: f32::from(a) / 255.0,
        })
    }

    pub fn with_alpha_mul(self, k: f32) -> Color {
        Color {
            a: self.a * k,
            ..self
        }
    }

    pub fn to_vello(self) -> AlphaColor<Srgb> {
        AlphaColor::new([self.r, self.g, self.b, self.a])
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).map_err(serde::de::Error::custom)
    }
}
