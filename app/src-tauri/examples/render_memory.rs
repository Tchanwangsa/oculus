//! Measures the embedder's raster path (render + PNG, no network) over every
//! PDF in the live library, read-only — what `pdf_render::budget`'s constants
//! are calibrated against (docs/retrieval.md).
//!
//!   cargo run --release --example render_memory -- [--slots N] [--docs N]
//!       [--budget-mb N] [--only TEXT] [--skip TEXT] [--calibrate]
//!       [--viewer TEXT:PAGE [--viewer-repeat N] [--viewer-only]]
//!
//! The default run renders `--docs` documents at once (1, as the app's index
//! queue does) under a budget of `--slots` render slots (the app's default
//! when omitted), and prints wall time, pages/s, pages rendered below 200 DPI
//! and the process's peak RSS. `--viewer` also renders one page, of the first
//! PDF whose path contains TEXT, in the viewer's lane at the viewer's pixel cap,
//! `--viewer-repeat` times, beside the run or (`--viewer-only`) alone.
//! `--calibrate` renders one page at a time and prints how much heap each
//! render peaked at against its pixels, which sets `BASE_BYTES` and
//! `BYTES_PER_PIXEL`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use app_lib::embed::raster;
use app_lib::embed::voyage::batch::MAX_RENDER_PIXELS as MAX_PIXELS;
use app_lib::library::pdf_render::budget::{self, Budget, Lane};

/// The viewer's raster caps (`pdf_view.rs`, `app/src/lib/pdfView.ts`).
const VIEWER_MAX_SIDE: f64 = 8192.0;
const VIEWER_MAX_PIXELS: f64 = 16_000_000.0;

/// Past this a page counts as large in `--calibrate`'s split: A4 and 16:9
/// slides at 200 DPI are about 4M.
const LARGE_PIXELS: u64 = 6_000_000;

/// Counts live heap bytes and their high-water mark, for `--calibrate`.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        System.dealloc(pointer, layout);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        // A move holds both blocks for a moment; count it as if it moves.
        let live = LIVE.load(Ordering::Relaxed);
        PEAK.fetch_max(live + size, Ordering::Relaxed);
        let moved = System.realloc(pointer, layout, size);
        if !moved.is_null() {
            if size >= layout.size() {
                let live = LIVE.fetch_add(size - layout.size(), Ordering::Relaxed)
                    + (size - layout.size());
                PEAK.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - size, Ordering::Relaxed);
            }
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage fills the struct it is handed.
    let usage = unsafe {
        libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr());
        usage.assume_init()
    };
    // Bytes on macOS, kilobytes on Linux.
    if cfg!(target_os = "macos") {
        usage.ru_maxrss as u64
    } else {
        usage.ru_maxrss as u64 * 1024
    }
}

fn find_pdfs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_pdfs(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
        {
            out.push(path);
        }
    }
}

fn flag(args: &[String], name: &str) -> Option<usize> {
    let at = args.iter().position(|arg| arg == name)?;
    args.get(at + 1)?.parse().ok()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut pdfs = Vec::new();
    find_pdfs(
        &app_lib::library::paths::data_dir().join("courses"),
        &mut pdfs,
    );
    pdfs.sort();
    let viewer = args
        .iter()
        .position(|arg| arg == "--viewer")
        .and_then(|at| args.get(at + 1))
        .and_then(|spec| spec.rsplit_once(':'))
        .map(|(text, page)| {
            let pdf = pdfs
                .iter()
                .find(|path| path.to_string_lossy().contains(text))
                .expect("--viewer matches no PDF")
                .clone();
            (pdf, page.parse::<u32>().expect("--viewer TEXT:PAGE"))
        });
    // `--only TEXT` keeps the paths containing it, `--skip TEXT` drops them.
    for (name, keep) in [("--only", true), ("--skip", false)] {
        if let Some(at) = args.iter().position(|arg| arg == name) {
            let text = args.get(at + 1).cloned().unwrap_or_default();
            pdfs.retain(|path| path.to_string_lossy().contains(&text) == keep);
        }
    }
    let bytes: u64 = pdfs
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok())
        .map(|meta| meta.len())
        .sum();
    println!("{} PDFs, {} MB", pdfs.len(), bytes >> 20);

    let budget_bytes =
        flag(&args, "--budget-mb").map_or(budget::BUDGET_BYTES, |mb| (mb as u64) << 20);
    let slots = flag(&args, "--slots");
    if args.iter().any(|arg| arg == "--calibrate") {
        budget::install(Budget::new(budget_bytes, 1));
        calibrate(&pdfs);
        return;
    }
    if let Some(slots) = slots {
        budget::install(Budget::new(budget_bytes, slots));
    }
    let docs = flag(&args, "--docs").unwrap_or(1).max(1);
    if slots.is_none() && budget_bytes != budget::BUDGET_BYTES {
        let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
        budget::install(Budget::new(
            budget_bytes,
            (cores / 2).clamp(1, budget::MAX_SLOTS),
        ));
    }
    let repeat = flag(&args, "--viewer-repeat").unwrap_or(1);
    let viewer = viewer.map(|(pdf, page)| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn(move || view(&pdf, page, repeat))
            .unwrap()
    });
    if !(viewer.is_some() && args.iter().any(|arg| arg == "--viewer-only")) {
        run(&pdfs, docs, budget_bytes);
    }
    if let Some(viewer) = viewer {
        viewer.join().unwrap();
        println!(
            "after the viewer renders: peak RSS {} MB",
            peak_rss_bytes() >> 20
        );
    }
}

