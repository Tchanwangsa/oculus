//! Page rasterization — what a PDF page looks like, as PNG bytes, for
//! retrieval's page-image embeddings (see `docs/retrieval.md`).
//!
//! The renderer is pdfium via `pdfium-render`. The native library is fetched,
//! not vendored (`bun run pdfium` into `app/src-tauri/binaries/`); see
//! `library_candidates` for how it is found at runtime.

use std::ffi::OsString;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use pdfium_render::prelude::{PdfRenderConfig, Pdfium, PdfiumError, PdfiumInternalError, Pixels};

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
    crate::ratelimit::hold(&SESSION)
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
/// properties of one document. Mapped into `EmbedError` in `voyage/batch.rs`.
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

/// Pixels for a page this many points wide at `dpi`, bit-exact with MuPDF's
/// `fz_round_rect` (`ceil(x - 0.001)`; the epsilon stops an integral edge
/// gaining a blank column) so page geometry matches the existing renders.
fn pixels_for(points: f32, dpi: u32) -> u32 {
    let scaled = points * dpi as f32 / 72.0;
    ((scaled - 0.001).ceil() as i64).max(1) as u32
}

/// The DPI one page renders at: [`RENDER_DPI`], or less when its pixels would
/// exceed the backend's per-image ceiling (a poster page in a deck), which
/// would otherwise fail the whole document. Costs nothing: the backend
/// downscales far below that ceiling before it bills. Rounded down and then
/// verified, because [`pixels_for`] rounds up.
fn dpi_for_page(width_pt: f32, height_pt: f32, dpi: u32, max_pixels: Option<u64>) -> u32 {
    let Some(max_pixels) = max_pixels.filter(|max| *max > 0) else {
        return dpi;
    };
    let pixels =
        |at: u32| u64::from(pixels_for(width_pt, at)) * u64::from(pixels_for(height_pt, at));
    if pixels(dpi) <= max_pixels {
        return dpi;
    }
    let scale = (max_pixels as f64 / pixels(dpi) as f64).sqrt();
    let mut at = ((dpi as f64 * scale).floor() as u32).max(1);
    while at > 1 && pixels(at) > max_pixels {
        at -= 1;
    }
    at
}

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

/// pdfium's page count. It can disagree with `hayro-syntax`'s (which `parse/`
/// counts with): the two repair damaged xrefs and page trees differently, and
/// may read different revisions. Since `page_no` is the join key, callers
/// treat a mismatch as a document error.
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

// --- binding to libpdfium -------------------------------------------------

/// `Pdfium::new` may only be called once per process, so the binding is a
/// singleton (`Send + Sync` under the `thread_safe` feature).
static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

fn pdfium() -> Result<&'static Pdfium, RasterError> {
    match PDFIUM.get_or_init(bind) {
        Ok(pdfium) => Ok(pdfium),
        Err(message) => Err(RasterError::Library(message.clone())),
    }
}

fn bind() -> Result<Pdfium, String> {
    let mut tried = Vec::new();
    for candidate in library_candidates() {
        if !candidate.exists() {
            continue;
        }
        match Pdfium::bind_to_library(&candidate) {
            Ok(bindings) => return Ok(Pdfium::new(bindings)),
            Err(error) => tried.push(format!("{} ({error})", candidate.display())),
        }
    }

    // Last resort: a system pdfium. Its version may not match the pinned one,
    // and a mismatch fails at bind time.
    match Pdfium::bind_to_system_library() {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(error) => {
            tried.push(format!("system library ({error})"));
            Err(format!(
                "no usable libpdfium; run `bun run pdfium` in app/ to fetch one. Tried: {}",
                tried.join("; ")
            ))
        }
    }
}

/// Where to look for the library, in order: `OCULUS_PDFIUM_LIB` (the dylib
/// or its directory); the bundle's `Contents/Frameworks/`; beside the
/// executable; `binaries/` in an ancestor (dev builds under `target/`); and the
/// compile-time manifest dir. Relative to `current_exe()`, never the cwd.
fn library_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let file_name = Pdfium::pdfium_platform_library_name();

    if let Some(explicit) = std::env::var_os("OCULUS_PDFIUM_LIB") {
        let path = PathBuf::from(explicit);
        if path.is_dir() {
            candidates.push(path.join(&file_name));
        } else {
            candidates.push(path);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            // Bundled .app: Contents/MacOS/Oculus -> Contents/Frameworks/.
            candidates.push(dir.join("../Frameworks").join(&file_name));
            // Beside the executable, for a flat install layout.
            candidates.push(dir.join(&file_name));
            // Dev: target/debug/app, target/debug/deps/<test> -> src-tauri/binaries.
            push_ancestor_binaries(&mut candidates, dir, &file_name);
        }
    }

    // What `cargo test` normally hits; absent in release builds.
    candidates.push(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(&file_name),
    );

    candidates
}

