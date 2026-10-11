//! Laid-out tree -> draw calls. Per node, in order: shadow, fill, then
//! (inside the clip when `clip` is set) image, path or text and children,
//! then border on top. Opacity and `blur` wrap the node in an isolated group,
//! and `clipInset` clips all of it. Transforms (translate, scale, rotate in
//! degrees) apply about `transformOrigin` (default the centre) and are never
//! pixel-snapped.

mod canvas;
mod gradient;
mod path;
mod text;

pub use canvas::Canvas;

use crate::layout::LNode;
use crate::scene::{Color, Inset, ObjectFit};
use canvas::rounded;
use vello_cpu::kurbo::{Affine, Rect, Shape, Vec2};

/// Errors only if a glyph run cannot be drawn.
pub fn paint(c: &mut Canvas, n: &LNode, parent: Affine, text_color: Color) -> Result<(), String> {
    let s = &n.style;
    let opacity = s.opacity();
    if opacity <= 0.0 {
        return Ok(());
    }
    let t = parent * local_transform(n);
    let inset = match s.clip_inset {
        Some(i) => match inset_rect(n.rect, i) {
            Some(r) => Some(r),
            None => return Ok(()),
        },
        None => None,
    };
    if let Some(r) = inset {
        c.push_clip(t, &r.to_path(0.1));
    }
    let blur = s.blur();
    let group = opacity < 1.0 || blur > 0.0;
    if group {
        c.push_group(t, opacity, blur);
    }
    let radius = f64::from(s.radius());
    if let Some(sh) = &s.shadow {
        let spread = f64::from(sh.spread);
        let r = n.rect.inflate(spread, spread) + Vec2::new(f64::from(sh.x), f64::from(sh.y));
        c.shadow(
            t,
            r,
            (radius + spread).max(0.0),
            sh.blur.max(0.0) / 2.0,
            sh.color,
        );
    }
    if let Some(f) = &s.fill {
        c.fill(t, n.rect, radius, f, 1.0);
    }
    let clip = s.clip == Some(true);
    if clip {
        c.push_clip(t, &rounded(n.rect, radius));
    }
    if let Some(img) = &n.image {
        let (iw, ih) = (f64::from(img.width()), f64::from(img.height()));
        let r = n.content;
        let (sx, sy) = (r.width() / iw.max(1.0), r.height() / ih.max(1.0));
        let (sx, sy) = match s.object_fit.unwrap_or_default() {
            ObjectFit::Fill => (sx, sy),
            ObjectFit::Cover => (sx.max(sy), sx.max(sy)),
            ObjectFit::Contain => (sx.min(sy), sx.min(sy)),
        };
        let placed = Rect::from_center_size(r.center(), (iw * sx, ih * sy));
        let shape = rounded(placed.intersect(r), radius);
        let place =
            Affine::translate(placed.origin().to_vec2()) * Affine::scale_non_uniform(sx, sy);
        c.image(t, img, place, &shape, 1.0);
    }
    if let Some(p) = &n.path {
        path::draw(c, t, n.content, p);
    }
    // Layers pushed above are popped below even on error: vello refuses
    // to rasterise with a layer still open.
    let mut result = Ok(());
    if let Some((key, layout)) = &n.text {
        let color = s.color.unwrap_or(text_color);
        let at = t * Affine::translate((n.content.x0, n.content.y0));
        result = text::draw(c, at, layout, key.text(), color, s.reveal.as_ref());
    }
    for child in &n.children {
        if result.is_err() {
            break;
        }
        result = paint(c, child, t, text_color);
    }
    if clip {
        c.pop();
    }
    if let Some(b) = &s.border
        && b.width > 0.0
    {
        c.stroke(t, n.rect, radius, f64::from(b.width), b.color);
    }
    if group {
        c.pop();
    }
    if inset.is_some() {
        c.pop();
    }
    result
}

/// The border box less `clipInset`'s fractions; a side at 0 does not clip
/// (shadows and overflow show there). `None` when nothing is left.
fn inset_rect(r: Rect, Inset([t, right, b, l]): Inset) -> Option<Rect> {
    const OPEN: f64 = 1e5;
    let side = |f: f32| f64::from(f.clamp(0.0, 1.0));
    let (w, h) = (r.width(), r.height());
    let cut = Rect::new(
        if l > 0.0 {
            r.x0 + w * side(l)
        } else {
            r.x0 - OPEN
        },
        if t > 0.0 {
            r.y0 + h * side(t)
        } else {
            r.y0 - OPEN
        },
        if right > 0.0 {
            r.x1 - w * side(right)
        } else {
            r.x1 + OPEN
        },
        if b > 0.0 {
            r.y1 - h * side(b)
        } else {
            r.y1 + OPEN
        },
    );
    (cut.x1 > cut.x0 && cut.y1 > cut.y0).then_some(cut)
}

/// translate(o + d) * rotate * scale * translate(-o), o = the transform
/// origin (the box centre unless `transformOrigin` says otherwise).
fn local_transform(n: &LNode) -> Affine {
    let s = &n.style;
    if !s.has_transform() {
        return Affine::IDENTITY;
    }
    let c = match s.transform_origin {
        None => n.rect.center().to_vec2(),
        Some(o) => Vec2::new(
            n.rect.x0 + n.rect.width() * f64::from(o.x),
            n.rect.y0 + n.rect.height() * f64::from(o.y),
        ),
    };
    let d = Vec2::new(
        f64::from(s.translate_x.unwrap_or(0.0)),
        f64::from(s.translate_y.unwrap_or(0.0)),
    );
    Affine::translate(c + d)
        * Affine::rotate(f64::from(s.rotate.unwrap_or(0.0)).to_radians())
        * Affine::scale(f64::from(s.scale.unwrap_or(1.0)))
        * Affine::translate(-c)
}
