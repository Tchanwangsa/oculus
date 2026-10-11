//! Laid-out tree -> draw calls. Per node, in order: shadow, fill, then
//! (inside the clip when `clip` is set) image or text and children, then
//! border on top. Opacity and `blur` wrap the node in an isolated group.
//! Transforms (translate, scale, rotate in degrees) apply about the node's
//! centre and are never pixel-snapped.

mod canvas;
mod gradient;

pub use canvas::Canvas;

use crate::layout::LNode;
use crate::scene::{Color, ObjectFit};
use canvas::rounded;
use vello_cpu::kurbo::{Affine, Rect, Vec2};

/// Errors only if a glyph run cannot be drawn.
pub fn paint(c: &mut Canvas, n: &LNode, parent: Affine, text_color: Color) -> Result<(), String> {
    let s = &n.style;
    let opacity = s.opacity();
    if opacity <= 0.0 {
        return Ok(());
    }
    let t = parent * local_transform(n);
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
    // Layers pushed above are popped below even on error: vello refuses
    // to rasterise with a layer still open.
    let mut result = Ok(());
    if let Some((_, layout)) = &n.text {
        let color = s.color.unwrap_or(text_color);
        result = c.text(t, (n.content.x0, n.content.y0), layout, color);
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
    result
}

/// translate(c + d) * rotate * scale * translate(-c), c = the box centre.
fn local_transform(n: &LNode) -> Affine {
    let s = &n.style;
    if !s.has_transform() {
        return Affine::IDENTITY;
    }
    let c = n.rect.center().to_vec2();
    let d = Vec2::new(
        f64::from(s.translate_x.unwrap_or(0.0)),
        f64::from(s.translate_y.unwrap_or(0.0)),
    );
    Affine::translate(c + d)
        * Affine::rotate(f64::from(s.rotate.unwrap_or(0.0)).to_radians())
        * Affine::scale(f64::from(s.scale.unwrap_or(1.0)))
        * Affine::translate(-c)
}
