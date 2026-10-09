//! Page rasterization — what a PDF page looks like, as PNG bytes, for
//! retrieval's page-image embeddings (see `docs/retrieval.md`).
//!
//! The renderer is pdfium via `pdfium-render`. The native library is fetched,
//! not vendored (`bun run pdfium` into `app/src-tauri/binaries/`); see
//! `pdfium::library_candidates` for how it is found at runtime.

use std::io::Cursor;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use pdfium_render::prelude::{PdfRenderConfig, PdfiumError, PdfiumInternalError, Pixels};

mod dpi;
mod pdfium;
#[cfg(test)]
mod tests;

use dpi::{dpi_for_page, pixels_for};
use pdfium::pdfium;

/// Rendered above the model's own pixel cap and left for it to downscale, so
/// the raster is never the bottleneck. Not a tuning knob: lowering it silently
/// costs recall on formula and diagram pages, and stored vectors were made at
/// this value.
pub const RENDER_DPI: u32 = 200;

/// One pdfium session at a time, process-wide. `pdfium-render`'s
/// `thread_safe` serialises single FFI calls, but the whole session must be
/// atomic: concurrent sessions tear pdfium's state, and because
/// `FPDF_GetLastError()` is process-global a thread can read another's error —
/// e.g. `Encrypted` on a file that is not. Every entry point that opens a
/// document takes this first; a caller may wait, never get a torn answer.
static SESSION: Mutex<()> = Mutex::new(());

/// A poisoned lock still guards a usable library.
fn session() -> MutexGuard<'static, ()> {
    crate::providers::ratelimit::hold(&SESSION)
}

/// One rendered page, 1-based like `page_no` everywhere else (the join key).
#[derive(Clone)]
pub struct RenderedPage {
    pub page_no: u32,
    pub width: u32,
    pub height: u32,
    /// PNG bytes, RGB8, base64'd straight into the request body.
    pub png: Vec<u8>,
}

impl std::fmt::Debug for RenderedPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderedPage")
            .field("page_no", &self.page_no)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("png_bytes", &self.png.len())
            .finish()
    }
}

/// `Library` is an install problem that fails every file alike; the rest are
/// properties of one document. Mapped into `EmbedError` in `voyage/batch/run.rs`.
#[derive(Debug)]
pub enum RasterError {
    /// libpdfium could not be found or bound.
    Library(String),
    /// The file could not be opened or parsed as a PDF.
    Unreadable(String),
    /// Password-protected or locked by its security handler.
    Encrypted,
    /// A valid PDF with no pages — an error, since zero vectors would look done.
    Empty,
    /// One page failed to render or encode.
    Page { page_no: u32, message: String },
}

impl std::fmt::Display for RasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RasterError::Library(message) => write!(f, "pdfium unavailable: {message}"),
            RasterError::Unreadable(message) => write!(f, "unreadable pdf: {message}"),
            RasterError::Encrypted => write!(f, "pdf is password protected"),
            RasterError::Empty => write!(f, "pdf has no pages"),
            RasterError::Page { page_no, message } => {
                write!(f, "page {page_no} failed to render: {message}")
            }
        }
    }
}

impl std::error::Error for RasterError {}

/// Renders every page of `pdf` at [`RENDER_DPI`], handing each to `on_page` as
/// it is produced, and returns the page count. Streams because a whole deck of
/// decoded pages would be gigabytes; `on_page` may return an error to stop.
///
/// `max_pixels` is the backend's per-image ceiling, if any (see
/// [`dpi_for_page`]).
pub fn render_pages<F>(
    pdf: &Path,
    max_pixels: Option<u64>,
    mut on_page: F,
) -> Result<u32, RasterError>
where
    F: FnMut(RenderedPage) -> Result<(), RasterError>,
{
    let _session = session();
    let pdfium = pdfium()?;
    let document = pdfium.load_pdf_from_file(pdf, None).map_err(load_error)?;
    let pages = document.pages();

    let count = pages.len();
    if count <= 0 {
        return Err(RasterError::Empty);
    }

    for index in 0..count {
        on_page(render_one(
            &pages,
            index as u32 + 1,
            RENDER_DPI,
            max_pixels,
        )?)?;
    }

    Ok(count as u32)
}

