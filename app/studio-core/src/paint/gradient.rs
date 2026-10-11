//! CSS `linear-gradient(angle, stops)` over a box, rasterised here rather
//! than by vello: 0 deg points up, 90 right, and the gradient line is just
//! long enough for the box's corners to reach the first and last stops.
//!
//! An 8-bit ramp across a dark range bands (docs/studio.md, "Gradients are
//! dithered"), so the ramp is evaluated in f32 per device pixel and
//! dithered to 8 bits with an 8×8 Bayer matrix fixed to the canvas, so it
//! never shimmers from frame to frame. vello draws the result as an image
//! paint sampled nearest at its own pixel grid, under the shape's
//! anti-aliased coverage.

mod row;

use crate::scene::Color;
use std::sync::Arc;
use vello_cpu::kurbo::{Affine, Point, Rect, Vec2};
use vello_cpu::peniko::ImageAlphaType;
use vello_cpu::{PixelMetadata, Pixmap};

/// Ordered-dither thresholds: entry k is the k-th of 64 levels.
const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// The gradient line through box `r`: where offset 0 and offset 1 sit.
fn line(r: Rect, angle_deg: f32) -> (Point, Vec2) {
    let a = f64::from(angle_deg).to_radians();
    let (dx, dy) = (a.sin(), -a.cos());
    let half = ((r.width() * a.sin()).abs() + (r.height() * a.cos()).abs()) / 2.0;
    let c = r.center();
    let start = Point::new(c.x - dx * half, c.y - dy * half);
    (start, Vec2::new(dx * 2.0 * half, dy * 2.0 * half))
}

/// The ramp for the gradient of box `r` under `t`, covering `shape` (local
/// bounds) clipped to the `canvas`, and the paint transform that puts it
/// back in local units. `None` when nothing of it is on the canvas.
pub fn ramp(
    t: Affine,
    r: Rect,
    angle_deg: f32,
    stops: &[(f32, Color)],
    alpha: f32,
    shape: Rect,
    canvas: (u16, u16),
) -> Option<(Arc<Pixmap>, Affine)> {
    if t.determinant().abs() < 1e-12 || stops.is_empty() {
        return None;
    }
    let full = Rect::new(0.0, 0.0, f64::from(canvas.0), f64::from(canvas.1));
    let device = t.transform_rect_bbox(shape).intersect(full).expand();
    let (x0, y0) = (device.x0.max(0.0) as u32, device.y0.max(0.0) as u32);
    let (x1, y1) = (device.x1.max(0.0) as u32, device.y1.max(0.0) as u32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    // Offset along the line is affine in device px: s = sx·x + sy·y + s0.
    let (start, d) = line(r, angle_deg);
    let len2 = d.hypot2();
    let inv = t.inverse();
    let [a, b, c, dd, e, f] = inv.as_coeffs();
    let (sx, sy, s0) = if len2 > 0.0 {
        (
            (a * d.x + b * d.y) / len2,
            (c * d.x + dd * d.y) / len2,
            ((e - start.x) * d.x + (f - start.y) * d.y) / len2,
        )
    } else {
        (0.0, 0.0, 0.0)
    };
    let stops: Vec<(f64, [f32; 4])> = stops
        .iter()
        .map(|(o, c)| {
            let c = c.with_alpha_mul(alpha);
            let v = [c.r * c.a, c.g * c.a, c.b * c.a, c.a].map(|v| v * 255.0);
            (f64::from(*o), v)
        })
        .collect();
    let mut data = vec![0u8; w as usize * h as usize * 4];
    for (j, px) in data.chunks_exact_mut(w as usize * 4).enumerate() {
        let y = y0 + j as u32;
        let first = sx * (f64::from(x0) + 0.5) + sy * (f64::from(y) + 0.5) + s0;
        let bayer = BAYER[(y & 7) as usize].map(|b| (f32::from(b) + 0.5) / 64.0);
        row::fill(px, &stops, first, sx, &bayer, x0 as usize);
    }
    // Alpha 255 stays 255 whatever the threshold, since the cast saturates.
    let opaque = stops.iter().all(|(_, c)| c[3] >= 255.0);
    let meta = PixelMetadata {
        may_have_transparency: !opaque,
        alpha_type: ImageAlphaType::AlphaPremultiplied,
    };
    let pixmap = Pixmap::from_parts(data, w as u16, h as u16, meta);
    let place = inv * Affine::translate((f64::from(x0), f64::from(y0)));
    Some((Arc::new(pixmap), place))
}

#[cfg(test)]
mod tests;
