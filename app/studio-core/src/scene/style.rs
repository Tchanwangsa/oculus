//! The `style` prop: a typed, CSS-like subset. Every key is optional; an
//! unknown key is an error naming the closest valid one.
//!
//! Blur follows CSS: `blur` is `filter: blur()` (the value is the gaussian
//! sigma in px) and `shadow.blur` is the `box-shadow` blur radius
//! (sigma = blur / 2).

use super::color::Color;
use super::values::{
    Align, Border, Dim, Fill, FlexDirection, FlexWrap, Justify, ObjectFit, Position, Shadow, Sides,
    TextAlign, describe,
};
use crate::suggest::did_you_mean;
use serde::Deserialize;
use serde_json::{Map, Value};

macro_rules! style {
    ($($key:literal => $field:ident: $ty:ty),* $(,)?) => {
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct Style {
            $(pub $field: Option<$ty>,)*
        }

        /// Every key `style` accepts.
        pub const STYLE_KEYS: &[&str] = &[$($key),*];

        impl Style {
            fn set(&mut self, key: &str, v: &Value) -> Result<(), String> {
                match key {
                    $($key => {
                        self.$field = Some(<$ty>::deserialize(v).map_err(|e| {
                            format!("style.{key} = {}: {e}", describe(v))
                        })?);
                        Ok(())
                    })*
                    _ => Err(format!(
                        "unknown style key `{key}`{}",
                        did_you_mean(key, STYLE_KEYS.iter().copied())
                    )),
                }
            }
        }
    };
}

style! {
    "width" => width: Dim,
    "height" => height: Dim,
    "minWidth" => min_width: Dim,
    "minHeight" => min_height: Dim,
    "maxWidth" => max_width: Dim,
    "maxHeight" => max_height: Dim,
    "flexDirection" => flex_direction: FlexDirection,
    "flexWrap" => flex_wrap: FlexWrap,
    "flexGrow" => flex_grow: f32,
    "flexShrink" => flex_shrink: f32,
    "gap" => gap: f32,
    "padding" => padding: Sides,
    "margin" => margin: Sides,
    "alignItems" => align_items: Align,
    "alignSelf" => align_self: Align,
    "justifyContent" => justify_content: Justify,
    "position" => position: Position,
    "left" => left: Dim,
    "top" => top: Dim,
    "right" => right: Dim,
    "bottom" => bottom: Dim,
    "fill" => fill: Fill,
    "radius" => radius: f32,
    "border" => border: Border,
    "shadow" => shadow: Shadow,
    "blur" => blur: f32,
    "opacity" => opacity: f32,
    "clip" => clip: bool,
    "translateX" => translate_x: f32,
    "translateY" => translate_y: f32,
    "scale" => scale: f32,
    "rotate" => rotate: f32,
    "fontFamily" => font_family: String,
    "fontSize" => font_size: f32,
    "fontWeight" => font_weight: f32,
    "color" => color: Color,
    "lineHeight" => line_height: f32,
    "letterSpacing" => letter_spacing: f32,
    "textAlign" => text_align: TextAlign,
    "objectFit" => object_fit: ObjectFit,
}

impl Style {
    /// Reads a `style` object, stopping at the first bad key.
    pub fn from_json(obj: &Map<String, Value>) -> Result<Style, String> {
        let mut s = Style::default();
        for (k, v) in obj {
            s.set(k, v)?;
        }
        Ok(s)
    }

    pub fn opacity(&self) -> f32 {
        self.opacity.unwrap_or(1.0).clamp(0.0, 1.0)
    }

    pub fn radius(&self) -> f32 {
        self.radius.unwrap_or(0.0).max(0.0)
    }

    pub fn blur(&self) -> f32 {
        self.blur.unwrap_or(0.0).max(0.0)
    }

    pub fn has_transform(&self) -> bool {
        self.translate_x.unwrap_or(0.0) != 0.0
            || self.translate_y.unwrap_or(0.0) != 0.0
            || self.scale.unwrap_or(1.0) != 1.0
            || self.rotate.unwrap_or(0.0) != 0.0
    }
}

#[cfg(test)]
mod tests;
