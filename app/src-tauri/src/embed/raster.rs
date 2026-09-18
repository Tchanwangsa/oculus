//! Page rasterization — what a PDF page *looks like*, as PNG bytes.
//!
//! Retrieval embeds the rendered page, not scraped text: image embeddings
//! roughly double recall on formula and diagram pages, which is a measured
//! result and not a taste (`CLAUDE.md`, `docs/retrieval.md`). PyMuPDF used to
//! do the rendering inside the Python sidecar (`render_pages` in
//! `sidecar/embedder.py`); cutting Python out of the app takes the rasterizer
//! with it, and MinerU's result ZIP is no substitute — it returns cropped
//! figures, never page rasters. This module is the replacement, and it is the
//! only genuinely new capability in the cloud migration.
//!
//! The renderer is pdfium (Chromium's PDF engine) reached through
//! `pdfium-render`. pdfium ships as a native dynamic library with no source on
//! crates.io, so `app/scripts/fetch-pdfium.mjs` downloads a prebuilt one into
//! `app/src-tauri/binaries/` — gitignored fetched-artifact territory, like the
//! ffmpeg sidecar beside it. See `library_path` for how it is found at runtime.

use std::ffi::OsString;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use pdfium_render::prelude::{
    Pdfium, PdfiumError, PdfiumInternalError, PdfRenderConfig, Pixels,
};

/// Pages are rendered well above the embedding model's own pixel cap and let
/// the model downscale, so this only has to be high enough not to *be* the
/// bottleneck — past that point extra pixels are thrown away before they reach
/// the encoder, and the real speed/accuracy knob is the model's token budget,
/// not the raster.
///
/// The Python sweep that fixed this (recorded above `RENDER_DPI` in
/// `sidecar/embedder.py`) varied the token budget, not the DPI, and found
/// retrieval accuracy flat from the model default all the way down to ~640
/// tokens before falling off a cliff — and only figure-heavy pages cared at
/// all. 200 DPI sits comfortably above every setting on that curve, so it is a
/// value nothing measured wanted changed. **It is not a tuning parameter**: it
/// is the input side of a pipeline whose output side was tuned against it.
/// Lowering it to "save time" moves the bottleneck onto the raster, silently,
/// and the symptom shows up as worse recall on exactly the formula and diagram
/// pages this whole approach exists to serve.
pub const RENDER_DPI: u32 = 200;

/// **One pdfium session at a time, process-wide.**
///
/// pdfium is not safe to use from two threads, and `pdfium-render`'s
/// `thread_safe` feature — which *is* enabled here — is not enough on its own:
/// it serialises individual FFI calls, where the unit that has to be atomic is
/// the whole session, from `load_pdf_from_file` through the last page render.
///
/// This was measured, not assumed. Two threads sweeping the same 166-file
/// library: **0 failures serially, 18 per thread concurrently**. The errors are
/// worth knowing because neither looks like a race:
///
/// * `PdfiumLibraryInternalError(Unknown)` loading a page — pdfium's own
///   document state, torn.
/// * **`Encrypted`, on files that are not encrypted.** `FPDF_GetLastError()` is
///   process-global and is a *separate* call from the one that failed, so a
///   thread reads whichever error another thread left behind. That is the
///   dangerous one: it is a confident, specific, wrong diagnosis, and it was
///   reaching the settings page as "47 files could not be measured".
///
/// Every entry point below that opens a document takes this first. The hold can
/// be long — an embed's render pass is a whole document — so the trade is
/// explicit: a caller may wait, and no caller gets a torn answer. `render_page`
/// renders one page rather than sweeping the document for it, which is both
/// cheaper and a shorter hold on the interactive path.
static SESSION: Mutex<()> = Mutex::new(());

/// A poisoned lock means another thread panicked inside pdfium; the library is
/// still there and refusing every render afterwards would be worse than
/// carrying on. Same call as the two usage ledgers', kept local so the modules
/// do not reach into each other.
fn session() -> MutexGuard<'static, ()> {
    SESSION.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One rendered page, 1-based to match `page_no` everywhere else in the app —
