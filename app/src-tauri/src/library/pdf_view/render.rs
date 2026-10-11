//! A page rendered to exact-size RGBA, under a viewer reservation from the
//! process-wide render budget.

use hayro::hayro_syntax::Pdf;

use super::documents::page_of;
use crate::library::pdf_render::budget::{self, Lane};
use crate::library::pdf_render::{self, RenderError};

const MAX_SIDE: u32 = 8192;
/// Past this the frontend CSS-scales the raster (`MAX_PIXELS` in
/// `app/src/lib/pdf/pdfRender.ts`): a 16M-pixel layered page can take ~1 GB to
/// render.
const MAX_PIXELS: u64 = 16_000_000;

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
    let _held = budget::global().reserve(Lane::Viewer, budget::estimate(width, height));
    pdf_render::render_rgba(page, width, height).map_err(|error| {
        match error {
            RenderError::BadSize => "invalid-size",
            RenderError::Panicked => "render-failed",
        }
        .to_string()
    })
}
