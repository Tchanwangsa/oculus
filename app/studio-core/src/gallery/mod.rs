//! A gallery: one component's `examples` entries, each rendered as a still
//! on its own, the way the component is checked by eye. Each example is a
//! call with its props, at one moment; the examples render in parallel, one
//! `Engine` per thread, and the stills are put back in declaration order, so
//! the output does not depend on the thread count.
//!
//! The moment is the middle of the first scene that uses the component (its
//! length is `ctx.duration`), or 1 s of a 2 s preview when no scene uses it.
//! A gallery renders without narration, so `word()` finds no cues.

mod workers;

use crate::error::{Result, StudioError};
use crate::project::{ComponentSource, Project};
use crate::render::{Frame, Size};
use crate::strip::labels::Label;
use crate::strip::sheet;
use crate::theme::Theme;
use std::path::{Path, PathBuf};

/// Every example in a gallery renders at the same moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moment {
    /// Seconds from the start of the component's scene.
    pub t: f64,
    /// The scene's length in seconds (`ctx.duration`).
    pub duration: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct GalleryOptions {
    /// Output scale over the video's size (1.0 is full size).
    pub scale: f64,
    /// Render threads (default: half the cores, at most one per example,
    /// capped to the memory budget).
    pub threads: Option<usize>,
}

impl Default for GalleryOptions {
    fn default() -> Self {
        GalleryOptions {
            scale: 1.0,
            threads: None,
        }
    }
}

/// One example's still.
#[derive(Debug)]
pub struct Still {
    pub example: String,
    pub frame: Frame,
}

#[derive(Debug)]
pub struct Gallery {
    pub component: String,
    pub moment: Moment,
    /// In the order `examples` declares them.
    pub stills: Vec<Still>,
    /// The worker threads used.
    pub threads: usize,
}

impl Gallery {
    /// The stills tiled `cols` wide, each labelled with its example's name,
    /// written as a PNG.
    pub fn write_sheet(&self, path: impl AsRef<Path>, cols: usize) -> Result<()> {
        let frames: Vec<Frame> = self.stills.iter().map(|s| s.frame.clone()).collect();
        let labels: Vec<Label> = self
            .stills
            .iter()
            .map(|s| Label {
                time: s.example.clone(),
                scene: None,
            })
            .collect();
        let sheet = sheet::draw(&frames, &labels, cols)?;
        crate::render::write_png(&sheet.rgba, sheet.width, sheet.height, path.as_ref())
    }
}

/// The gallery of a video's component: its library cascade and theme, the
/// video's size and assets, and the moment from its first scene. `t`
/// overrides the moment (seconds from the scene's start, or from the start
/// of the 2 s preview).
pub fn render(
    project: &Project,
    component: &str,
    t: Option<f64>,
    options: &GalleryOptions,
) -> Result<Gallery> {
    let timeline = project.timeline()?;
    let scene = project
        .video
        .scenes
        .iter()
        .position(|s| s.component == component);
    let default = match scene {
        Some(i) => {
            let duration = timeline.start(i + 1) - timeline.start(i);
            Moment {
                t: duration / 2.0,
                duration,
            }
        }
        None => Moment {
            t: 1.0,
            duration: 2.0,
        },
    };
    let moment = Moment {
        t: t.unwrap_or(default.t),
        duration: default.duration,
    };
    render_source(
        project.components.as_ref(),
        &project.theme,
        project.video.size,
        Some(project.assets_dir()),
        component,
        moment,
        options,
    )
}

/// A gallery from any component source (a video's cascade, or the starter
/// set in tests), at `size` and `moment`.
pub fn render_source(
    components: &dyn ComponentSource,
    theme: &Theme,
    size: Size,
    assets: Option<PathBuf>,
    component: &str,
    moment: Moment,
    options: &GalleryOptions,
) -> Result<Gallery> {
    if !(moment.t.is_finite() && moment.t >= 0.0 && moment.t < moment.duration) {
        return Err(StudioError::Render(format!(
            "gallery: t = {} s is outside the component's {} s; t must be in [0, {})",
            moment.t, moment.duration, moment.duration
        )));
    }
    let tsx = components.read(component)?;
    let examples = examples_of(component, &tsx, theme, size, assets.clone(), options)?;
    let threads = workers::threads(size, options, examples.len());
    let inputs = workers::Inputs {
        name: component,
        tsx: &tsx,
        theme,
        size,
        assets,
        scale: options.scale,
        moment,
    };
    let frames = workers::render_all(&inputs, &examples, threads)?;
    let stills = examples
        .into_iter()
        .zip(frames)
        .map(|((example, _), frame)| Still { example, frame })
        .collect();
    Ok(Gallery {
        component: component.to_owned(),
        moment,
        stills,
        threads,
    })
}

/// The component's `examples` in declaration order, each an object of
/// props. The schema check happens when an example renders.
fn examples_of(
    component: &str,
    tsx: &str,
    theme: &Theme,
    size: Size,
    assets: Option<PathBuf>,
    options: &GalleryOptions,
) -> Result<Vec<(String, serde_json::Map<String, serde_json::Value>)>> {
    let mut engine = crate::render::Engine::new(
        theme.clone(),
        size,
        assets,
        crate::render::RenderOptions {
            scale: options.scale,
            motion_blur: None,
        },
    )?;
    let info = engine.load_component(component, tsx)?;
    let mut out = Vec::with_capacity(info.examples.len());
    for (name, value) in &info.examples {
        let serde_json::Value::Object(props) = value else {
            return Err(StudioError::Render(format!(
                "{component}: examples.{name} must be an object of props, got {}",
                crate::scene::describe(value)
            )));
        };
        out.push((name.clone(), props.clone()));
    }
    if out.is_empty() {
        return Err(StudioError::Render(format!(
            "{component} has no `examples`; add one to render a gallery"
        )));
    }
    Ok(out)
}