/// the `pages` table, `<stem>.pages.json`, and the citation deep links all
/// count from 1, and this is the join key retrieval rests on.
#[derive(Clone)]
pub struct RenderedPage {
    pub page_no: u32,
    pub width: u32,
    pub height: u32,
    /// PNG bytes, RGB8. The embedding client base64s these straight into the
    /// request body, so nothing downstream needs a decoder.
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

/// Deliberately local rather than a reuse of `embed::EmbedError`: the two are
/// being written in parallel and will be reconciled. The split it draws is the
/// one that matters for the caller anyway — `Library` is an install problem
/// that will fail identically for every file, while the rest are properties of
/// one document.
#[derive(Debug)]
pub enum RasterError {
    /// libpdfium could not be found or bound. Latching, in `ParseError`'s
    /// vocabulary: retrying another file will not help.
    Library(String),
    /// The file could not be opened or parsed as a PDF.
    Unreadable(String),
    /// Password-protected or otherwise locked by its security handler. There
    /// is no password to try — the library holds coursework, not secrets.
    Encrypted,
    /// A structurally valid PDF with no pages. MinerU's page count would
    /// disagree with reality here too, so it is an error rather than an empty
    /// result: a file that embeds to zero vectors would look "done" forever.
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

impl RasterError {
    /// Mirrors `ParseError::kind()` so one failure UI can read both.
    pub fn kind(&self) -> &'static str {
        match self {
            RasterError::Library(_) => "raster-library-missing",
            RasterError::Unreadable(_) => "unreadable-pdf",
            RasterError::Encrypted => "encrypted-pdf",
            RasterError::Empty => "empty-pdf",
            RasterError::Page { .. } => "page-render-failed",
        }
    }

