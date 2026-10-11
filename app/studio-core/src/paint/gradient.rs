//! CSS `linear-gradient(angle, stops)` -> a vello gradient over a box: 0 deg
//! points up, 90 right, and the gradient line is just long enough for the
//! box's corners to reach the first and last stops.

use crate::scene::Color;
use vello_cpu::kurbo::Rect;
use vello_cpu::peniko::{ColorStop, ColorStops, Extend, Gradient, LinearGradientPosition};

pub fn linear(r: Rect, angle_deg: f32, stops: &[(f32, Color)], alpha: f32) -> Gradient {
    let a = f64::from(angle_deg).to_radians();
    let (dx, dy) = (a.sin(), -a.cos());
    let half = ((r.width() * a.sin()).abs() + (r.height() * a.cos()).abs()) / 2.0;
    let c = r.center();
    let stops: Vec<ColorStop> = stops
        .iter()
        .map(|(o, col)| ColorStop::from((*o, col.with_alpha_mul(alpha).to_vello())))
        .collect();
    Gradient {
        kind: LinearGradientPosition {
            start: (c.x - dx * half, c.y - dy * half).into(),
            end: (c.x + dx * half, c.y + dy * half).into(),
        }
        .into(),
        stops: ColorStops::from(stops.as_slice()),
        extend: Extend::Pad,
        ..Gradient::default()
    }
}
