//! A region of one PDF page rendered by hayro at an exact pixel size, opaque
//! on white: only the region is interpreted into a context its size, so a
//! small figure at 2x never rasterises the whole page at that scale.

use std::io::Cursor;

use hayro::hayro_interpret::util::TransformExt;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::page::Page;
use hayro::hayro_syntax::Pdf;
use hayro::kurbo::Affine;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::vello_cpu::{self, Pixmap, RenderContext};
use hayro::{RenderCache, RenderSettings};
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

use super::geometry::Frac;
use crate::library::pdf_render::{self, budget, guarded, OpenError, RENDER_STACK};

/// RGB8 pixels, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

/// Runs `work` on the PDF in `bytes`, on a thread with hayro's stack
/// ([`RENDER_STACK`]), every hayro call inside it guarded.
pub fn with_pdf<T: Send + 'static>(
    bytes: Vec<u8>,
    work: impl FnOnce(&Pdf) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let thread = std::thread::Builder::new()
        .name("studio-slide".into())
        .stack_size(RENDER_STACK)
        .spawn(move || {
            let pdf = pdf_render::parse(bytes).map_err(|e| match e {
                OpenError::Encrypted => "the PDF is password-protected".to_owned(),
                OpenError::Invalid => "the file is not a PDF the renderer can read".to_owned(),
                OpenError::Panicked => "the renderer crashed reading the PDF".to_owned(),
                OpenError::Read(e) => e,
            })?;
            guarded(|| work(&pdf)).unwrap_or_else(|_| Err("the renderer crashed".to_owned()))
        })
        .map_err(|e| format!("start a render thread: {e}"))?;
    thread
        .join()
        .unwrap_or_else(|_| Err("the renderer crashed".to_owned()))
}

/// Page `page_no` (1-based), or an error naming the pages there are.
pub fn page_of<'a>(pdf: &'a Pdf, page_no: u32) -> Result<&'a Page<'a>, String> {
    let pages = pdf.pages();
    page_no
        .checked_sub(1)
        .and_then(|i| pages.get(i as usize))
        .ok_or_else(|| format!("no page {page_no}; the PDF has pages 1-{}", pages.len()))
}

/// The page's size in points as rendered: its crop box, rotated.
pub fn page_size(page: &Page<'_>) -> (f64, f64) {
    let (w, h) = page.render_dimensions();
    (f64::from(w), f64::from(h))
}

/// `region` of `page` (fractions of the rendered page) at exactly
/// `width` x `height` px, under a render-budget reservation.
pub fn render_region(
    page: &Page<'_>,
    region: &Frac,
    width: u32,
    height: u32,
) -> Result<Raster, String> {
    let (Ok(w16), Ok(h16)) = (u16::try_from(width), u16::try_from(height)) else {
        return Err(format!("{width}x{height} is past the renderer's size"));
    };
    if width == 0 || height == 0 {
        return Err("a raster needs at least one pixel a side".to_owned());
    }
    let _held = budget::global().reserve(budget::Lane::Viewer, budget::estimate(width, height));
    let (pw, ph) = page_size(page);
    let scale_x = f64::from(width) / ((region[2] - region[0]) * pw);
    let scale_y = f64::from(height) / ((region[3] - region[1]) * ph);
    let transform = Affine::translate((-region[0] * pw * scale_x, -region[1] * ph * scale_y))
        * Affine::scale_non_uniform(scale_x, scale_y)
        * page.initial_transform(true).to_kurbo();
    let pixmap = guarded(|| {
        let mut ctx = RenderContext::new(w16, h16);
        hayro::render_into(
            page,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &mut ctx,
            transform,
        );
        ctx.flush();
        let mut pixmap = Pixmap::new(w16, h16);
        ctx.render_with(
            &mut pixmap,
            &mut vello_cpu::Resources::default(),
            vello_cpu::RasterizerSettings {
                target_init: vello_cpu::TargetInit::Clear(WHITE),
                ..Default::default()
            },
        );
        pixmap
    })
    .map_err(|_| "the renderer crashed on this page".to_owned())?;
    // Opaque on white, so premultiplied bytes are the straight ones.
    let rgba = pixmap.take_rgba8(ImageAlphaType::AlphaPremultiplied);
    let rgb = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|[r, g, b, _]| [*r, *g, *b])
        .collect();
    Ok(Raster { width, height, rgb })
}

/// `raster` as PNG bytes; the same pixels always give the same bytes.
pub fn encode_png(raster: &Raster) -> Result<Vec<u8>, String> {
    let mut png = Vec::new();
    PngEncoder::new_with_quality(
        Cursor::new(&mut png),
        CompressionType::Best,
        FilterType::Adaptive,
    )
    .write_image(
        &raster.rgb,
        raster.width,
        raster.height,
        ExtendedColorType::Rgb8,
    )
    .map_err(|e| format!("encode the PNG: {e}"))?;
    Ok(png)
}