    /// Only a missing library is worth retrying — after the user installs it.
    /// A malformed document will be just as malformed next time.
    pub fn retryable(&self) -> bool {
        matches!(self, RasterError::Library(_))
    }
}

/// The pixel size PyMuPDF would have produced for a page this many points
/// wide, at `dpi`.
///
/// Kept bit-exact with MuPDF rather than "close enough", because the app has
/// 166 files already embedded from Python renders: a renderer that rounded the
/// other way would produce a different aspect ratio on half the pages, and the
/// stored vectors are compared against new ones by dot product. MuPDF's
/// `fz_round_rect` is `ceil(x - 0.001)` on the far edge (the epsilon keeps an
/// exactly-integral edge from gaining a blank pixel column), and the near edge
/// is 0 for every page whose box starts at the origin.
fn pixels_for(points: f32, dpi: u32) -> u32 {
    let scaled = points * dpi as f32 / 72.0;
    ((scaled - 0.001).ceil() as i64).max(1) as u32
}

/// The DPI one page is actually rendered at: [`RENDER_DPI`], or less when that
/// page is so large that the pixels would be refused.
///
/// **A poster page in an ordinary deck is why this exists.** Every backend has
/// a hard ceiling on the pixels in one image — Voyage's is 16M — and a page
/// past it is not a page that can be split, so without this the whole document
/// fails on it and nothing about a retry can change that. Three real files in
/// the library were stuck exactly there (`page-too-many-pixels-p23` and two
/// p1s): one A0-sized page each, in decks whose other 364 pages were fine.
///
/// **Nothing about it costs quality or money, and that is the argument for
/// clamping rather than skipping.** The backend downscales to its own billing
/// cap (2,000,000 px for Voyage) before it encodes or charges, so every pixel
/// between that cap and the hard ceiling is already thrown away; a page brought
/// down to 16M px is still ~8x past the point where more pixels are looked at,
/// bills the identical 2M, and yields a vector from the same image at the same
/// scale the model would have downscaled it to anyway. Only pages that would
/// otherwise be **refused outright** are touched — every page in the library
/// that embeds today still renders byte-identically at 200 DPI.
///
/// Rounded *down* and then verified, because [`pixels_for`] rounds up: a scale
/// factor that lands a hair over the ceiling would trade a document error for a
/// document error.
fn dpi_for_page(width_pt: f32, height_pt: f32, dpi: u32, max_pixels: Option<u64>) -> u32 {
    let Some(max_pixels) = max_pixels.filter(|max| *max > 0) else { return dpi };
    let pixels = |at: u32| u64::from(pixels_for(width_pt, at)) * u64::from(pixels_for(height_pt, at));
    if pixels(dpi) <= max_pixels {
        return dpi;
    }
    let scale = (max_pixels as f64 / pixels(dpi) as f64).sqrt();
    let mut at = ((dpi as f64 * scale).floor() as u32).max(1);
    // At most a couple of turns in practice — `pixels_for`'s ceil can only
    // overshoot by a pixel on each edge.
    while at > 1 && pixels(at) > max_pixels {
        at -= 1;
    }
    at
}

/// Renders every page of `pdf` at [`RENDER_DPI`], handing each to `on_page` as
/// it is produced. Returns the number of pages rendered.
///
/// `max_pixels` is the caller's backend ceiling for one image, or `None` for a
/// caller that has none. A page over it is rendered smaller rather than
/// refused — see [`dpi_for_page`], which is the whole of that story.
///
/// Streaming rather than returning a `Vec` is the point: a 191-page deck at
/// 200 DPI is ~2 GB of decoded RGB if held at once, and Python's `render_pages`
/// was a generator for the same reason. `on_page` may return an error to stop
/// early — a full-document embed that runs out of quota on page 40 should not
/// render the other 150.
pub fn render_pages<F>(pdf: &Path, max_pixels: Option<u64>, on_page: F) -> Result<u32, RasterError>
where
    F: FnMut(RenderedPage) -> Result<(), RasterError>,
{
    render_pages_at_dpi(pdf, RENDER_DPI, max_pixels, on_page)
}

/// [`render_pages`] with an explicit DPI. Exists for tests and for the odd
/// caller that wants a thumbnail; production embedding always takes the
/// constant, for the reasons on [`RENDER_DPI`].
pub fn render_pages_at_dpi<F>(
    pdf: &Path,
    dpi: u32,
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
        on_page(render_one(&pages, index as u32 + 1, dpi, max_pixels)?)?;
    }

    Ok(count as u32)
}

/// Rasterise one 1-based page of an already-open document.
///
/// Shared by the sweep and by [`render_page`] so the two cannot drift: the
/// pixel arithmetic, the `set_fixed_size` choice and the PNG settings below are
/// each load-bearing, and a second copy of them is a second chance to get the
/// page geometry subtly wrong — which retrieval would only show as hits on the
/// wrong slide. The caller holds [`SESSION`].
fn render_one(
    pages: &pdfium_render::prelude::PdfPages<'_>,
    page_no: u32,
    dpi: u32,
    max_pixels: Option<u64>,
) -> Result<RenderedPage, RasterError> {
    let page = pages
        .get(page_no as i32 - 1)
        .map_err(|error| RasterError::Page { page_no, message: error.to_string() })?;

    // Ordinarily `dpi`. Lower only for a page whose pixels would be refused —
    // see `dpi_for_page`.
    let dpi = dpi_for_page(page.width().value, page.height().value, dpi, max_pixels);
    let width = pixels_for(page.width().value, dpi);
    let height = pixels_for(page.height().value, dpi);

    // `set_fixed_size` rather than `set_target_size`: the dimensions are
    // already computed from this page's own box, and letting pdfium
    // re-derive them from an aspect ratio reintroduces the rounding
    // disagreement with MuPDF that `pixels_for` exists to remove.
    let config = PdfRenderConfig::new().set_fixed_size(width as Pixels, height as Pixels);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|error| RasterError::Page { page_no, message: error.to_string() })?;

    // `as_image` is what normalises pdfium's channel order (it renders
    // reversed-byte-order by default); dropping to RGB8 matches what
    // Python handed the embedder, which was PIL "RGB".
    let rgb = bitmap
        .as_image()
        .map_err(|error| RasterError::Page { page_no, message: error.to_string() })?
        .into_rgb8();

    let mut png = Vec::new();
    // Fast compression, not best: these bytes live long enough to be
    // base64'd into one HTTP request and are then dropped. Trading ~10%
    // of body size for several times the encode speed is the right way
    // round for a 166-file re-index.
    PngEncoder::new_with_quality(
        Cursor::new(&mut png),
        CompressionType::Fast,
        FilterType::Adaptive,
    )
    .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
    .map_err(|error| RasterError::Page { page_no, message: error.to_string() })?;

    Ok(RenderedPage { page_no, width: rgb.width(), height: rgb.height(), png })
}

