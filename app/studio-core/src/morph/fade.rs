//! An unmatched node's opacity at its own eased progress `q`, by the
//! scope's fade. Overlap fades out over `q` 0..0.6 and in over 0.4..1, so
//! the two cross while the matched pieces move; sequential fades out over
//! 0..0.4 and in over 0.4..1, never together.

use crate::easing::in_out_cubic;
use crate::scene::Fade;

const OVERLAP_OUT: f32 = 0.6;
const OVERLAP_IN: f32 = 0.4;
const SEQUENTIAL: f32 = 0.4;

fn ramp(q: f32, from: f32, to: f32) -> f32 {
    in_out_cubic(((q - from) / (to - from)).clamp(0.0, 1.0))
}

pub fn leaving(fade: Fade, q: f32) -> f32 {
    let end = match fade {
        Fade::Overlap => OVERLAP_OUT,
        Fade::Sequential => SEQUENTIAL,
    };
    1.0 - ramp(q, 0.0, end)
}

pub fn arriving(fade: Fade, q: f32) -> f32 {
    let start = match fade {
        Fade::Overlap => OVERLAP_IN,
        Fade::Sequential => SEQUENTIAL,
    };
    ramp(q, start, 1.0)
}
