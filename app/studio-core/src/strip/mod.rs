//! A contact sheet of a video: `n` frames evenly spaced over `[from, to]`,
//! tiled `cols` wide, each labelled underneath with its time. It is how an
//! agent or a reviewer checks motion without encoding a video. Frames render
//! in parallel, one `Renderer` per thread (`workers`); the sheet and its
//! labels are drawn by studio-core itself (`sheet`, `labels`).

mod labels;
mod sheet;
mod workers;

use crate::error::{Result, StudioError};
use crate::project::Project;
use crate::project::Timeline;
use crate::render::Time;
use crate::render::{MotionBlur, RenderOptions, last_moment, write_png};
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct StripOptions {
    /// Global seconds of the first frame.
    pub from: f64,
    /// Global seconds of the last frame; at or past the video's end it is
    /// clamped to its last moment.
    pub to: f64,
    /// How many frames; 1 draws `from` alone.
    pub n: usize,
    /// Tiles per row.
    pub cols: usize,
    /// Each tile's scale over the video's size.
    pub scale: f64,
    /// Worker threads (default: half the cores, at most `n`), capped so the
    /// renderers stay within the memory budget.
    pub threads: Option<usize>,
    pub motion_blur: Option<MotionBlur>,
}

impl Default for StripOptions {
    /// The whole video, 12 frames, 4 a row, quarter-size tiles.
    fn default() -> Self {
        StripOptions {
            from: 0.0,
            to: f64::INFINITY,
            n: 12,
            cols: 4,
            scale: 0.25,
            threads: None,
            motion_blur: None,
        }
    }
}

/// The sheet as straight-alpha RGBA8, row-major.
#[derive(Clone, Debug)]
pub struct Strip {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Each tile's global time, in tile order.
    pub times: Vec<f64>,
    /// A tile's frame size in px, without its label bar.
    pub tile: (u32, u32),
    /// Tiles per row (`cols`, or `n` when there are fewer frames).
    pub cols: usize,
    /// The worker threads used.
    pub threads: usize,
}

impl Strip {
    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<()> {
        write_png(&self.rgba, self.width, self.height, path.as_ref())
    }
}

/// Renders the frames and tiles them. A frame that fails fails the strip
/// with its time and scene named (`StudioError::Frame`).
pub fn strip(project: &Project, options: StripOptions) -> Result<Strip> {
    let timeline = project.timeline();
    let times = times(&options, timeline.length())?;
    if options.cols == 0 {
        return Err(bad("cols must be at least 1".to_owned()));
    }
    // Refuse an oversized sheet before rendering its frames.
    let side = |v: u32| (f64::from(v) * options.scale).round().max(1.0) as u32;
    let size = project.video.size;
    let tile = (side(size.width), side(size.height));
    sheet::fits(tile, times.len(), options.cols, labels::TALL)?;
    let render = RenderOptions {
        scale: options.scale,
        motion_blur: options.motion_blur,
    };
    let threads = workers::threads(project, &options, times.len());
    let frames = workers::render_all(project, render, &times, threads)?;
    let labels = captions(&timeline, &times, project.video.scenes.len() > 1)?;
    let sheet = sheet::draw(&frames, &labels, options.cols)?;
    Ok(Strip {
        rgba: sheet.rgba,
        width: sheet.width,
        height: sheet.height,
        times,
        tile: sheet.tile,
        cols: sheet.cols,
        threads,
    })
}

fn bad(message: String) -> StudioError {
    StudioError::Render(format!("strip: {message}"))
}

/// `n` times evenly spaced over `[from, to]`, `to` clamped inside the video.
fn times(o: &StripOptions, length: f64) -> Result<Vec<f64>> {
    if o.n == 0 {
        return Err(bad("n must be at least 1".to_owned()));
    }
    if !(o.from.is_finite() && o.from >= 0.0 && o.from < length) {
        return Err(StudioError::TimeOutOfRange { t: o.from, length });
    }
    if o.to.is_nan() || o.to < o.from {
        return Err(bad(format!(
            "to ({} s) is before from ({} s)",
            o.to, o.from
        )));
    }
    let to = o.to.min(last_moment(length)).max(o.from);
    if o.n == 1 {
        return Ok(vec![o.from]);
    }
    let step = (to - o.from) / (o.n - 1) as f64;
    Ok((0..o.n)
        .map(|i| {
            if i + 1 == o.n {
                to
            } else {
                o.from + step * i as f64
            }
        })
        .collect())
}

/// Each tile's label: its global time, and `scene N · t` for a video of
/// more than one scene.
fn captions(timeline: &Timeline, times: &[f64], scenes: bool) -> Result<Vec<labels::Label>> {
    times
        .iter()
        .map(|&t| {
            let (scene, st) = timeline.locate(Time::Global(t))?;
            let (start, end) = (timeline.start(scene), timeline.start(scene + 1));
            Ok(labels::Label {
                time: format!("{} s", seconds(t, end)),
                scene: scenes.then(|| format!("scene {scene} · {} s", seconds(st, end - start))),
            })
        })
        .collect()
}

/// Three decimals, rounded, except that a time just inside `limit` (its
/// scene's end) never reads as `limit` itself.
fn seconds(t: f64, limit: f64) -> String {
    let rounded = format!("{t:.3}");
    match rounded.parse::<f64>() {
        Ok(r) if r >= limit && t < limit => format!("{:.3}", (t * 1000.0).floor() / 1000.0),
        _ => rounded,
    }
}

#[cfg(test)]
mod tests;
