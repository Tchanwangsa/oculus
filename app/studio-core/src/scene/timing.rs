//! How a `<Morph>` (or a magic-move transition) spends its `p`: `lag`
//! staggers its pieces in reading order (manim's `lag_ratio`), `fade` says
//! when unmatched pieces fade, and `ease` shapes each piece's own progress.
//! The morph engine applies it (`morph::clock`).

use crate::easing::Easing;
use std::sync::Arc;

/// When unmatched pieces fade, as fractions of their own progress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fade {
    /// Leaving pieces fade out and arriving ones in over overlapping
    /// windows, each riding with its nearest matched sibling's move, as
    /// manim's TransformMatchingTex does.
    #[default]
    Overlap,
    /// Leaving pieces fade out over 0..0.4 and arriving ones in over
    /// 0.4..1, in place, so the two are never visible together.
    Sequential,
}

pub const FADES: &[(&str, Fade)] = &[("overlap", Fade::Overlap), ("sequential", Fade::Sequential)];

/// A piece's progress curve: linear, a named easing, or a table sampled
/// evenly over 0..1 (the prelude samples JS easings, springs included).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Curve {
    #[default]
    Linear,
    Easing(Easing),
    Table(Arc<[f32]>),
}

impl Curve {
    /// Linear passes values outside 0..1 through, so an overshooting `p`
    /// overshoots; the others clamp `u` to their range first.
    pub fn apply(&self, u: f32) -> f32 {
        match self {
            Curve::Linear => u,
            Curve::Easing(e) => e.apply(f64::from(u.clamp(0.0, 1.0))) as f32,
            Curve::Table(t) => sample(t, u),
        }
    }
}

fn sample(t: &[f32], u: f32) -> f32 {
    let n = t.len();
    if n == 0 {
        return u;
    }
    if n == 1 {
        return t[0];
    }
    let x = u.clamp(0.0, 1.0) * (n - 1) as f32;
    let i = (x.floor() as usize).min(n - 2);
    let f = x - i as f32;
    t[i] + (t[i + 1] - t[i]) * f
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timing {
    /// 0..1: how far into one piece's run the next piece starts; 0 moves
    /// everything together.
    pub lag: f32,
    pub fade: Fade,
    pub ease: Curve,
}
