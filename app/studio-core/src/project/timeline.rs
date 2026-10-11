//! Global time -> (scene, scene-relative time), by cumulative scene length
//! (`Project::scene_length`: the hold, or the narration clip with its lead
//! and tail).

use crate::error::{Result, StudioError};
use crate::render::{Time, last_moment};

#[derive(Clone, Debug)]
pub struct Timeline {
    lengths: Vec<f64>,
}

impl Timeline {
    /// From each scene's length in seconds, in order.
    pub fn new(lengths: impl Iterator<Item = f64>) -> Timeline {
        Timeline {
            lengths: lengths.collect(),
        }
    }

    /// The video's length in seconds.
    pub fn length(&self) -> f64 {
        self.lengths.iter().sum()
    }

    /// Scene `index`'s length in seconds, if it exists.
    pub fn scene_length(&self, index: usize) -> Option<f64> {
        self.lengths.get(index).copied()
    }

    /// When scene `index` starts, in global seconds.
    pub fn start(&self, index: usize) -> f64 {
        self.lengths.iter().take(index).sum()
    }

    /// Scene `index`'s first and last moment in global seconds: the last is
    /// just inside its length, as a transition holds the outgoing scene.
    pub fn window(&self, index: usize) -> Result<(f64, f64)> {
        let length = self.get(index)?;
        let start = self.start(index);
        Ok((start, start + last_moment(length)))
    }

    /// The scene index and scene-relative t for a time; out of range is an
    /// error stating the valid range.
    pub fn locate(&self, at: Time) -> Result<(usize, f64)> {
        match at {
            Time::Global(t) => {
                let length = self.length();
                if !(t.is_finite() && t >= 0.0 && t < length) {
                    return Err(StudioError::TimeOutOfRange { t, length });
                }
                let mut start = 0.0;
                for (i, l) in self.lengths.iter().enumerate() {
                    if t < start + l {
                        return Ok((i, t - start));
                    }
                    start += l;
                }
                Err(StudioError::TimeOutOfRange { t, length })
            }
            Time::Scene { index, t } => {
                let length = self.get(index)?;
                if !(t.is_finite() && t >= 0.0 && t < length) {
                    return Err(StudioError::SceneTimeOutOfRange { index, t, length });
                }
                Ok((index, t))
            }
        }
    }

    fn get(&self, index: usize) -> Result<f64> {
        self.scene_length(index)
            .ok_or(StudioError::SceneOutOfRange {
                index,
                count: self.lengths.len(),
            })
    }
}

#[cfg(test)]
mod tests;
