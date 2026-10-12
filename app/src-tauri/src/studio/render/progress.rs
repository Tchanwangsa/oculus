//! What `render` tells its caller as it goes and at the end.

use std::path::PathBuf;

use serde::Serialize;

use super::preset::Preset;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Its segment was in the cache; nothing rendered.
    Cached,
    /// Rendered and encoded now.
    Rendered,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SegmentReport {
    /// The scene, counting from 0.
    pub scene: usize,
    pub id: Option<String>,
    pub outcome: Outcome,
    pub frames: u64,
    /// Rendering and encoding it; 0 when cached.
    pub ms: u64,
    /// Frames a second while rendering it; `None` when cached.
    pub fps: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// The mp4 written.
    pub path: PathBuf,
    pub preset: Preset,
    /// Output px.
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u64,
    /// Seconds: `frames / fps`.
    pub duration: f64,
    /// Whether it has a narration track.
    pub audio: bool,
    /// Frame renderers.
    pub threads: usize,
    /// In video order.
    pub segments: Vec<SegmentReport>,
    /// Files written beside `path`: the captions as `.vtt`, when the video
    /// is narrated.
    pub sidecars: Vec<PathBuf>,
    /// Cache files deleted afterwards: nothing any preset's last render
    /// uses.
    pub pruned: Vec<String>,
    pub ms: u64,
}

/// Each step as it happens, for a progress line or an app event.
#[derive(Debug)]
pub enum Progress<'a> {
    /// The plan is made: `segments` scenes, `cached` of them in the cache,
    /// `frames` to render on `threads` workers.
    Planned {
        segments: usize,
        cached: usize,
        frames: u64,
        threads: usize,
    },
    /// Segment `n` of `of` (1-based) starts encoding.
    Segment { n: usize, of: usize, scene: usize },
    /// A frame of segment `n` was encoded: `frame` of `frames` (1-based),
    /// at `fps` frames a second over the segment so far.
    Frame {
        n: usize,
        frame: u64,
        frames: u64,
        fps: f64,
    },
    /// A segment is done, cached or rendered.
    Done(&'a SegmentReport),
    /// Mixing the narration and joining the segments.
    Muxing,
}
