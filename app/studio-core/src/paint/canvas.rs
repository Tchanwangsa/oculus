//! A vello_cpu render context, one per renderer and reused every frame.
//! Single-threaded on purpose: vello's own threads panic with filter
//! layers; renders parallelise across frames instead. Glyphs are drawn as
//! unhinted outlines with the atlas off, so text keeps sub-pixel positions.

use super::gradient::linear;
use crate::layout::TextLayout;
use crate::scene::{Color, Fill};
use parley::PositionedLayoutItem;
use std::sync::Arc;
use vello_cpu::filter_effects::{Filter, FilterFunction};
use vello_cpu::kurbo::{Affine, BezPath, Rect, RoundedRect, Shape};
use vello_cpu::peniko::{Extend, ImageQuality, ImageSampler};
use vello_cpu::{
    Glyph, Image, ImageSource, Level, Pixmap, RasterizerSettings, RenderContext, RenderSettings,
    Resources,
};

/// Flattening tolerance for rounded corners, in px.
const TOLERANCE: f64 = 0.05;

pub struct Canvas {
    ctx: RenderContext,
    res: Resources,
    width: u16,
    height: u16,
}

pub fn rounded(r: Rect, radius: f64) -> BezPath {
    let radius = radius
        .min(r.width().abs() / 2.0)
        .min(r.height().abs() / 2.0)
        .max(0.0);
    RoundedRect::from_rect(r, radius).to_path(TOLERANCE)
}

impl Canvas {
    pub fn new(width: u16, height: u16) -> Canvas {
        Canvas {
            ctx: RenderContext::new_with(
                width,
                height,
                RenderSettings {
                    level: Level::new(),
                    num_threads: 0,
                },
            ),
            res: Resources::new(),
            width,
            height,
        }
    }

    pub fn begin(&mut self, background: Color) {
        self.ctx.reset();
        self.ctx.set_transform(Affine::IDENTITY);
        self.ctx.set_paint(background.to_vello());
        self.ctx.fill_rect(&Rect::new(
            0.0,
            0.0,
            f64::from(self.width),
            f64::from(self.height),
        ));
    }

    /// An isolated group, composited on `pop` with opacity and a gaussian
    /// blur (`sigma` px in local units).
    pub fn push_group(&mut self, t: Affine, opacity: f32, sigma: f32) {
        self.ctx.set_transform(t);
        let filter =
            (sigma > 0.0).then(|| Filter::from_function(FilterFunction::Blur { radius: sigma }));
        self.ctx.push_layer(None, None, Some(opacity), None, filter);
    }

    pub fn push_clip(&mut self, t: Affine, shape: &BezPath) {
        self.ctx.set_transform(t);
        self.ctx.push_clip_layer(shape);
    }

    pub fn pop(&mut self) {
        self.ctx.pop_layer();
    }

    pub fn fill(&mut self, t: Affine, r: Rect, radius: f64, fill: &Fill, alpha: f32) {
        self.ctx.set_transform(t);
        match fill {
            Fill::Solid(c) => self.ctx.set_paint(c.with_alpha_mul(alpha).to_vello()),
            Fill::Linear { angle_deg, stops } => {
                self.ctx.set_paint(linear(r, *angle_deg, stops, alpha));
            }
        }
        if radius <= 0.0 {
            self.ctx.fill_rect(&r);
        } else {
            self.ctx.fill_path(&rounded(r, radius));
        }
    }

    /// A `box-shadow` under a rounded rect: analytic gaussian, `sigma` px.
    pub fn shadow(&mut self, t: Affine, r: Rect, radius: f64, sigma: f32, color: Color) {
        self.ctx.set_transform(t);
        self.ctx.set_paint(color.to_vello());
        self.ctx
            .fill_blurred_rounded_rect(&r, radius as f32, sigma.max(0.01), false);
    }

    pub fn stroke(&mut self, t: Affine, r: Rect, radius: f64, width: f64, color: Color) {
        self.ctx.set_transform(t);
        self.ctx.set_paint(color.to_vello());
        self.ctx.set_stroke(vello_cpu::kurbo::Stroke::new(width));
        let inner = r.inset(-width / 2.0);
        self.ctx
            .stroke_path(&rounded(inner, (radius - width / 2.0).max(0.0)));
    }

    /// Draws `img` scaled by `place` (image px -> local px), clipped to
    /// `shape`.
    pub fn image(
        &mut self,
        t: Affine,
        img: &Arc<Pixmap>,
        place: Affine,
        shape: &BezPath,
        alpha: f32,
    ) {
        self.ctx.set_transform(t);
        self.ctx.set_paint(Image {
            image: ImageSource::Pixmap(Arc::clone(img)),
            sampler: ImageSampler {
                x_extend: Extend::Pad,
                y_extend: Extend::Pad,
                quality: ImageQuality::High,
                alpha,
            },
        });
        self.ctx.set_paint_transform(place);
        self.ctx.fill_path(shape);
        self.ctx.reset_paint_transform();
    }

    /// Draws a laid-out text block with its top-left at `origin`.
    pub fn text(
        &mut self,
        t: Affine,
        origin: (f64, f64),
        layout: &TextLayout,
        color: Color,
    ) -> Result<(), String> {
        self.ctx.set_transform(t * Affine::translate(origin));
        self.ctx.set_paint(color.to_vello());
        let mut coords: Vec<i16> = Vec::new();
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let r = run.run();
                coords.clear();
                coords.extend(r.normalized_coords().iter().map(|c| c.to_bits()));
                let glyphs = run.positioned_glyphs().map(|g| Glyph {
                    id: g.id,
                    x: g.x,
                    y: g.y,
                });
                self.ctx
                    .glyph_run(&mut self.res, r.font())
                    .font_size(r.font_size())
                    .hint(false)
                    .normalized_coords(&coords)
                    .atlas_cache(false)
                    .fill_glyphs(glyphs)
                    .map_err(|e| format!("drawing glyphs failed: {e:?}"))?;
            }
        }
        Ok(())
    }

    /// Rasterises the frame into straight-alpha RGBA8.
    pub fn finish(&mut self) -> Vec<u8> {
        let mut pm = Pixmap::new(self.width, self.height);
        self.ctx.flush();
        self.ctx
            .render_with(&mut pm, &mut self.res, RasterizerSettings::default());
        pm.take_rgba8(vello_cpu::peniko::ImageAlphaType::Alpha)
    }
}