/// Renders a single 1-based page, and **only** that page.
///
/// It used to run [`render_pages`] over the whole document and keep the one it
/// wanted, which rasterised a 191-page deck to answer a question about page 3.
/// That was merely wasteful until [`SESSION`] arrived; now it would also hold
/// the process's one pdfium session for the length of the sweep — and, since
/// `render_pages` takes the same lock, deadlock on the way in.
pub fn render_page(pdf: &Path, page_no: u32) -> Result<RenderedPage, RasterError> {
    if page_no == 0 {
        return Err(RasterError::Page { page_no, message: "pages are numbered from 1".into() });
    }
    let _session = session();
    let pdfium = pdfium()?;
    let document = pdfium.load_pdf_from_file(pdf, None).map_err(load_error)?;
    let pages = document.pages();
    let count = pages.len();
    if count <= 0 {
        return Err(RasterError::Empty);
    }
    if page_no > count as u32 {
        return Err(RasterError::Page { page_no, message: "page number out of range".into() });
    }
    render_one(&pages, page_no, RENDER_DPI, None)
}

/// Every page's size in pixels at [`RENDER_DPI`], **without rendering any of
/// them**.
///
/// This is the estimator's input, and it is a different call from
/// [`render_pages`] on purpose. Voyage bills a page image by its pixels
/// (capped at 2,000,000), so "what will this run cost and how long will it
/// take" is answerable exactly from the page boxes — pdfium parses those out
/// of the document structure, where rasterising 2,980 pages to find out would
/// cost more than the embedding it is trying to predict.
///
/// The arithmetic is [`pixels_for`], the same function `render_pages_at_dpi`
/// uses, so an estimate and the run it predicts cannot disagree about a page's
/// size.
///
/// The one page they *can* disagree about costs nothing: a page over the
/// backend's per-image ceiling is rendered at a lower DPI (see
/// [`dpi_for_page`]) while this reports its full size. Both are past the
/// 2,000,000-pixel cap the backend bills at, so the estimate is unchanged
/// either way — and a `page_sizes` that guessed at a caller's ceiling would be
/// a second place for that number to live.
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

/// pdfium's page count.
///
/// **This can disagree with `lopdf`'s**, which is what `parse/mineru/` counts
/// with, and `page_no` is the join key retrieval rests on — so the disagreement
/// is worth stating rather than discovering. lopdf walks the page tree and
/// counts the leaves it can reach; pdfium loads the document the way a viewer
/// would, repairing what it can. They part company on: a `/Count` that lies
/// (pdfium trusts the tree it walks, lopdf trusts what it reached), a damaged
/// xref that pdfium rebuilds by scanning for objects and lopdf cannot follow,
/// a page tree with a cycle or a node that is not reachable from the catalog,
/// linearized or incrementally-updated files where the two disagree about
/// which revision is current, and encrypted documents — lopdf reads the
/// structure of some files pdfium refuses outright. Callers that need the two
/// to agree should compare them and treat a mismatch as a document error
/// rather than embedding pages against the wrong numbers.
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

