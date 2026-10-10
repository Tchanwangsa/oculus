//! A page rendered to exact-size RGBA.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

use super::documents::{guarded, page_of};

const MAX_SIDE: u32 = 8192;
const MAX_PIXELS: u64 = 40_000_000;

pub(super) fn check_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("invalid-size".into());
    }
    if width > MAX_SIDE || height > MAX_SIDE || width as u64 * height as u64 > MAX_PIXELS {
        return Err("too-large".into());
    }
    Ok(())
}

/// Page `page` (1-based) as RGBA8, exactly `width` × `height`, on white.
pub(super) fn render_page(
    pdf: &Pdf,
    page: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    check_size(width, height)?;
    let page = page_of(pdf, page)?;
    let pixmap = guarded(|| {
        let (page_width, page_height) = page.render_dimensions();
        let settings = PixmapSettings {
            x_scale: scale_to(page_width, width),
            y_scale: scale_to(page_height, height),
            bg_color: WHITE,
        };
        hayro::render(
            page,
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &settings,
        )
    })?;
    let (got_width, got_height) = (pixmap.width() as usize, pixmap.height() as usize);
    // The white background is opaque, so premultiplied bytes are already the
    // straight ones and need no unpremultiply pass.
    let rgba = pixmap.take_rgba8(ImageAlphaType::AlphaPremultiplied);
    Ok(fit(
        rgba,
        got_width,
        got_height,
        width as usize,
        height as usize,
    ))
}

/// The scale at which hayro's `(side * scale) as u16` truncates to exactly
/// `pixels`: the plain quotient, nudged up an ulp at a time when f32 rounding
/// lands it just short.
pub(super) fn scale_to(side: f32, pixels: u32) -> f32 {
    let mut scale = pixels as f32 / side;
    for _ in 0..64 {
        if (side * scale) as u32 >= pixels {
            break;
        }
        scale = f32::from_bits(scale.to_bits() + 1);
    }
    scale
}

/// `rgba` (`width` × `height`) cropped or padded with white to the requested
/// size, should hayro's size ever differ from it.
fn fit(
    rgba: Vec<u8>,
    width: usize,
    height: usize,
    want_width: usize,
    want_height: usize,
) -> Vec<u8> {
    if width == want_width && height == want_height && rgba.len() == width * height * 4 {
        return rgba;
    }
    let mut out = vec![255u8; want_width * want_height * 4];
    let (copy_width, copy_height) = (width.min(want_width), height.min(want_height));
    for row in 0..copy_height {
        let from = row * width * 4;
        let to = row * want_width * 4;
        if let Some(source) = rgba.get(from..from + copy_width * 4) {
            out[to..to + copy_width * 4].copy_from_slice(source);
        }
    }
    out
}
