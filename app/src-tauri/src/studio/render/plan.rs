//! The plan: one segment per scene, covering its whole frames, keyed and
//! looked up in the cache; the frames left to render, in order; and where
//! each narration clip lands.

use std::ops::Range;
use std::path::{Path, PathBuf};

use super::audio::Clip;
use super::cache;
use super::key::{segment_key, Trees};
use super::settings::Settings;
use super::workers::Job;

/// What a scene contributes to the plan, from studio-core.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Scene {
    pub index: usize,
    pub id: Option<String>,
    /// Global frame indices.
    pub frames: Range<u64>,
    /// `Project::scene_fingerprint`.
    pub fingerprint: Vec<u8>,
    /// The clip, for a narrated scene.
    pub clip: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Segment {
    pub scene: usize,
    pub id: Option<String>,
    pub frames: Range<u64>,
    pub key: String,
    pub path: PathBuf,
    pub cached: bool,
}

impl Segment {
    pub fn len(&self) -> u64 {
        self.frames.end - self.frames.start
    }
}

/// Each scene's segment, `cached` when its file is in `dir`'s cache.
pub(super) fn segments(
    dir: &Path,
    scenes: &[Scene],
    trees: &Trees,
    settings: &Settings,
) -> Vec<Segment> {
    scenes
        .iter()
        .map(|s| {
            let key = segment_key(&s.fingerprint, trees, settings);
            let path = cache::segment(dir, &key);
            Segment {
                scene: s.index,
                id: s.id.clone(),
                frames: s.frames.clone(),
                cached: path.is_file(),
                key,
                path,
            }
        })
        .collect()
}

/// The frames of every segment not cached, in order. Each samples its
/// scene at `k / fps`, `k` counted from the scene's first frame, so a
/// scene's frames do not depend on where it sits: what makes a segment
/// reusable after an earlier scene changes length.
pub(super) fn jobs(segments: &[Segment], fps: u32) -> Vec<Job> {
    let fps = f64::from(fps);
    segments
        .iter()
        .filter(|s| !s.cached)
        .flat_map(|s| {
            s.frames.clone().map(move |n| Job {
                scene: s.scene,
                t: (n - s.frames.start) as f64 / fps,
                global: n as f64 / fps,
            })
        })
        .collect()
}

/// Each narrated scene's clip at its scene's first frame plus `lead`.
pub(super) fn clips(scenes: &[Scene], fps: u32, lead: f64) -> Vec<Clip> {
    scenes
        .iter()
        .filter_map(|s| {
            let path = s.clip.clone()?;
            Some(Clip {
                path,
                at: s.frames.start as f64 / f64::from(fps) + lead,
            })
        })
        .collect()
}