fn push_ancestor_binaries(candidates: &mut Vec<PathBuf>, from: &Path, file_name: &OsString) {
    for ancestor in from.ancestors().take(6) {
        candidates.push(ancestor.join("binaries").join(file_name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    /// A real PDF built in memory: pdfium parses it for real.
    fn synthetic_pdf(pages: usize) -> (Scratch, PathBuf) {
        use lopdf::{dictionary, Document, Object};

        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let font_id = document.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let resources_id = document.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        let mut kids = Vec::new();
        for index in 0..pages {
            let content = lopdf::content::Content {
                operations: vec![
                    lopdf::content::Operation::new("BT", vec![]),
                    lopdf::content::Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    lopdf::content::Operation::new("Td", vec![40.into(), 700.into()]),
                    lopdf::content::Operation::new(
                        "Tj",
                        vec![Object::string_literal(format!("page {}", index + 1))],
                    ),
                    lopdf::content::Operation::new("ET", vec![]),
                ],
            };
            let content_id = document.add_object(lopdf::Stream::new(
                dictionary! {},
                content.encode().unwrap(),
            ));
            let page_id = document.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                // US Letter.
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            kids.push(page_id.into());
        }

        let count = kids.len() as i64;
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => count,
                "Resources" => resources_id,
            }),
        );
        let catalog_id = document.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog_id);

        let dir = Scratch::new("raster-synthetic");
        let path = dir.join("synthetic.pdf");
        document.save(&path).unwrap();
        (dir, path)
    }

    /// Tests needing libpdfium skip without it (`bun run pdfium`).
    /// The count `parse/` writes into the record.
    fn hayro_page_count(pdf: &Path) -> u32 {
        let bytes = std::fs::read(pdf).unwrap();
        hayro_syntax::Pdf::new(bytes).unwrap().pages().len() as u32
    }

    fn library_present() -> bool {
        let available = pdfium().is_ok();
        if !available {
            eprintln!("skipping: libpdfium not fetched (run `bun run pdfium` in app/)");
        }
        available
    }

    #[test]
    fn render_dpi_matches_the_stored_vectors() {
        assert_eq!(RENDER_DPI, 200);
    }

    #[test]
    fn pixel_maths_matches_mupdf_rounding() {
        // Landscape A4 as PyMuPDF renders it.
        assert_eq!(pixels_for(842.0, 200), 2339);
        assert_eq!(pixels_for(595.0, 200), 1653);
        assert_eq!(pixels_for(612.0, 200), 1700);
        assert_eq!(pixels_for(792.0, 200), 2200);
        // An exactly-integral edge gains no blank column.
        assert_eq!(pixels_for(360.0, 72), 360);
        assert_eq!(pixels_for(72.0, 200), 200);
        // Degenerate boxes still produce a renderable bitmap.
        assert_eq!(pixels_for(0.0, 200), 1);
    }

    #[test]
    fn an_oversized_page_is_rendered_smaller_rather_than_refused() {
        // Quoted, not imported: raster does not know which backend asks.
        const MAX: u64 = 16_000_000;

        // A0 landscape.
        let raw =
            u64::from(pixels_for(3370.0, RENDER_DPI)) * u64::from(pixels_for(2384.0, RENDER_DPI));
        assert!(raw > MAX, "fixture is not actually oversized: {raw}");

        let dpi = dpi_for_page(3370.0, 2384.0, RENDER_DPI, Some(MAX));
        assert!(dpi < RENDER_DPI);
        let clamped = u64::from(pixels_for(3370.0, dpi)) * u64::from(pixels_for(2384.0, dpi));
        assert!(
            clamped <= MAX,
            "still over the ceiling: {clamped} at {dpi} DPI"
        );
        // Only just under: one DPI more must not fit.
        let over = u64::from(pixels_for(3370.0, dpi + 1)) * u64::from(pixels_for(2384.0, dpi + 1));
        assert!(over > MAX, "clamped further than it had to: {dpi} DPI");
        assert!(clamped > 2_000_000 * 4);
    }

    #[test]
    fn an_ordinary_page_is_untouched_by_the_clamp() {
        assert_eq!(
            dpi_for_page(842.0, 595.0, RENDER_DPI, Some(16_000_000)),
            RENDER_DPI
        );
        assert_eq!(
            dpi_for_page(612.0, 792.0, RENDER_DPI, Some(16_000_000)),
            RENDER_DPI
        );
        assert_eq!(dpi_for_page(3370.0, 2384.0, RENDER_DPI, None), RENDER_DPI);
    }

    #[test]
    fn renders_every_page_once_in_order() {
        if !library_present() {
            return;
        }
        let (_dir, pdf) = synthetic_pdf(3);

        let mut seen = Vec::new();
        let count = render_pages(&pdf, None, |page| {
            assert!(page.png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
            seen.push((page.page_no, page.width, page.height));
            Ok(())
        })
        .unwrap();

        assert_eq!(count, 3);
        assert_eq!(
            seen,
            vec![(1, 1700, 2200), (2, 1700, 2200), (3, 1700, 2200)]
        );
    }

    /// The estimator's sizes are the renderer's sizes.
    #[test]
    fn measured_sizes_are_the_sizes_that_get_rendered() {
        if !library_present() {
            return;
        }
        let (_dir, pdf) = synthetic_pdf(3);

        let measured = page_sizes(&pdf).unwrap();
        let mut rendered = Vec::new();
        render_pages(&pdf, None, |page| {
            rendered.push((page.width, page.height));
            Ok(())
        })
        .unwrap();

        assert_eq!(measured, rendered);
    }

    /// The invariant [`SESSION`] exists for.
    #[test]
    fn two_threads_reading_the_same_document_both_get_the_truth() {
        if !library_present() {
            return;
        }
        let (_dir, pdf) = synthetic_pdf(6);
        let expected = page_sizes(&pdf).unwrap();

        let workers: Vec<_> = (0..4)
            .map(|_| {
                let path = pdf.clone();
                std::thread::spawn(move || (0..12).map(|_| page_sizes(&path)).collect::<Vec<_>>())
            })
            .collect();

        for worker in workers {
            for attempt in worker.join().unwrap() {
                assert_eq!(attempt.as_ref().map_err(|e| e.to_string()), Ok(&expected));
            }
        }
    }

    #[test]
    fn a_missing_file_is_measurable_as_an_error_not_a_panic() {
        if !library_present() {
            return;
        }
        assert!(page_sizes(Path::new("/nope/does-not-exist.pdf")).is_err());
    }

    #[test]
    fn page_count_agrees_with_hayro_on_a_well_formed_file() {
        if !library_present() {
            return;
        }
        let (_dir, pdf) = synthetic_pdf(4);
        assert_eq!(page_count(&pdf).unwrap(), hayro_page_count(&pdf));
    }

    #[test]
    fn a_caller_can_stop_early() {
        if !library_present() {
            return;
        }
        let (_dir, pdf) = synthetic_pdf(5);
        let mut rendered = 0;
        let result = render_pages(&pdf, None, |_| {
            rendered += 1;
            if rendered == 2 {
                return Err(RasterError::Empty);
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(rendered, 2, "rendering continued past the caller's error");
    }

    #[test]
    fn a_malformed_file_is_an_error_not_a_panic() {
        if !library_present() {
            return;
        }
        let dir = Scratch::new("raster-not-a");
        let pdf = dir.join("not-a.pdf");
        std::fs::write(&pdf, b"%PDF-1.7\nthis is not a pdf at all\n").unwrap();
        assert!(matches!(
            render_pages(&pdf, None, |_| Ok(())),
            Err(RasterError::Unreadable(_) | RasterError::Empty)
        ));
    }

    #[test]
    fn a_missing_file_is_an_error_not_a_panic() {
        if !library_present() {
            return;
        }
        let missing = Path::new("/nonexistent/oculus/raster/missing.pdf");
        assert!(render_pages(missing, None, |_| Ok(())).is_err());
        assert!(page_count(missing).is_err());
    }

    #[test]
    fn an_empty_file_is_an_error_not_a_panic() {
        if !library_present() {
            return;
        }
        let dir = Scratch::new("raster-empty");
        let pdf = dir.join("empty.pdf");
        std::fs::write(&pdf, b"").unwrap();
        assert!(render_pages(&pdf, None, |_| Ok(())).is_err());
    }

    /// Off unless `OCULUS_RASTER_PDF` points at a real library PDF.
    #[test]
    fn renders_a_real_library_pdf() {
        let Some(path) = std::env::var_os("OCULUS_RASTER_PDF") else {
            eprintln!("skipping: set OCULUS_RASTER_PDF to a real library PDF");
            return;
        };
        if !library_present() {
            return;
        }
        let path = PathBuf::from(path);
        let mut first = None;
        let count = render_pages(&path, None, |page| {
            if page.page_no == 1 {
                first = Some(page);
            }
            Ok(())
        })
        .unwrap();

        assert!(count > 0);
        let hayro_count = hayro_page_count(&path);
        eprintln!("pages: pdfium {count}, hayro {hayro_count}");
        assert_eq!(count, hayro_count);

        let first = first.expect("no page 1");
        assert!(first.png.len() > 1024, "page 1 PNG is suspiciously small");
        eprintln!(
            "page 1: {} x {} ({} bytes)",
            first.width,
            first.height,
            first.png.len()
        );

        // `OCULUS_RASTER_OUT` keeps page 1 for a visual check.
        if let Some(out) = std::env::var_os("OCULUS_RASTER_OUT") {
            std::fs::write(&out, &first.png).unwrap();
            eprintln!("wrote {}", PathBuf::from(out).display());
        }
    }
}
