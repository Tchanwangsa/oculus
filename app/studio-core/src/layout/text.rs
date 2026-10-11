//! Text shaping with parley over the bundled fonts. `quantize` is off so
//! glyph positions and line metrics stay fractional (text that glides must
//! not step). Optical size follows the font size, as CSS
//! `font-optical-sizing: auto` does (Inter has an `opsz` axis; fonts
//! without one ignore it). Shaped layouts are pooled across frames by content and style:
//! a frame takes from what the previous frame returned, and anything not
//! reused for a frame is dropped, so the pool stays bounded.

use crate::fonts::font_context;
use crate::scene::{NumericVariant, Style, TextAlign};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontFamilyName, FontWeight, Layout,
    LayoutContext, LineHeight, StyleProperty,
};
use std::collections::HashMap;

pub type TextLayout = Layout<()>;

/// Everything shaping depends on (floats as bits, so it hashes).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct TextKey {
    text: String,
    family: String,
    size: u32,
    weight: u32,
    line_height: u32,
    letter_spacing: u32,
    tabular: bool,
}

impl TextKey {
    pub fn new(text: &str, family: &str, s: &Style) -> TextKey {
        TextKey {
            text: text.to_owned(),
            family: family.to_owned(),
            size: font_size(s).to_bits(),
            weight: s.font_weight.unwrap_or(400.0).to_bits(),
            line_height: s.line_height.unwrap_or(1.25).to_bits(),
            letter_spacing: s.letter_spacing.unwrap_or(0.0).to_bits(),
            tabular: s.font_variant_numeric == Some(NumericVariant::TabularNums),
        }
    }

    /// The string that was shaped.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The same string shaped between `a` and `b`: size, weight, line
    /// height and letter spacing interpolated. `None` when the two differ in
    /// anything that cannot be (the string, the family, tabular digits).
    pub fn between(a: &TextKey, b: &TextKey, p: f32) -> Option<TextKey> {
        if a.text != b.text || a.family != b.family || a.tabular != b.tabular {
            return None;
        }
        let mix = |x: u32, y: u32| {
            if x == y {
                return y;
            }
            let (x, y) = (f32::from_bits(x), f32::from_bits(y));
            (x + (y - x) * p).to_bits()
        };
        Some(TextKey {
            text: b.text.clone(),
            family: b.family.clone(),
            size: mix(a.size, b.size),
            weight: mix(a.weight, b.weight),
            line_height: mix(a.line_height, b.line_height),
            letter_spacing: mix(a.letter_spacing, b.letter_spacing),
            tabular: b.tabular,
        })
    }
}

fn font_size(s: &Style) -> f32 {
    s.font_size.unwrap_or(16.0).max(0.0)
}

pub struct Typeset {
    font_cx: FontContext,
    layout_cx: LayoutContext<()>,
    pub families: Vec<String>,
    previous: HashMap<TextKey, Vec<TextLayout>>,
    current: HashMap<TextKey, Vec<TextLayout>>,
}

impl Typeset {
    pub fn new() -> Typeset {
        let (font_cx, families) = font_context();
        Typeset {
            font_cx,
            layout_cx: LayoutContext::new(),
            families,
            previous: HashMap::new(),
            current: HashMap::new(),
        }
    }

    /// A shaped (not yet line-broken) layout for the key.
    pub fn take(&mut self, key: &TextKey) -> TextLayout {
        if let Some(l) = self.previous.get_mut(key).and_then(Vec::pop) {
            return l;
        }
        if let Some(l) = self.current.get_mut(key).and_then(Vec::pop) {
            return l;
        }
        let size = f32::from_bits(key.size);
        let optical = format!("\"opsz\" {size}");
        let mut b = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, &key.text, 1.0, false);
        b.push_default(StyleProperty::FontVariations(optical.as_str().into()));
        b.push_default(StyleProperty::FontFamily(FontFamily::Single(
            FontFamilyName::Named(key.family.as_str().into()),
        )));
        b.push_default(StyleProperty::FontSize(size));
        b.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from_bits(
            key.weight,
        ))));
        b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            f32::from_bits(key.line_height),
        )));
        b.push_default(StyleProperty::LetterSpacing(f32::from_bits(
            key.letter_spacing,
        )));
        if key.tabular {
            b.push_default(StyleProperty::FontFeatures("\"tnum\" 1".into()));
        }
        b.build(&key.text)
    }

    /// Hands a layout back after its frame is painted.
    pub fn give_back(&mut self, key: TextKey, layout: TextLayout) {
        self.current.entry(key).or_default().push(layout);
    }

    /// Ends a frame: what this frame returned is what the next may reuse.
    pub fn end_frame(&mut self) {
        self.previous = std::mem::take(&mut self.current);
    }
}

/// Breaks lines at the content width and aligns them in it.
pub fn finish(layout: &mut TextLayout, width: f32, align: Option<TextAlign>) {
    // A hair of slack: the box was sized from this layout's own width, and
    // float noise must not wrap the last word.
    layout.break_all_lines(Some(width + 1e-3));
    let alignment = match align.unwrap_or_default() {
        TextAlign::Left => Alignment::Left,
        TextAlign::Center => Alignment::Center,
        TextAlign::Right => Alignment::Right,
        TextAlign::Justify => Alignment::Justify,
    };
    layout.align(alignment, AlignmentOptions::default());
}
