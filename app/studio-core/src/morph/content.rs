//! A matched pair's content: text, image or path. The same text (string,
//! family and line count) re-shapes at the interpolated size and width, the
//! same image or path data morphs its geometry, and anything else swaps in
//! halves: A's content fades out over p 0..0.5, B's in over 0.5..1, so the
//! two are never visible together. Halves draw as child nodes of the pair.

use super::blend::{fill, lerp64, rect};
use super::colour::{MorphColor, mix};
use crate::easing::in_out_cubic;
use crate::layout::{LNode, TextKey, Typeset, finish};
use crate::scene::{Kind, PathSpec, StrokeSpec, Style, TextAlign};
use std::sync::Arc;
use vello_cpu::kurbo::{Rect, Vec2};

/// Moves the content of `a` and `b` into `out` (whose rects and style are
/// already blended); text layouts that are not used go back to the pool.
pub fn blend(
    out: &mut LNode,
    a: &mut LNode,
    b: &mut LNode,
    p: f32,
    ts: &mut Typeset,
    c: MorphColor,
) {
    if let (Some((ka, la)), Some((kb, lb))) = (&a.text, &b.text)
        && la.len() == lb.len()
    {
        let width = out.content.width() as f32;
        let key = if ka == kb {
            None
        } else {
            TextKey::between(ka, kb, p)
        };
        if ka == kb || key.is_some() {
            let (Some((ka, la)), Some((kb, mut lb))) = (a.text.take(), b.text.take()) else {
                return;
            };
            ts.give_back(ka, la);
            match key {
                None => {
                    if (out.content.width() - b.content.width()).abs() > 1e-9 {
                        finish(&mut lb, width, out.style.text_align);
                    }
                    out.text = Some((kb, lb));
                }
                Some(key) => {
                    ts.give_back(kb, lb);
                    let mut layout = ts.take(&key);
                    finish(&mut layout, width, out.style.text_align);
                    out.text = Some((key, layout));
                }
            }
            return;
        }
    }
    if let (Some(x), Some(y)) = (&a.image, &b.image)
        && Arc::ptr_eq(x, y)
    {
        out.image = b.image.take();
        return;
    }
    if let (Some(x), Some(y)) = (&a.path, &b.path)
        && x.d.elements() == y.d.elements()
    {
        out.path = Some(Box::new(path(x, y, p, c)));
        return;
    }
    let fa = 1.0 - in_out_cubic((p / 0.5).clamp(0.0, 1.0));
    let fb = in_out_cubic(((p - 0.5) / 0.5).clamp(0.0, 1.0));
    let halves: Vec<LNode> = [(a, fa), (b, fb)]
        .into_iter()
        .filter_map(|(side, alpha)| half(out.content, side, alpha, ts))
        .collect();
    out.children.splice(0..0, halves);
}

/// One side's content as a child of the pair at `alpha`, or `None` when it
/// has none or is invisible.
fn half(at: Rect, s: &mut LNode, alpha: f32, ts: &mut Typeset) -> Option<LNode> {
    if alpha <= 0.0 {
        if let Some((k, l)) = s.text.take() {
            ts.give_back(k, l);
        }
        return None;
    }
    let mut style = Style {
        opacity: (alpha < 1.0).then_some(alpha),
        ..Style::default()
    };
    let (kind, r) = if s.text.is_some() {
        // The text keeps its own lines, placed in the morphing box as its
        // alignment would place it.
        let k = match s.style.text_align.unwrap_or_default() {
            TextAlign::Center => 0.5,
            TextAlign::Right => 1.0,
            TextAlign::Left | TextAlign::Justify => 0.0,
        };
        style.color = s.style.color;
        style.reveal = s.style.reveal;
        let dx = k * (at.width() - s.content.width());
        let origin = at.origin() + Vec2::new(dx, 0.0);
        (Kind::Text, Rect::from_origin_size(origin, s.content.size()))
    } else if s.image.is_some() {
        style.object_fit = s.style.object_fit;
        style.radius = s.style.radius;
        (Kind::Image, at)
    } else if s.path.is_some() {
        (Kind::Path, at)
    } else {
        return None;
    };
    Some(LNode {
        kind,
        style,
        rect: r,
        content: r,
        text: s.text.take(),
        image: s.image.take(),
        path: s.path.take(),
        children: Vec::new(),
        key: None,
        loc: None,
        scope: None,
    })
}

/// The same path data: viewBox, fill, stroke and trim morph.
fn path(a: &PathSpec, b: &PathSpec, p: f32, c: MorphColor) -> PathSpec {
    let trim = if a.trim == b.trim {
        b.trim
    } else {
        let ((s0, e0), (s1, e1)) = (a.trim.unwrap_or((0.0, 1.0)), b.trim.unwrap_or((0.0, 1.0)));
        Some((lerp64(s0, s1, p), lerp64(e0, e1, p)))
    };
    PathSpec {
        d: b.d.clone(),
        view_box: rect(a.view_box, b.view_box, p),
        fill: fill(a.fill.as_ref(), b.fill.as_ref(), p, c),
        stroke: stroke(a.stroke.as_ref(), b.stroke.as_ref(), p, c),
        trim,
    }
}

/// A stroke that appears or goes grows from or thins to zero width.
fn stroke(
    a: Option<&StrokeSpec>,
    b: Option<&StrokeSpec>,
    p: f32,
    c: MorphColor,
) -> Option<StrokeSpec> {
    if a == b {
        return b.cloned();
    }
    let (x, y) = match (a, b) {
        (Some(x), Some(y)) => (x.clone(), y.clone()),
        (Some(x), None) => (
            x.clone(),
            StrokeSpec {
                width: 0.0,
                ..x.clone()
            },
        ),
        (None, Some(y)) => (
            StrokeSpec {
                width: 0.0,
                ..y.clone()
            },
            y.clone(),
        ),
        (None, None) => return None,
    };
    let dash = if x.dash.len() == y.dash.len() {
        x.dash
            .iter()
            .zip(&y.dash)
            .map(|(u, v)| lerp64(*u, *v, p))
            .collect()
    } else {
        y.dash.clone()
    };
    Some(StrokeSpec {
        width: lerp64(x.width, y.width, p),
        color: mix(x.color, y.color, p, c),
        dash,
        dash_offset: lerp64(x.dash_offset, y.dash_offset, p),
        ..y
    })
}
