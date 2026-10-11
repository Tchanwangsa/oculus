//! A page at exactly the pixel size asked for, opaque on white, and the
//! big-stack threads page interpretation runs on.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::page::Page;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

use super::documents::guarded;

/// The stack every hayro page interpretation runs on. hayro recurses through
/// forms, patterns and Type 3 glyphs, and a stack overflow aborts the process,
/// which `catch_unwind` cannot stop. Reserved, not committed.
pub(crate) const RENDER_STACK: usize = 256 << 20;

/// hayro sizes its render context in `u16`s.
pub const MAX_RENDER_SIDE: u32 = u16::MAX as u32;

/// Runs `work` on its own thread with a [`RENDER_STACK`] stack and awaits it,
/// for async callers: tokio's blocking threads have small stacks. A panic in
/// `work` is "render-failed".
pub(crate) async fn on_render_thread<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let (send, receive) = tokio::sync::oneshot::channel();
    std::thread::Builder::new()
        .name("pdf-render".into())
        .stack_size(RENDER_STACK)
        .spawn(move || {
            let outcome = guarded(work).unwrap_or_else(|_| Err("render-failed".into()));
            let _ = send.send(outcome);
        })
        .map_err(|error| format!("start a render thread: {error}"))?;
    receive.await.map_err(|_| "render-failed".to_string())?
}

/// Why [`render_rgba`] produced nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    /// A side is zero or past [`MAX_RENDER_SIDE`].
    BadSize,
    Panicked,
}

/// `page` as RGBA8, exactly `width` × `height`, opaque on white. The caller
/// holds a [`budget`] reservation for it.
pub fn render_rgba(page: &Page<'_>, width: u32, height: u32) -> Result<Vec<u8>, RenderError> {
    if width == 0 || height == 0 || width > MAX_RENDER_SIDE || height > MAX_RENDER_SIDE {
        return Err(RenderError::BadSize);
    }
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
    })
    .map_err(|_| RenderError::Panicked)?;
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
pub(crate) fn scale_to(side: f32, pixels: u32) -> f32 {
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