/// Rasterise one 1-based page of an already-open document. The caller holds
/// [`SESSION`].
fn render_one(
    pages: &pdfium_render::prelude::PdfPages<'_>,
    page_no: u32,
    dpi: u32,
    max_pixels: Option<u64>,
) -> Result<RenderedPage, RasterError> {
    let page = pages
        .get(page_no as i32 - 1)
        .map_err(|error| RasterError::Page {
            page_no,
            message: error.to_string(),
        })?;

    let dpi = dpi_for_page(page.width().value, page.height().value, dpi, max_pixels);
    let width = pixels_for(page.width().value, dpi);
    let height = pixels_for(page.height().value, dpi);

    // `set_fixed_size`, not `set_target_size`: pdfium re-deriving the size
    // from an aspect ratio would undo `pixels_for`'s rounding.
    let config = PdfRenderConfig::new().set_fixed_size(width as Pixels, height as Pixels);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|error| RasterError::Page {
            page_no,
            message: error.to_string(),
        })?;

    // `as_image` normalises pdfium's reversed channel order.
    let rgb = bitmap
        .as_image()
        .map_err(|error| RasterError::Page {
            page_no,
            message: error.to_string(),
        })?
        .into_rgb8();

    let mut png = Vec::new();
    // Fast compression: the bytes live for one request, so encode speed wins.
    PngEncoder::new_with_quality(
        Cursor::new(&mut png),
        CompressionType::Fast,
        FilterType::Adaptive,
    )
    .write_image(
        rgb.as_raw(),
        rgb.width(),
        rgb.height(),
        ExtendedColorType::Rgb8,
    )
    .map_err(|error| RasterError::Page {
        page_no,
        message: error.to_string(),
    })?;

    Ok(RenderedPage {
        page_no,
        width: rgb.width(),
        height: rgb.height(),
        png,
    })
}

/// Every page's size in pixels at [`RENDER_DPI`], from the page boxes without
/// rendering — the estimator's input, via the same [`pixels_for`]. A page the
/// renderer clamps (see [`dpi_for_page`]) reports its full size here; both are
/// past the billing cap, so the estimate is unchanged.
pub fn page_sizes(pdf: &Path) -> Result<Vec<(u32, u32)>, RasterError> {
    let _session = session();
    let pdfium = pdfium()?;
    let document = pdfium.load_pdf_from_file(pdf, None).map_err(load_error)?;
    let pages = document.pages();
    let count = pages.len();
    if count <= 0 {
        return Err(RasterError::Empty);
    }
    let mut sizes = Vec::with_capacity(count as usize);
    for index in 0..count {
        let page = pages.get(index).map_err(|error| RasterError::Page {
            page_no: index as u32 + 1,
            message: error.to_string(),
        })?;
        sizes.push((
            pixels_for(page.width().value, RENDER_DPI),
            pixels_for(page.height().value, RENDER_DPI),
        ));
    }
    Ok(sizes)
}

/// pdfium's page count. It can disagree with `lopdf`'s (which `parse/`
/// counts with): pdfium repairs damaged xrefs and page trees the way a viewer
/// does, and the two may read different revisions. Since `page_no` is the join
/// key, callers treat a mismatch as a document error.
pub fn page_count(pdf: &Path) -> Result<u32, RasterError> {
    let _session = session();
    let pdfium = pdfium()?;
    let document = pdfium.load_pdf_from_file(pdf, None).map_err(load_error)?;
    let count = document.pages().len();
    if count <= 0 {
        return Err(RasterError::Empty);
    }
    Ok(count as u32)
}

/// Did libpdfium bind? Asked by the embedder's `health()` so a missing library
/// refuses the run once rather than failing every file.
pub fn available() -> Result<(), RasterError> {
    let _session = session();
    pdfium().map(|_| ())
}

fn load_error(error: PdfiumError) -> RasterError {
    match &error {
        PdfiumError::PdfiumLibraryInternalError(
            PdfiumInternalError::PasswordError | PdfiumInternalError::SecurityError,
        ) => RasterError::Encrypted,
        _ => RasterError::Unreadable(error.to_string()),
    }
}
