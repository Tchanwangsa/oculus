//! A typeset formula -> the Box that replaces its `Math` node. The Box is
//! the formula's size (its width by height plus depth, at `fontSize` ×
//! `KATEX_SCALE` px per em) and holds every piece absolutely placed. Its
//! first child is an empty, in-flow strut as tall as the formula's height,
//! so the Box's baseline (in a Row with `alignItems: "baseline"`) is the
//! formula's. A Math's style is a Box's plus `fontSize` and `color`; the
//! other text keys are ignored, so a `theme.type` token can be spread in,
//! and keys that size or pad the Box are refused.

use super::expand::MathEnv;
use super::{Formula, KATEX_SCALE, Shape};
use super::{colour, write};
use crate::scene::{
    Align, Dim, Fill, Kind, Node, PathSpec, Position, StrokeSpec, Style, WriteSpec,
};
use vello_cpu::kurbo::Rect;

/// Text's default size, when a Math sets no `fontSize`.
const DEFAULT_FONT_SIZE: f32 = 16.0;

/// The style keys a Math refuses, by name, and whether the style has them.
fn refused(s: &Style) -> Option<&'static str> {
    let keys: [(&str, bool); 16] = [
        ("width", s.width.is_some()),
        ("height", s.height.is_some()),
        ("minWidth", s.min_width.is_some()),
        ("minHeight", s.min_height.is_some()),
        ("maxWidth", s.max_width.is_some()),
        ("maxHeight", s.max_height.is_some()),
        ("padding", s.padding.is_some()),
        ("border", s.border.is_some()),
        ("flexDirection", s.flex_direction.is_some()),
        ("flexWrap", s.flex_wrap.is_some()),
        ("gap", s.gap.is_some()),
        ("alignItems", s.align_items.is_some()),
        ("justifyContent", s.justify_content.is_some()),
        ("textAlign", s.text_align.is_some()),
        ("reveal", s.reveal.is_some()),
        ("objectFit", s.object_fit.is_some()),
    ];
    keys.into_iter().find(|(_, set)| *set).map(|(k, _)| k)
}

/// The Box, or a message and whether it is about the style.
pub fn node(
    math: &Node,
    f: &Formula,
    prefix: Option<&str>,
    env: &MathEnv,
    written: Option<&WriteSpec>,
) -> Result<Node, (String, bool)> {
    if let Some(key) = refused(&math.style) {
        return Err((
            format!(
                "style.{key}: a <Math> is the size of its formula and takes no {key}; put it in a Box to size, pad or align it"
            ),
            true,
        ));
    }
    let em = f64::from(math.style.font_size.unwrap_or(DEFAULT_FONT_SIZE).max(0.0)) * KATEX_SCALE;
    let ink = math.style.color.unwrap_or(env.text);
    let px = |v: f64| Dim::Px((v * em) as f32);
    let mut style = Style {
        font_family: None,
        font_size: None,
        font_weight: None,
        color: None,
        line_height: None,
        letter_spacing: None,
        font_variant_numeric: None,
        ..math.style.clone()
    };
    style.width = Some(px(f.width));
    style.height = Some(px(f.height + f.depth));
    let mut children = Vec::with_capacity(f.items.len() + 1);
    children.push(leaf(
        Kind::Box,
        Style {
            width: Some(Dim::Px(0.0)),
            height: Some(px(f.height.max(0.0))),
            align_self: Some(Align::FlexStart),
            flex_shrink: Some(0.0),
            ..Style::default()
        },
        None,
        math.loc,
    ));
    let progress = written
        .map(|w| write::plan(f, w))
        .transpose()
        .map_err(|m| (m, false))?;
    for (i, item) in f.items.iter().enumerate() {
        let colour = match &item.colour {
            Some(c) => colour::resolve(c, env.colours).map_err(|m| (m, false))?,
            None => ink,
        };
        if item.colour.is_some() && colour.a <= 0.0 {
            continue;
        }
        let r = item.rect;
        let mut s = Style {
            position: Some(Position::Absolute),
            left: Some(px(r.x0)),
            top: Some(px(r.y0 + f.height)),
            width: Some(px(r.width())),
            height: Some(px(r.height())),
            ..Style::default()
        };
        let key = prefix.map(|p| format!("{p}/{}", item.key));
        let mut n = match &item.shape {
            Shape::Rule => {
                s.fill = Some(Fill::Solid(colour));
                leaf(Kind::Box, s, key, math.loc)
            }
            Shape::Glyph(g) => {
                let mut n = leaf(Kind::Path, s, key, math.loc);
                n.path = Some(Box::new(PathSpec {
                    d: (*g.outline).clone(),
                    view_box: g.view,
                    fill: Some(Fill::Solid(colour)),
                    stroke: None,
                    trim: None,
                }));
                n
            }
            Shape::Path { d, stroke } => {
                let mut n = leaf(Kind::Path, s, key, math.loc);
                n.path = Some(Box::new(PathSpec {
                    d: d.clone(),
                    view_box: Rect::from_origin_size((0.0, 0.0), r.size()),
                    fill: stroke.is_none().then_some(Fill::Solid(colour)),
                    stroke: stroke.map(|width| StrokeSpec {
                        width,
                        color: colour,
                        cap: Default::default(),
                        join: Default::default(),
                        dash: Vec::new(),
                        dash_offset: 0.0,
                    }),
                    trim: None,
                }));
                n
            }
        };
        n.loc = math.loc;
        if let (Some(u), Some(w)) = (progress.as_ref().map(|p| p[i]), written) {
            write::apply(&mut n, &item.shape, colour, u, w.stroke);
        }
        children.push(n);
    }
    let mut out = leaf(Kind::Box, style, math.key.clone(), math.loc);
    out.children = children;
    Ok(out)
}

fn leaf(kind: Kind, style: Style, key: Option<String>, loc: Option<(u32, u32)>) -> Node {
    Node {
        kind,
        style,
        text: String::new(),
        src: String::new(),
        path: None,
        math: None,
        overlay: None,
        children: Vec::new(),
        key,
        loc,
        p: 0.0,
        timing: None,
        scope: None,
    }
}
