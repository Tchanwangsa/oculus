//! A moment's caption as an element tree, laid out and painted like a
//! component's: a column of lines on a rounded plate, each word its own
//! `Text` in a padded box so the highlight can move between words without
//! the line reflowing.

use serde_json::{Value, json};

use super::phrase::Phrase;
use super::style::{CaptionStyle, Highlight, Placement};
use super::timing::Moment;
use crate::project::CuedWord;
use crate::scene::Color;

/// Line height, in em.
const LINE_HEIGHT: f64 = 1.25;
/// Each word's padding (the highlight pill's), in em: vertical, horizontal.
/// The horizontal side is under 40% of Inter's 0.28 em space, so a pill
/// stays clear of its neighbours' letters.
const WORD_PAD: (f64, f64) = (0.06, 0.1);
/// The gap between word boxes, in em: with both paddings, words sit 0.32 em
/// apart, a little wider than a space.
const WORD_GAP: f64 = 0.12;
/// The plate's padding beyond the words', in em: vertical, horizontal.
const PLATE_PAD: (f64, f64) = (0.22, 0.34);
/// The widest the plate gets, as a fraction of the frame's width.
const MAX_WIDTH: f64 = 0.9;

/// The tree for `moment` of a scene whose words are `words`, on a frame of
/// `width` x `height` layout px.
pub fn tree(
    style: &CaptionStyle,
    phrases: &[Phrase],
    words: &[CuedWord],
    moment: Moment,
    (width, height): (u32, u32),
) -> Value {
    let em = style.font_size * CaptionStyle::unit(width, height);
    let phrase = &phrases[moment.phrase];
    let lines: Vec<Value> = phrase
        .lines
        .iter()
        .map(|line| {
            let words: Vec<Value> = line
                .clone()
                .map(|i| word(style, &words[i].text, i, moment, em))
                .collect();
            json!({
                "k": "box",
                "style": {
                    "flexDirection": "row",
                    "justifyContent": "center",
                    "gap": em * WORD_GAP,
                },
                "children": words,
            })
        })
        .collect();
    let mut plate = json!({
        "flexDirection": "column",
        "alignItems": "center",
        "maxWidth": f64::from(width) * MAX_WIDTH,
        "padding": [em * PLATE_PAD.0, em * PLATE_PAD.1],
        "radius": em * 0.4,
    });
    if let Some(fill) = style.plate {
        plate["fill"] = hex(fill).into();
    }
    let edge = style.margin * f64::from(height);
    let (justify, pad) = match style.placement {
        Placement::Bottom => ("flex-end", [0.0, 0.0, edge, 0.0]),
        Placement::Top => ("flex-start", [edge, 0.0, 0.0, 0.0]),
    };
    json!({
        "k": "box",
        "style": {
            "flexDirection": "column",
            "alignItems": "center",
            "justifyContent": justify,
            "padding": pad,
        },
        "children": [{ "k": "box", "style": plate, "children": lines }],
    })
}

/// Word `i` at `moment`: said, being said, or still to come.
fn word(style: &CaptionStyle, text: &str, i: usize, moment: Moment, em: f64) -> Value {
    let current = moment.current == Some(i);
    let (colour, fill) = match (current, style.highlight_style) {
        (true, Highlight::Color) => (style.highlight, None),
        (true, Highlight::Pill) => (style.highlight_text, Some(style.highlight)),
        (false, _) => (style.color, None),
    };
    let mut text_style = json!({
        "fontFamily": style.font_family,
        "fontSize": em,
        "fontWeight": style.font_weight,
        "lineHeight": LINE_HEIGHT,
        "color": hex(colour),
    });
    if !current && i >= moment.said {
        text_style["opacity"] = style.upcoming.into();
    }
    let mut boxed = json!({
        "flexShrink": 0,
        "padding": [em * WORD_PAD.0, em * WORD_PAD.1],
        "radius": em * 0.3,
    });
    if let Some(fill) = fill {
        boxed["fill"] = hex(fill).into();
    }
    json!({
        "k": "box",
        "style": boxed,
        "children": [{ "k": "text", "text": text, "style": text_style }],
    })
}

/// `#rrggbbaa`.
fn hex(c: Color) -> String {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", b(c.r), b(c.g), b(c.b), b(c.a))
}
