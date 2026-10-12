//! Easings in Rust, for transitions and the morph engine's fades. They
//! match the prelude's (`src/host/prelude/motion.js`) number for number:
//! the same polynomials and the same cubic-bezier solver.

/// A transition's easing: a named polynomial or CSS cubic-bezier points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    OutCubic,
    InOutCubic,
    Bezier([f64; 4]),
}

/// The easings that are prelude functions, by their JS names.
pub const NAMED: &[(&str, Easing)] = &[
    ("linear", Easing::Linear),
    ("easeOutCubic", Easing::OutCubic),
    ("easeInOutCubic", Easing::InOutCubic),
];

impl Easing {
    pub fn apply(self, p: f64) -> f64 {
        match self {
            Easing::Linear => p,
            Easing::OutCubic => 1.0 - (1.0 - p).powi(3),
            Easing::InOutCubic => in_out(p),
            Easing::Bezier([x1, y1, x2, y2]) => cubic_bezier(x1, y1, x2, y2, p),
        }
    }
}

fn in_out(p: f64) -> f64 {
    if p < 0.5 {
        4.0 * p * p * p
    } else {
        1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
    }
}

/// easeInOutCubic on f32, for the morph engine's fades.
pub fn in_out_cubic(p: f32) -> f32 {
    in_out(f64::from(p)) as f32
}

/// CSS cubic-bezier(x1, y1, x2, y2) at `p`: Newton on x(s) = p, falling
/// back to bisection, then y(s). The same steps as the prelude's.
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, p: f64) -> f64 {
    let bx =
        |s: f64| 3.0 * x1 * s * (1.0 - s) * (1.0 - s) + 3.0 * x2 * s * s * (1.0 - s) + s * s * s;
    let by =
        |s: f64| 3.0 * y1 * s * (1.0 - s) * (1.0 - s) + 3.0 * y2 * s * s * (1.0 - s) + s * s * s;
    let dx = |s: f64| {
        3.0 * x1 * (1.0 - s) * (1.0 - s)
            + 6.0 * (x2 - x1) * s * (1.0 - s)
            + 3.0 * (1.0 - x2) * s * s
    };
    if p <= 0.0 {
        return 0.0;
    }
    if p >= 1.0 {
        return 1.0;
    }
    let mut s = p;
    for _ in 0..8 {
        let d = dx(s);
        if d.abs() < 1e-6 {
            break;
        }
        let next = s - (bx(s) - p) / d;
        if !(0.0..=1.0).contains(&next) {
            break;
        }
        s = next;
    }
    if (bx(s) - p).abs() > 1e-5 {
        let (mut lo, mut hi) = (0.0, 1.0);
        s = p;
        for _ in 0..40 {
            if bx(s) < p {
                lo = s;
            } else {
                hi = s;
            }
            s = (lo + hi) / 2.0;
        }
    }
    by(s)
}

#[cfg(test)]
mod tests;
