//! `<Indicate>`: its targets pulse, as manim's Indicate does: they scale up
//! about their joint centre (to `scale`, 1.2 by default) and flash to its
//! colour (the accent by default), then settle back, over `progress` 0..1
//! along a there-and-back curve. It changes the target nodes' own style
//! (every node whose key the target covers, and what is below them).

use super::targets::covers;
use super::{OverlayError, OverlayInk};
use crate::easing::in_out_cubic;
use crate::layout::LNode;
use crate::scene::{Color, Fill, Origin, OverlayKind, OverlaySpec};
use vello_cpu::kurbo::Rect;

pub fn apply(root: &mut LNode, ink: OverlayInk) -> Result<(), OverlayError> {
    let mut found = Vec::new();
    collect(root, &mut found);
    for (spec, loc) in found {
        let OverlayKind::Indicate { target, scale } = &spec.kind else {
            continue;
        };
        let p = spec.progress.clamp(0.0, 1.0);
        let k = in_out_cubic(1.0 - (2.0 * p - 1.0).abs());
        let mut union: Option<Rect> = None;
        boxes(root, target, &mut union);
        let Some(centre) = union.map(|r| r.center()) else {
            let index = super::targets::Index::new(root);
            return Err(index.missing("Indicate", target, loc));
        };
        if k <= 0.0 {
            continue;
        }
        let pulse = Pulse {
            scale: 1.0 + (scale - 1.0) * k,
            colour: spec.color.unwrap_or(ink.accent),
            k,
            text: ink.text,
            centre: (centre.x, centre.y),
        };
        pulse_all(root, target, &pulse);
    }
    Ok(())
}

fn collect(n: &LNode, out: &mut Vec<(OverlaySpec, Option<(u32, u32)>)>) {
    if let Some(p) = &n.overlay
        && matches!(p.now.kind, OverlayKind::Indicate { .. })
    {
        out.push((p.now.clone(), n.loc));
    }
    for c in &n.children {
        collect(c, out);
    }
}

/// The union of the outermost covered nodes' rects.
fn boxes(n: &LNode, target: &str, out: &mut Option<Rect>) {
    if n.key.as_deref().is_some_and(|k| covers(target, k)) {
        *out = Some(out.map_or(n.rect, |r| r.union(n.rect)));
        return;
    }
    for c in &n.children {
        boxes(c, target, out);
    }
}

struct Pulse {
    scale: f32,
    colour: Color,
    k: f32,
    text: Color,
    centre: (f64, f64),
}

fn pulse_all(n: &mut LNode, target: &str, p: &Pulse) {
    if n.key.as_deref().is_some_and(|k| covers(target, k)) {
        let (w, h) = (n.rect.width(), n.rect.height());
        let s = &mut n.style;
        if s.transform_origin.is_none() && w > 0.0 && h > 0.0 {
            s.transform_origin = Some(Origin {
                x: ((p.centre.0 - n.rect.x0) / w) as f32,
                y: ((p.centre.1 - n.rect.y0) / h) as f32,
            });
        }
        s.scale = Some(s.scale.unwrap_or(1.0) * p.scale);
        tint(n, p);
        return;
    }
    for c in &mut n.children {
        pulse_all(c, target, p);
    }
}

fn towards(c: Color, p: &Pulse) -> Color {
    let l = |a: f32, b: f32| a + (b - a) * p.k;
    Color {
        r: l(c.r, p.colour.r),
        g: l(c.g, p.colour.g),
        b: l(c.b, p.colour.b),
        a: c.a,
    }
}

/// Every colour of a node and its subtree, towards the pulse's.
fn tint(n: &mut LNode, p: &Pulse) {
    let fill = |f: &mut Option<Fill>| {
        if let Some(Fill::Solid(c)) = f {
            *c = towards(*c, p);
        }
    };
    fill(&mut n.style.fill);
    if let Some(b) = &mut n.style.border {
        b.color = towards(b.color, p);
    }
    if n.text.is_some() {
        n.style.color = Some(towards(n.style.color.unwrap_or(p.text), p));
    }
    if let Some(path) = &mut n.path {
        fill(&mut path.fill);
        if let Some(s) = &mut path.stroke {
            s.color = towards(s.color, p);
        }
    }
    for c in &mut n.children {
        tint(c, p);
    }
}