/// One page in the viewer's lane at the viewer's cap, `repeat` times, as a
/// zoomed-in reader would ask for it.
fn view(pdf: &Path, page_no: u32, repeat: usize) {
    let document = app_lib::library::pdf_render::open(pdf).expect("open the viewer's PDF");
    let page = &document.pages()[page_no as usize - 1];
    let (width_pt, height_pt) = page.render_dimensions();
    let (w, h) = (width_pt as f64 * 100.0, height_pt as f64 * 100.0);
    let k = (VIEWER_MAX_SIDE / w)
        .min(VIEWER_MAX_SIDE / h)
        .min((VIEWER_MAX_PIXELS / (w * h)).sqrt());
    let (width, height) = ((w * k).floor() as u32, (h * k).floor() as u32);
    for _ in 0..repeat {
        let started = Instant::now();
        let held = budget::global().reserve(Lane::Viewer, budget::estimate(width, height));
        let waited = started.elapsed();
        let rgba =
            app_lib::library::pdf_render::render_rgba(page, width, height).expect("viewer render");
        drop(held);
        println!(
            "viewer: {} p{page_no} at {width}x{height}: waited {:.0} ms, rendered in {:.0} ms ({} MB)",
            pdf.file_name().unwrap_or_default().to_string_lossy(),
            waited.as_secs_f64() * 1e3,
            (started.elapsed() - waited).as_secs_f64() * 1e3,
            rgba.len() >> 20
        );
    }
}

/// Every document through `render_pages`, `docs` at a time.
fn run(pdfs: &[PathBuf], docs: usize, budget_bytes: u64) {
    let embed_slots = budget::global().embed_slots();
    println!(
        "budget {} MB, {embed_slots} embedder render thread(s) per document, {docs} document(s) at once",
        budget_bytes >> 20
    );
    let next = AtomicUsize::new(0);
    let pages = AtomicU64::new(0);
    let png_bytes = AtomicU64::new(0);
    let smaller = AtomicU64::new(0);
    let failures = Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..docs {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(pdf) = pdfs.get(index) else {
                    return;
                };
                let full = raster::page_sizes(pdf).unwrap_or_default();
                let outcome = raster::render_pages(pdf, Some(MAX_PIXELS), |page| {
                    pages.fetch_add(1, Ordering::Relaxed);
                    if full.get(page.page_no as usize - 1) != Some(&(page.width, page.height)) {
                        smaller.fetch_add(1, Ordering::Relaxed);
                    }
                    png_bytes.fetch_add(page.png.len() as u64, Ordering::Relaxed);
                    Ok(())
                });
                if let Err(error) = outcome {
                    failures
                        .lock()
                        .unwrap()
                        .push(format!("{}: {error}", pdf.display()));
                }
            });
        }
    });
    let seconds = started.elapsed().as_secs_f64();
    let pages = pages.into_inner();
    println!(
        "{pages} pages in {seconds:.1} s = {:.1} pages/s ({} below 200 DPI); PNG {} MB; peak RSS {} MB",
        pages as f64 / seconds,
        smaller.into_inner(),
        png_bytes.into_inner() >> 20,
        peak_rss_bytes() >> 20
    );
    for failure in failures.into_inner().unwrap() {
        println!("failed: {failure}");
    }
}

