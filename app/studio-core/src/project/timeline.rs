//! Global time -> (scene, scene-relative time), by cumulative `hold`.

use crate::error::{Result, StudioError};
use crate::render::{Time, last_moment};

#[derive(Clone, Debug)]
pub struct Timeline {
    holds: Vec<f64>,
}

impl Timeline {
    pub fn new(holds: impl Iterator<Item = f64>) -> Timeline {
        Timeline {
            holds: holds.collect(),
        }
    }

    pub fn length(&self) -> f64 {
        self.holds.iter().sum()
    }

    /// When scene `index` starts, in global seconds.
    pub fn start(&self, index: usize) -> f64 {
        self.holds.iter().take(index).sum()
    }

    /// Scene `index`'s first and last moment in global seconds: the last is
    /// just inside its hold, as a transition holds the outgoing scene.
    pub fn window(&self, index: usize) -> Result<(f64, f64)> {
        let Some(&hold) = self.holds.get(index) else {
            return Err(StudioError::SceneOutOfRange {
                index,
                count: self.holds.len(),
            });
        };
        let start = self.start(index);
        Ok((start, start + last_moment(hold)))
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
                for (i, hold) in self.holds.iter().enumerate() {
                    if t < start + hold {
                        return Ok((i, t - start));
                    }
                    start += hold;
                }
                Err(StudioError::TimeOutOfRange { t, length })
            }
            Time::Scene { index, t } => {
                let Some(&hold) = self.holds.get(index) else {
                    return Err(StudioError::SceneOutOfRange {
                        index,
                        count: self.holds.len(),
                    });
                };
                if !(t.is_finite() && t >= 0.0 && t < hold) {
                    return Err(StudioError::SceneTimeOutOfRange { index, t, hold });
                }
                Ok((index, t))
            }
        }
    }
}

#[cfg(test)]
mod tests;
