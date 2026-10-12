//! Motion blur as temporal supersampling: a frame is the average of N
//! sub-frames spread over the shutter, `t ± shutter / fps / 2`, kept inside
//! the frame's scene so a cut stays a cut. N adapts to the motion: the
//! scene's laid-out layers at the shutter's start and end are compared
//! (`measure`), and `N = clamp(ceil(px), 1, max_samples)` for the largest
//! move of any node in output px: one sub-frame per px, so a fast edge
//! smears instead of doubling. A still frame is N = 1, which is
//! exactly the sharp frame; it costs two extra layouts and no extra paint.
//! Sub-frames average premultiplied (`average`).

mod average;
mod measure;

use super::Frame;
use super::renderer::{Renderer, last_moment};
use crate::error::{Result, StudioError};
use average::Accumulator;

/// The export's shutter. `shutter` is the fraction of a frame interval
/// the shutter is open (0.5 is a 180° shutter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionBlur {
    pub fps: f64,
    pub shutter: f64,
    pub max_samples: u32,
}

impl Default for MotionBlur {
    fn default() -> Self {
        MotionBlur {
            fps: 30.0,
            shutter: 0.5,
            max_samples: 32,
        }
    }
}

/// What a render is for, which the `motionBlur` token decides on: `finals`
/// blurs `Final` renders (the export), `always` both, `off` neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Previews, proxies and drafts.
    Draft,
    /// The export.
    Final,
}

/// Output px of movement per sub-frame; wider spacing shows as copies.
const PX_PER_SAMPLE: f64 = 1.0;

impl MotionBlur {
    pub(crate) fn check(&self) -> Result<()> {
        let bad = |what: String| Err(StudioError::Render(format!("motion blur: {what}")));
        if !(self.fps.is_finite() && self.fps > 0.0) {
            return bad(format!("fps is {}; it must be a positive number", self.fps));
        }
        if !(self.shutter.is_finite() && self.shutter > 0.0 && self.shutter <= 1.0) {
            return bad(format!(
                "shutter is {}; it is the open fraction of a frame, in (0, 1]",
                self.shutter
            ));
        }
        if !(1..=256).contains(&self.max_samples) {
            return bad(format!("max_samples is {}; use 1 to 256", self.max_samples));
        }
        Ok(())
    }

    /// Sub-frames for a move of `px` output px over the shutter.
    fn samples(&self, px: f64) -> u32 {
        let n = (px / PX_PER_SAMPLE).ceil();
        if n.is_finite() && n > 1.0 {
            (n.min(f64::from(self.max_samples))) as u32
        } else {
            1
        }
    }
}

/// Scene `index` at scene-relative `t`, blurred over the shutter.
pub(super) fn frame(r: &mut Renderer, index: usize, t: f64, mb: MotionBlur) -> Result<Frame> {
    let half = mb.shutter / mb.fps / 2.0;
    let t0 = (t - half).max(0.0);
    let t1 = (t + half).min(last_moment(r.hold(index)));
    let n = if t1 > t0 {
        mb.samples(moved(r, index, t0, t1)?)
    } else {
        1
    };
    if n == 1 {
        return r.sharp(index, t);
    }
    let mut acc: Option<Accumulator> = None;
    let mut logs = Vec::new();
    for i in 0..n {
        let at = t0 + (f64::from(i) + 0.5) / f64::from(n) * (t1 - t0);
        let f = r.sharp(index, at)?;
        if i == n / 2 {
            logs = f.logs;
        }
        let acc = acc.get_or_insert_with(|| Accumulator::new(f.width, f.height));
        acc.add(&f.rgba);
    }
    let acc = acc.ok_or_else(|| StudioError::Render("motion blur drew no sub-frames".into()))?;
    Ok(acc.finish(logs, n))
}

/// The largest move of any node between `t0` and `t1`, in output px.
fn moved(r: &mut Renderer, index: usize, t0: f64, t1: f64) -> Result<f64> {
    let a = r.layers(index, t0)?;
    let b = match r.layers(index, t1) {
        Ok(b) => b,
        Err(e) => {
            r.engine.discard(a);
            return Err(e);
        }
    };
    let px = measure::displacement(&a, &b) * r.engine.scale();
    r.engine.discard(a);
    r.engine.discard(b);
    Ok(px)
}

#[cfg(test)]
mod tests;