/// One page at a time: each render's heap high-water mark over what was live
/// before it, against its pixels.
fn calibrate(pdfs: &[PathBuf]) {
    // (pixels, peak bytes over the baseline, file, page)
    let mut samples: Vec<(u64, u64, usize, u32)> = Vec::new();
    for (file, pdf) in pdfs.iter().enumerate() {
        let Ok(document) = app_lib::library::pdf_render::open(pdf) else {
            println!("unopenable: {}", pdf.display());
            continue;
        };
        for page_no in 1..=document.pages().len() as u32 {
            let base = LIVE.load(Ordering::SeqCst);
            PEAK.store(base, Ordering::SeqCst);
            let Ok(page) = raster::render_page(&document, page_no, Some(MAX_PIXELS)) else {
                println!("failed: {} p{page_no}", pdf.display());
                continue;
            };
            let peak = PEAK.load(Ordering::SeqCst).saturating_sub(base) as u64;
            samples.push((
                u64::from(page.width) * u64::from(page.height),
                peak,
                file,
                page_no,
            ));
        }
    }
    let count = samples.len();
    println!("{count} pages, peak RSS {} MB", peak_rss_bytes() >> 20);

    let mut per_pixel: Vec<f64> = samples
        .iter()
        .map(|(pixels, peak, ..)| *peak as f64 / *pixels as f64)
        .collect();
    per_pixel.sort_by(f64::total_cmp);
    let quantile = |sorted: &[f64], q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
    println!(
        "peak bytes per pixel: median {:.2}, p90 {:.2}, p99 {:.2}, max {:.2}",
        quantile(&per_pixel, 0.5),
        quantile(&per_pixel, 0.9),
        quantile(&per_pixel, 0.99),
        quantile(&per_pixel, 1.0)
    );

    // The same for ordinary pages and large ones (posters, design exports).
    for (label, large) in [("under", false), ("past", true)] {
        let mut class: Vec<f64> = samples
            .iter()
            .filter(|(pixels, ..)| (*pixels > LARGE_PIXELS) == large)
            .map(|(pixels, peak, ..)| *peak as f64 / *pixels as f64)
            .collect();
        if class.is_empty() {
            continue;
        }
        class.sort_by(f64::total_cmp);
        let worst = samples
            .iter()
            .filter(|(pixels, ..)| (*pixels > LARGE_PIXELS) == large)
            .map(|(_, peak, ..)| *peak)
            .max()
            .unwrap_or(0);
        println!(
            "{label} {:.1} Mpx: {} pages, bytes per pixel median {:.2}, p99 {:.2}, max {:.2}; largest peak {:.1} MB",
            LARGE_PIXELS as f64 / 1e6,
            class.len(),
            quantile(&class, 0.5),
            quantile(&class, 0.99),
            quantile(&class, 1.0),
            worst as f64 / 1048576.0
        );
    }

    // For each candidate factor, the fixed part that covers 99% and 100% of pages.
    for factor in [6u64, 8, 10, 12, 16] {
        let mut over: Vec<f64> = samples
            .iter()
            .map(|(pixels, peak, ..)| peak.saturating_sub(factor * pixels) as f64)
            .collect();
        over.sort_by(f64::total_cmp);
        println!(
            "{factor:>2} B/px: base for p99 {:.1} MB, for all {:.1} MB",
            quantile(&over, 0.99) / 1048576.0,
            quantile(&over, 1.0) / 1048576.0
        );
    }

    samples.sort_by_key(|(pixels, peak, ..)| std::cmp::Reverse(peak.saturating_sub(8 * pixels)));
    println!("largest past 8 B/px:");
    for (pixels, peak, file, page_no) in samples.iter().take(8) {
        println!(
            "  {:>7.1} MB at {:>5.1} Mpx  {} p{page_no}",
            *peak as f64 / 1048576.0,
            *pixels as f64 / 1e6,
            pdfs[*file].display()
        );
    }
}
