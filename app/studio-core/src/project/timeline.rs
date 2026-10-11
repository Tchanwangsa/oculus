//! Global time -> (scene, scene-relative time), on a timeline of whole
//! frames. Each scene's length (`Project::scene_length`: the hold, or the
//! narration clip with its lead and tail) rounds up to whole frames at the
//! video's fps, so scene `i` covers frames `scene_frames(i)` and its frames
//! sample scene-relative times `k / fps` wherever the scene sits.

use crate::error::{Result, StudioError};
use crate::render::{Time, last_moment};
use std::ops::Range;

#[derive(Clone, Debug)]
pub struct Timeline {
    fps: u32,
    /// Each scene's first frame, then the video's frame count.
    starts: Vec<u64>,
}

/// Seconds rounded up to whole frames at `fps`, at least one. A length
/// within a millionth of a frame of a whole one is that frame count, so
/// `0.1 * 30` is 3 frames, not 4.
pub(crate) fn whole_frames(seconds: f64, fps: u32) -> u64 {
    let frames = (seconds * f64::from(fps) - 1e-6).ceil();
    if frames.is_finite() && frames > 1.0 {
        frames as u64
    } else {
        1
    }
}

impl Timeline {
    /// From the video's fps and each scene's length in seconds, in order;
    /// each length rounds up to whole frames.
    pub fn new(fps: u32, lengths: impl Iterator<Item = f64>) -> Timeline {
        let mut starts = vec![0];
        let mut at = 0;
        for length in lengths {
            at += whole_frames(length, fps);
            starts.push(at);
        }
        Timeline { fps, starts }
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    /// The video's frame count.
    pub fn frames(&self) -> u64 {
        self.starts.last().copied().unwrap_or(0)
    }

    /// The frames scene `index` covers, as global frame indices.
    pub fn scene_frames(&self, index: usize) -> Result<Range<u64>> {
        self.get(index)?;
        Ok(self.starts[index]..self.starts[index + 1])
    }

    /// Frame `k`'s moment: scene-relative `j / fps` for the scene's `j`th
    /// frame, so a scene's frames are the same times wherever it sits.
    /// `Renderer::frame(Time::Global(k / fps))` lands on the same moment.
    pub fn frame_time(&self, k: u64) -> Result<Time> {
        let frames = self.frames();
        if k >= frames {
            return Err(StudioError::FrameOutOfRange {
                frame: k,
                frames,
                fps: self.fps,
            });
        }
        let index = self.scene_of_frame(k);
        Ok(Time::Scene {
            index,
            t: self.seconds(k - self.starts[index]),
        })
    }

    /// The video's length in seconds: its frame count over fps.
    pub fn length(&self) -> f64 {
        self.seconds(self.frames())
    }

    /// Scene `index`'s length in seconds (whole frames), if it exists.
    pub fn scene_length(&self, index: usize) -> Option<f64> {
        let range = self.scene_frames(index).ok()?;
        Some(self.seconds(range.end - range.start))
    }

    /// When scene `index` starts, in global seconds; past the last scene,
    /// the video's length.
    pub fn start(&self, index: usize) -> f64 {
        let last = self.starts.len() - 1;
        self.seconds(self.starts[index.min(last)])
    }

    /// Scene `index`'s first and last moment in global seconds: the last is
    /// just inside its length, as a transition holds the outgoing scene.
    pub fn window(&self, index: usize) -> Result<(f64, f64)> {
        let length = self.get(index)?;
        let start = self.start(index);
        Ok((start, start + last_moment(length)))
    }

    /// The scene index and scene-relative t for a time; out of range is an
    /// error stating the valid range. A global time on the frame grid
    /// (`k / fps`) gives exactly `frame_time(k)`.
    pub fn locate(&self, at: Time) -> Result<(usize, f64)> {
        match at {
            Time::Global(t) => {
                let length = self.length();
                if !(t.is_finite() && t >= 0.0 && t < length) {
                    return Err(StudioError::TimeOutOfRange { t, length });
                }
                let k = (t * f64::from(self.fps)).round();
                if self.seconds(k as u64) == t {
                    let index = self.scene_of_frame(k as u64);
                    return Ok((index, self.seconds(k as u64 - self.starts[index])));
                }
                let scenes = self.starts.len() - 1;
                let index = (0..scenes)
                    .find(|&i| t < self.seconds(self.starts[i + 1]))
                    .ok_or(StudioError::TimeOutOfRange { t, length })?;
                Ok((index, t - self.seconds(self.starts[index])))
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

    fn seconds(&self, frames: u64) -> f64 {
        frames as f64 / f64::from(self.fps)
    }

    /// The scene holding frame `k`, which is inside the video.
    fn scene_of_frame(&self, k: u64) -> usize {
        self.starts.partition_point(|&s| s <= k) - 1
    }

    fn get(&self, index: usize) -> Result<f64> {
        let count = self.starts.len() - 1;
        if index >= count {
            return Err(StudioError::SceneOutOfRange { index, count });
        }
        Ok(self.seconds(self.starts[index + 1] - self.starts[index]))
    }
}

#[cfg(test)]
mod tests;