/// Is the renderer usable at all — i.e. did libpdfium bind?
///
/// `RasterError::Library` is the one failure in this module that condemns
/// every file rather than one, and `embed::EmbedError` — which is
/// `ParseError`'s vocabulary on purpose — has no variant that is both latching
/// and fixable-by-the-user. So the embedder asks this in its `health()`
/// instead, and `embed::preflight` refuses the run **once**, before a single
/// file, rather than failing two hundred of them with the same message.
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

/// `Pdfium::new` may only be called once in a process — a second call returns
/// `PdfiumLibraryBindingsAlreadyInitialized` — so the binding is a process
/// singleton. `Pdfium` is `Send + Sync` (the crate's `thread_safe` feature,
/// on by default, serialises the C calls behind a mutex), so one static is
/// safe to share across the Tauri command threads.
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

    // Last resort: a pdfium already on the loader's search path (Homebrew, a
    // distro package). Never the first choice — a version that does not match
    // the one `fetch-pdfium.mjs` pins can be missing symbols this crate binds.
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

/// Where to look for the library, in order.
///
/// The load path has to satisfy two very different layouts. In development —
/// `cargo test`, `cargo run`, the `oculus` CLI out of `target/` — the binary
/// sits some levels below `app/src-tauri/`, so walking up from the executable
/// for a `binaries/` directory finds the fetched copy, and the compile-time
/// manifest directory catches the cases where it does not (a test binary run
/// from an unusual cwd, `cargo test` under a different target dir). In the
/// bundled `.app` the executable is `Contents/MacOS/Oculus` and Tauri's
/// `bundle.macOS.frameworks` has put the dylib in `Contents/Frameworks/`, one
/// hop sideways.
///
/// Everything is resolved relative to `current_exe()` rather than the working
/// directory: the app is launched by Finder with a cwd of `/`, and the CLI is
/// run from wherever the user happens to be standing.
///
/// `OCULUS_PDFIUM_LIB` overrides the lot — it takes either the dylib itself or
/// a directory containing it — so a packager or a test harness can point at a
/// copy without moving files around.
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

    // Compile-time fallback, which is the one `cargo test` normally hits.
    // Baked into release builds too, where it simply will not exist.
    candidates.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries").join(&file_name));

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

    /// A one-page PDF built in memory, so the render tests do not reach
    /// outside the repo. Same trick `parse/mineru/client.rs`'s tests use:
    /// pdfium parses the file for real, so a stub would not do.
    fn synthetic_pdf(pages: usize) -> tempdir::Guard {
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
            let content_id =
                document.add_object(lopdf::Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = document.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                // 612 x 792 pt — US Letter, so the expected pixel sizes below
                // are not the same numbers as the real-library fixture.
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

        let guard = tempdir::Guard::new("raster-synthetic.pdf");
        document.save(&guard.path).unwrap();
        guard
    }

    /// The tests that need libpdfium skip rather than fail on a checkout that
    /// has not run `bun run pdfium` yet, which is what keeps `cargo test`
    /// green without the fetched artifact.
    fn library_present() -> bool {
        let available = pdfium().is_ok();
        if !available {
            eprintln!("skipping: libpdfium not fetched (run `bun run pdfium` in app/)");
        }
        available
    }

    #[test]
    fn render_dpi_matches_the_python_sidecar() {
        // `sidecar/embedder.py` is the source of truth; if that constant moves
        // every stored vector was rendered at the old one.
        assert_eq!(RENDER_DPI, 200);
    }

    #[test]
    fn pixel_maths_matches_mupdf_rounding() {
        // The real fixture: 842 x 595 pt at 200 DPI is 2339 x 1653 in PyMuPDF.
        assert_eq!(pixels_for(842.0, 200), 2339);
        assert_eq!(pixels_for(595.0, 200), 1653);
        // US Letter, the synthetic fixture below.
        assert_eq!(pixels_for(612.0, 200), 1700);
        assert_eq!(pixels_for(792.0, 200), 2200);
        // An exactly-integral edge must not gain a blank pixel column — that
        // is what the epsilon in MuPDF's fz_round_rect is for.
        assert_eq!(pixels_for(360.0, 72), 360);
        assert_eq!(pixels_for(72.0, 200), 200);
        // Degenerate boxes still produce a renderable bitmap.
        assert_eq!(pixels_for(0.0, 200), 1);
    }

    #[test]
    fn an_oversized_page_is_rendered_smaller_rather_than_refused() {
        // Voyage's ceiling, quoted rather than imported: raster does not know
        // which backend is asking, and a test that read the constant from
        // `batch.rs` would pass even if the two stopped meaning the same thing.
        const MAX: u64 = 16_000_000;

        // A0 landscape, 3370 x 2384 pt — the shape that stranded three real
        // files. 200 DPI puts it at ~62M px, four times the ceiling.
        let raw = u64::from(pixels_for(3370.0, RENDER_DPI)) * u64::from(pixels_for(2384.0, RENDER_DPI));
        assert!(raw > MAX, "fixture is not actually oversized: {raw}");

        let dpi = dpi_for_page(3370.0, 2384.0, RENDER_DPI, Some(MAX));
        assert!(dpi < RENDER_DPI);
        let clamped =
            u64::from(pixels_for(3370.0, dpi)) * u64::from(pixels_for(2384.0, dpi));
        assert!(clamped <= MAX, "still over the ceiling: {clamped} at {dpi} DPI");
        // Only just under it: a clamp that overshot would cost detail for
        // nothing. One DPI more must not fit.
        let over = u64::from(pixels_for(3370.0, dpi + 1)) * u64::from(pixels_for(2384.0, dpi + 1));
        assert!(over > MAX, "clamped further than it had to: {dpi} DPI");
        // And it is still far past the 2M the backend bills and looks at.
        assert!(clamped > 2_000_000 * 4);
    }

    #[test]
    fn an_ordinary_page_is_untouched_by_the_clamp() {
        // Every page in the library that embeds today must keep rendering at
        // exactly 200 DPI — the stored vectors were made from those pixels.
        assert_eq!(dpi_for_page(842.0, 595.0, RENDER_DPI, Some(16_000_000)), RENDER_DPI);
        assert_eq!(dpi_for_page(612.0, 792.0, RENDER_DPI, Some(16_000_000)), RENDER_DPI);
        // No ceiling means no clamp, whatever the size.
        assert_eq!(dpi_for_page(3370.0, 2384.0, RENDER_DPI, None), RENDER_DPI);
    }

    #[test]
    fn renders_every_page_once_in_order() {
        if !library_present() {
            return;
        }
        let pdf = synthetic_pdf(3);

        let mut seen = Vec::new();
        let count = render_pages(&pdf.path, None, |page| {
            assert!(page.png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
            seen.push((page.page_no, page.width, page.height));
            Ok(())
        })
        .unwrap();

        assert_eq!(count, 3);
        assert_eq!(seen, vec![(1, 1700, 2200), (2, 1700, 2200), (3, 1700, 2200)]);
    }

    /// The estimator's whole claim to be exact: the sizes it predicts from the
    /// page boxes are the sizes the renderer produces. If these ever part
    /// company the settings page quotes a cost for a run that bills something
    /// else.
    #[test]
    fn measured_sizes_are_the_sizes_that_get_rendered() {
        if !library_present() {
            return;
        }
        let pdf = synthetic_pdf(3);

        let measured = page_sizes(&pdf.path).unwrap();
        let mut rendered = Vec::new();
        render_pages(&pdf.path, None, |page| {
            rendered.push((page.width, page.height));
            Ok(())
        })
        .unwrap();

        assert_eq!(measured, rendered);
    }

    /// The invariant [`SESSION`] exists for, pinned against the measurement
    /// that found it: two threads over the same file used to produce
    /// `PdfiumLibraryInternalError` and — worse — a confident `Encrypted`
    /// verdict on documents that are not encrypted, because
    /// `FPDF_GetLastError()` is process-global. Anything that removes the lock
    /// brings both back, and the second one is a lie rather than a failure.
    #[test]
    fn two_threads_reading_the_same_document_both_get_the_truth() {
        if !library_present() {
            return;
        }
        let pdf = synthetic_pdf(6);
        let expected = page_sizes(&pdf.path).unwrap();

        let workers: Vec<_> = (0..4)
            .map(|_| {
                let path = pdf.path.clone();
                std::thread::spawn(move || {
                    (0..12).map(|_| page_sizes(&path)).collect::<Vec<_>>()
                })
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
    fn page_count_agrees_with_lopdf_on_a_well_formed_file() {
        if !library_present() {
            return;
        }
        let pdf = synthetic_pdf(4);
        let theirs = lopdf::Document::load(&pdf.path).unwrap().get_pages().len() as u32;
        assert_eq!(page_count(&pdf.path).unwrap(), theirs);
    }

    #[test]
    fn a_caller_can_stop_early() {
        if !library_present() {
            return;
        }
        let pdf = synthetic_pdf(5);
        let mut rendered = 0;
        let result = render_pages(&pdf.path, None, |_| {
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
    fn render_page_picks_one_page() {
        if !library_present() {
            return;
        }
        let pdf = synthetic_pdf(3);
        assert_eq!(render_page(&pdf.path, 2).unwrap().page_no, 2);
        assert!(render_page(&pdf.path, 9).is_err());
    }

    #[test]
    fn a_malformed_file_is_an_error_not_a_panic() {
        if !library_present() {
            return;
        }
        let guard = tempdir::Guard::new("raster-not-a.pdf");
        std::fs::write(&guard.path, b"%PDF-1.7\nthis is not a pdf at all\n").unwrap();
        assert!(matches!(
            render_pages(&guard.path, None, |_| Ok(())),
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
        let guard = tempdir::Guard::new("raster-empty.pdf");
        std::fs::write(&guard.path, b"").unwrap();
        assert!(render_pages(&guard.path, None, |_| Ok(())).is_err());
    }

    /// The real-library check, against a PDF that only exists on a machine
    /// with a synced library. Point `OCULUS_RASTER_PDF` at one to run it;
    /// without it the test skips, because tests must not depend on files
    /// outside the repo.
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
        // The `page_no` join key is only sound while both counters agree; see
        // `page_count` for the cases where they do not.
        let lopdf_count = lopdf::Document::load(&path).unwrap().get_pages().len() as u32;
        eprintln!("pages: pdfium {count}, lopdf {lopdf_count}");
        assert_eq!(count, lopdf_count);

        let first = first.expect("no page 1");
        assert!(first.png.len() > 1024, "page 1 PNG is suspiciously small");
        eprintln!("page 1: {} x {} ({} bytes)", first.width, first.height, first.png.len());

        // Dimensions prove the geometry; only eyes prove the pixels. Set
        // `OCULUS_RASTER_OUT` to a file path to keep page 1 and look at it
        // beside PyMuPDF's render of the same page.
        if let Some(out) = std::env::var_os("OCULUS_RASTER_OUT") {
            std::fs::write(&out, &first.png).unwrap();
            eprintln!("wrote {}", PathBuf::from(out).display());
        }
    }


    /// A self-deleting temp path. The repo has no dev-dependency for this and
    /// the tests need three lines of it, not a crate.
    mod tempdir {
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};

        pub struct Guard {
            pub path: PathBuf,
        }

        impl Guard {
            pub fn new(name: &str) -> Self {
                static SEQUENCE: AtomicU64 = AtomicU64::new(0);
                let unique = format!(
                    "{}-{}-{}",
                    std::process::id(),
                    SEQUENCE.fetch_add(1, Ordering::Relaxed),
                    name
                );
                Guard { path: std::env::temp_dir().join(unique) }
            }
        }

        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}
