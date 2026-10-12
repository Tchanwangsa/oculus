//! The project-free renderer: component source + props + theme + size + t
//! -> frame. `Renderer` drives it for a video; the Components playground
//! and `gallery` drive it for one component.

mod prepare;
mod transition;

use super::{Frame, RenderOptions, Size};
use crate::error::{Result, StudioError};
use crate::host::{ComponentInfo, Host};
use crate::images::ImageCache;
use crate::layout::{LNode, Typeset};
use crate::math::MathCache;
use crate::morph::{Ink, MorphColor, duplicate_key};
use crate::paint::{Canvas, paint};
use crate::project::WordCues;
use crate::schema::validate_props;
use crate::theme::Theme;
use crate::transition::{MotionSpec, morph_color};
use serde_json::{Map, Value};
use std::path::PathBuf;
use vello_cpu::kurbo::Affine;

/// One component call: which component, its props, and when.
#[derive(Clone, Copy, Debug)]
pub struct SceneCall<'a> {
    pub component: &'a str,
    pub props: &'a Map<String, Value>,
    /// Scene-relative seconds.
    pub t: f64,
    /// The scene's length in seconds (`ctx.duration`).
    pub duration: f64,
    /// The scene's index, which seeds `Math.random` when it has no `id`.
    pub index: usize,
    /// The scene's `id`, which seeds `Math.random` when it has one.
    pub id: Option<&'a str>,
    /// The scene's narration timings for `word()` and `wordEnd()`; `None`
    /// for a silent scene or a component rendered on its own.
    pub words: Option<&'a WordCues>,
}

pub struct Engine {
    host: Host,
    typeset: Typeset,
    images: ImageCache,
    canvas: Canvas,
    theme: Theme,
    size: Size,
    scale: f64,
    pixels: (u16, u16),
    /// How `<Morph>` blends colours: the video's `morphColor`, else the
    /// theme's (a bad theme value falls back to oklch here; lint reports it).
    morph_color: MorphColor,
    /// Typeset `<Math>` formulas, kept across frames.
    math: MathCache,
}

impl Engine {
    /// `assets` is the folder `<Image src>` resolves in.
    pub fn new(
        theme: Theme,
        size: Size,
        assets: Option<PathBuf>,
        options: RenderOptions,
    ) -> Result<Engine> {
        let scale = options.scale;
        if !(scale.is_finite() && scale > 0.0) {
            return Err(StudioError::Render(format!(
                "scale must be a positive number, got {scale}"
            )));
        }
        let px = |v: u32, what: &str| {
            let n = (f64::from(v) * scale).round();
            if (1.0..=f64::from(u16::MAX)).contains(&n) {
                Ok(n as u16)
            } else {
                Err(StudioError::Render(format!(
                    "{what} {v} at scale {scale} is {n} px; frames must be 1 to 65535 px"
                )))
            }
        };
        let pixels = (px(size.width, "width")?, px(size.height, "height")?);
        let morph_color = morph_color(None, &MotionSpec::default(), &theme).unwrap_or_default();
        Ok(Engine {
            morph_color,
            host: Host::new(&theme.tokens)?,
            typeset: Typeset::new(),
            images: ImageCache::new(assets),
            canvas: Canvas::new(pixels.0, pixels.1),
            theme,
            size,
            scale,
            pixels,
            math: MathCache::default(),
        })
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub(crate) fn set_morph_color(&mut self, c: MorphColor) {
        self.morph_color = c;
    }

    /// The morph's colours: the theme's text colour and `c`.
    pub(crate) fn ink(&self, c: MorphColor) -> Ink {
        Ink {
            text: self.theme.text_color,
            colour: c,
        }
    }

    /// Compiles a component (replacing an earlier version of it) and returns
    /// its props schema and examples. A failed load keeps the earlier version.
    pub fn load_component(&mut self, name: &str, tsx: &str) -> Result<&ComponentInfo> {
        self.host.load(name, tsx)
    }

    pub fn component(&self, name: &str) -> Option<&ComponentInfo> {
        self.host.info(name)
    }

    /// Validates props against a loaded component's schema, with defaults
    /// filled in and tokens resolved.
    pub fn check_props(
        &self,
        name: &str,
        props: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        let info = self.component(name).ok_or_else(|| not_loaded(name))?;
        validate_props(name, &info.props, props, &self.theme)
    }

    /// Renders one call. The component runs in a fresh JS context (see
    /// `host`), so the frame never depends on what this engine rendered
    /// before it.
    pub fn render(&mut self, call: SceneCall<'_>) -> Result<Frame> {
        let prepared = self.prepare(call)?;
        self.compose(vec![prepared.tree], prepared.logs, call.component)
    }

    /// `render`, also refusing a tree with a key on two nodes (which only
    /// breaks a morph, so plain renders allow it): lint's render.
    pub(crate) fn render_checked(&mut self, call: SceneCall<'_>) -> Result<Frame> {
        let prepared = self.prepare(call)?;
        if let Some(e) = duplicate_key(&prepared.tree) {
            self.typeset.reclaim(prepared.tree);
            return Err(self.morph_error(call.component, call.component, e));
        }
        self.compose(vec![prepared.tree], prepared.logs, call.component)
    }

    /// Returns trees that will not be painted to the text pool.
    pub(crate) fn discard(&mut self, layers: Vec<LNode>) {
        for layer in layers {
            self.typeset.reclaim(layer);
        }
    }

    /// `discard`, then ends the frame as `compose` does, so a long run of
    /// trees that are never painted keeps the text pool small.
    pub(crate) fn release(&mut self, layers: Vec<LNode>) {
        self.discard(layers);
        self.typeset.end_frame();
    }

    /// Output px per layout px.
    pub(crate) fn scale(&self) -> f64 {
        self.scale
    }

    /// Paints laid-out trees in order onto one frame over the theme
    /// background, then returns their text layouts to the pool. `label`
    /// names the frame in a paint error.
    pub(crate) fn compose(
        &mut self,
        layers: Vec<LNode>,
        logs: Vec<String>,
        label: &str,
    ) -> Result<Frame> {
        self.canvas.begin(self.theme.background);
        let mut painted = Ok(());
        for layer in &layers {
            painted = paint(
                &mut self.canvas,
                layer,
                Affine::scale(self.scale),
                self.theme.text_color,
            );
            if painted.is_err() {
                break;
            }
        }
        // Always rasterise, so the context is reset with no open layers.
        let rgba = self.canvas.finish();
        for layer in layers {
            self.typeset.reclaim(layer);
        }
        self.typeset.end_frame();
        painted.map_err(|m| StudioError::Render(format!("{label}: {m}")))?;
        Ok(Frame {
            width: u32::from(self.pixels.0),
            height: u32::from(self.pixels.1),
            rgba,
            logs,
            samples: 1,
        })
    }
}

fn not_loaded(name: &str) -> StudioError {
    StudioError::ComponentNotFound {
        name: name.to_owned(),
        message: "it has not been loaded into this renderer".to_owned(),
    }
}
