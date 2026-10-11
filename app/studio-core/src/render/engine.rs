//! The project-free renderer: component source + props + theme + size + t
//! -> frame. `Renderer` drives it for a video; the Components playground
//! and `gallery` drive it for one component.

use super::{Frame, RenderOptions, Size};
use crate::error::{Result, StudioError};
use crate::host::{CallCtx, ComponentInfo, Host, seed, tsx_loc};
use crate::images::ImageCache;
use crate::layout::{LayoutError, Layouter, Typeset};
use crate::paint::{Canvas, paint};
use crate::scene::parse_tree;
use crate::schema::validate_props;
use crate::theme::Theme;
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
    /// The scene's index, which seeds `Math.random`.
    pub index: usize,
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
        Ok(Engine {
            host: Host::new(&theme.tokens)?,
            typeset: Typeset::new(),
            images: ImageCache::new(assets),
            canvas: Canvas::new(pixels.0, pixels.1),
            theme,
            size,
            scale,
            pixels,
        })
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
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
        let name = call.component;
        let props = self.check_props(name, call.props)?;
        let (json, logs) = self.host.call(
            name,
            &props,
            CallCtx {
                t: call.t,
                duration: call.duration,
                width: f64::from(self.size.width),
                height: f64::from(self.size.height),
                seed: seed(name, call.index),
            },
        )?;
        let tsx = self.host.source(name).unwrap_or_default();
        let loc = |l: Option<(u32, u32)>| l.map(|l| tsx_loc(tsx, l));
        let tree = parse_tree(&json).map_err(|e| {
            if e.is_style {
                StudioError::Style {
                    component: name.to_owned(),
                    node: e.path,
                    message: e.message,
                    loc: loc(e.loc),
                }
            } else {
                StudioError::Component {
                    component: name.to_owned(),
                    message: format!("{}: {}", e.path, e.message),
                    loc: loc(e.loc),
                }
            }
        })?;
        let mut layouter = Layouter {
            typeset: &mut self.typeset,
            images: &mut self.images,
            default_family: &self.theme.body_font,
        };
        let laid = layouter.layout(tree, self.size.width as f32, self.size.height as f32);
        let laid = laid.map_err(|e| match e {
            LayoutError::Font { family, loc: l } => StudioError::Font {
                component: name.to_owned(),
                family,
                available: self.typeset.families.clone(),
                loc: loc(l),
            },
            LayoutError::Image {
                src,
                message,
                loc: l,
            } => StudioError::Image {
                component: name.to_owned(),
                src,
                message,
                loc: loc(l),
            },
            LayoutError::Taffy(m) => StudioError::Render(format!("{name}: layout failed: {m}")),
        })?;
        self.canvas.begin(self.theme.background);
        let painted = paint(
            &mut self.canvas,
            &laid,
            Affine::scale(self.scale),
            self.theme.text_color,
        );
        // Always rasterise, so the context is reset with no open layers.
        let rgba = self.canvas.finish();
        painted.map_err(|m| StudioError::Render(format!("{name}: {m}")))?;
        self.typeset.reclaim(laid);
        self.typeset.end_frame();
        Ok(Frame {
            width: u32::from(self.pixels.0),
            height: u32::from(self.pixels.1),
            rgba,
            logs,
        })
    }
}

fn not_loaded(name: &str) -> StudioError {
    StudioError::ComponentNotFound {
        name: name.to_owned(),
        message: "it has not been loaded into this renderer".to_owned(),
    }
}
