//! The project's timeline of whole frames at `video.json`'s fps: what an
//! encoder walks. Frame `k` is at global `k / fps`; the last frame is one
//! frame before the video's length, never at it.

use super::{Project, Timeline};
use crate::error::Result;
use crate::render::Time;
use std::ops::Range;

impl Project {
    /// `video.json`'s `fps` (default 30).
    pub fn fps(&self) -> u32 {
        self.video.fps
    }

    /// Every scene's length in order, in whole frames; fails on the first
    /// scene whose length is unknown (see `scene_length`).
    pub fn timeline(&self) -> Result<Timeline> {
        let lengths = (0..self.video.scenes.len())
            .map(|i| self.scene_length(i))
            .collect::<Result<Vec<_>>>()?;
        Ok(Timeline::new(self.fps(), lengths.into_iter()))
    }

    /// Total length in seconds: the frame count over fps.
    pub fn length(&self) -> Result<f64> {
        Ok(self.timeline()?.length())
    }

    /// The video's frame count.
    pub fn frames(&self) -> Result<u64> {
        Ok(self.timeline()?.frames())
    }

    /// The global frame indices scene `index` covers.
    pub fn scene_frames(&self, index: usize) -> Result<Range<u64>> {
        self.timeline()?.scene_frames(index)
    }

    /// The moment frame `k` shows, for `Renderer::frame` (see
    /// `Timeline::frame_time`).
    pub fn frame_time(&self, k: u64) -> Result<Time> {
        self.timeline()?.frame_time(k)
    }
}
