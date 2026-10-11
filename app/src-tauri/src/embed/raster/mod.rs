//! Page rasterization for retrieval's page-image embeddings: what a PDF page
//! looks like, as PNG bytes (see `docs/retrieval.md`).
//!
//! A document's pages render on [`budget::Budget::embed_slots`] big-stack worker
//! threads at once and reach the caller in page order, one at a time. Each
//! render holds an embedder reservation from the process-wide [`budget`] from
//! the first pixel to the finished PNG.

use std::path::Path;

use hayro::hayro_syntax::page::Page;
use hayro::hayro_syntax::Pdf;

use crate::library::pdf_render::{self, budget, guarded, OpenError};

mod dpi;
#[cfg(test)]
mod tests;
mod workers;

use dpi::{dpi_for_page, pixels_for};
use workers::{render_document, render_one};

/// Rendered above the model's own pixel cap and left for it to downscale, so
/// the raster is never the bottleneck. Not a tuning knob: lowering it silently
/// costs recall on formula and diagram pages, and stored vectors were made at
/// this value.
pub const RENDER_DPI: u32 = 200;

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

/// Properties of one document, except `Worker`. Mapped into `EmbedError` in
/// `embed/voyage/batch.rs`.
#[derive(Debug)]
pub enum RasterError {
    /// The file could not be read or parsed as a PDF.
    Unreadable(String),
    /// Locked by a password other than the empty one.
    Encrypted,
    /// A valid PDF with no pages — an error, since zero vectors would look done.
    Empty,
    /// One page failed to render or encode.
    Page { page_no: u32, message: String },
    /// No render thread could be started; nothing to do with the document.
    Worker(String),
}

impl std::fmt::Display for RasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RasterError::Unreadable(message) => write!(f, "unreadable pdf: {message}"),
            RasterError::Encrypted => write!(f, "pdf is password protected"),
            RasterError::Empty => write!(f, "pdf has no pages"),
            RasterError::Page { page_no, message } => {
                write!(f, "page {page_no} failed to render: {message}")
            }
            RasterError::Worker(message) => write!(f, "no render thread: {message}"),
        }
    }
}

impl std::error::Error for RasterError {}

impl From<OpenError> for RasterError {
    fn from(error: OpenError) -> Self {
        match error {
            OpenError::Read(message) => RasterError::Unreadable(message),
            OpenError::Encrypted => RasterError::Encrypted,
            OpenError::Invalid => RasterError::Unreadable("not a pdf hayro can read".into()),
            OpenError::Panicked => RasterError::Unreadable("the renderer crashed on it".into()),
        }
    }
}

/// The page's pixel size at `dpi`. The box is hayro's render dimensions: the
/// crop box in points with `/Rotate` applied.
fn page_pixels(page: &Page<'_>, dpi: u32, max_pixels: Option<u64>) -> (u32, u32) {
    let (width_pt, height_pt) = page.render_dimensions();
    let dpi = dpi_for_page(width_pt, height_pt, dpi, max_pixels);
    (pixels_for(width_pt, dpi), pixels_for(height_pt, dpi))
}

fn open(pdf: &Path) -> Result<Pdf, RasterError> {
    let document = pdf_render::open(pdf)?;
    let count = guarded(|| document.pages().len())
        .map_err(|_| RasterError::Unreadable("the renderer crashed on it".into()))?;
    if count == 0 {
        return Err(RasterError::Empty);
    }
    Ok(document)
}

/// Renders every page of `pdf` at [`RENDER_DPI`], handing each to `on_page`
/// in page order as it is produced, and returns the page count. Streams
/// because a whole deck of decoded pages would be gigabytes; `on_page` may
/// return an error to stop, and so does the first page that fails.
///
/// `max_pixels` is the backend's per-image ceiling, if any (see
/// [`dpi_for_page`]).
pub fn render_pages<F>(pdf: &Path, max_pixels: Option<u64>, on_page: F) -> Result<u32, RasterError>
where
    F: FnMut(RenderedPage) -> Result<(), RasterError>,
{
    let document = open(pdf)?;
    render_document(&document, budget::global(), max_pixels, on_page)
}

/// One 1-based page of an open document, as [`render_pages`] renders it — for
/// measuring a page alone (`examples/render_memory.rs`).
pub fn render_page(
    document: &Pdf,
    page_no: u32,
    max_pixels: Option<u64>,
) -> Result<RenderedPage, RasterError> {
    let page = (page_no as usize)
        .checked_sub(1)
        .and_then(|index| document.pages().get(index))
        .ok_or_else(|| RasterError::Page {
            page_no,
            message: "no such page".into(),
        })?;
    render_one(page, page_no, budget::global(), max_pixels)
}

/// Every page's size in pixels at [`RENDER_DPI`], from the page boxes without
/// rendering — the estimator's input, via the same [`pixels_for`]. A page the
/// renderer clamps (see [`dpi_for_page`]) reports its full size here; both are
/// past the billing cap, so the estimate is unchanged.
pub fn page_sizes(pdf: &Path) -> Result<Vec<(u32, u32)>, RasterError> {
    let document = open(pdf)?;
    guarded(|| {
        document
            .pages()
            .iter()
            .map(|page| page_pixels(page, RENDER_DPI, None))
            .collect()
    })
    .map_err(|_| RasterError::Unreadable("the renderer crashed on its page boxes".into()))
}

/// The page count the renderer will produce: hayro's, the same reading of the
/// page tree `parse/` records, so a mismatch with a parse record means the
/// file changed underneath it.
pub fn page_count(pdf: &Path) -> Result<u32, RasterError> {
    Ok(open(pdf)?.pages().len() as u32)
}
