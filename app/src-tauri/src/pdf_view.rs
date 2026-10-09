//! The PDF viewer's backend: open a library PDF, render a page to exact-size
//! RGBA, and read a page's text lines and links, all with the pure-Rust
//! renderer hayro, for the viewer in docs/viewers.md.
//!
//! Opened documents sit in a small LRU keyed by path and modification stamp.
//! Renders and text extraction run on blocking threads, at most half the cores
//! at once, so a fast scroll queues rather than oversubscribes. Every hayro
//! call runs under `catch_unwind`: a malformed PDF fails its own request with
//! "render-failed" and nothing else.
//!
//! Text layout follows PdfCraft's reading-order extraction
//! (github.com/storytold/pdfcraft, MIT OR Apache-2.0); the adapted part carries
//! its licence notice below.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::SystemTime;

use hayro::hayro_interpret::font::{Glyph, GlyphRun, PositionedGlyph};
use hayro::hayro_interpret::hayro_cmap::BfString;
use hayro::hayro_interpret::{
    interpret_page, BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image,
    ImageDrawProps, InterpreterCache, InterpreterSettings, SoftMask, TransformExt,
};
use hayro::hayro_syntax::object::{Array, Dict, MaybeRef, Name, ObjRef, Object};
use hayro::hayro_syntax::object::{Rect as PdfRect, String as PdfString};
use hayro::hayro_syntax::page::Page;
use hayro::hayro_syntax::{LoadPdfError, Pdf};
use hayro::kurbo::{Affine, BezPath, Rect, Shape};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::peniko::ImageAlphaType;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use serde::Serialize;

/// The library roots the viewer may read: the asset-protocol scope in
/// tauri.conf.json.
const ROOTS: [&str; 3] = ["courses", "lectures", "agents"];
const OPEN_DOCS: usize = 4;
const MAX_SIDE: u32 = 8192;
const MAX_PIXELS: u64 = 40_000_000;

// ── Commands ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Debug)]
pub struct PageSize {
    width: f32,
    height: f32,
}

#[derive(Serialize, Debug)]
pub struct OpenedPdf {
    pages: Vec<PageSize>,
}

/// One line of page text in reading order. `chars` holds `text`'s UTF-16
/// length + 1 stops along the line: x for horizontal lines, y for vertical.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct TextLine {
    text: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    vertical: bool,
    chars: Vec<f32>,
}

#[derive(Serialize, Debug, Default)]
pub struct PageText {
    lines: Vec<TextLine>,
}

/// A link annotation's box and target: a URI, or a 1-based page.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Link {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<u32>,
}

#[tauri::command]
pub async fn pdf_open(path: String) -> Result<OpenedPdf, String> {
    crate::blocking::run(move || {
        let pdf = open_at(&crate::paths::data_dir(), &path)?;
        Ok(page_sizes(&pdf))
    })
    .await
}

/// Raw RGBA8, exactly `width` × `height`, sent as a binary IPC body.
#[tauri::command]
pub async fn pdf_render(
    path: String,
    page: u32,
    width: u32,
    height: u32,
) -> Result<tauri::ipc::Response, String> {
    check_size(width, height)?;
    let rgba = on_render_slot(move || {
        let pdf = open_at(&crate::paths::data_dir(), &path)?;
        render_page(&pdf, page, width, height)
    })
    .await?;
    Ok(tauri::ipc::Response::new(rgba))
}

#[tauri::command]
pub async fn pdf_text(path: String, page: u32) -> Result<PageText, String> {
    on_render_slot(move || {
        let pdf = open_at(&crate::paths::data_dir(), &path)?;
        page_text(&pdf, page)
    })
    .await
}

#[tauri::command]
pub async fn pdf_links(path: String, page: u32) -> Result<Vec<Link>, String> {
    crate::blocking::run(move || {
        let pdf = open_at(&crate::paths::data_dir(), &path)?;
        page_links(&pdf, page)
    })
    .await
}

#[tauri::command]
pub async fn pdf_close(path: String) -> Result<(), String> {
    let path = resolve_in(&crate::paths::data_dir(), &path)?;
    docs().retain(|doc| doc.path != path);
    Ok(())
}

/// Runs CPU-heavy work on a blocking thread once one of the render slots
/// (half the cores) is free.
async fn on_render_slot<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    static SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let slots = SLOTS.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        tokio::sync::Semaphore::new((cores / 2).max(1))
    });
    let _slot = slots.acquire().await.map_err(|error| error.to_string())?;
    crate::blocking::run(work).await
}

// ── Documents ─────────────────────────────────────────────────────────────────

/// `relative` under `root`, if it names something inside one of [`ROOTS`].
fn resolve_in(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    let mut parts = path.components();
    let in_root = matches!(
        parts.next(),
        Some(Component::Normal(first)) if first.to_str().is_some_and(|first| ROOTS.contains(&first))
    );
    if !in_root || !parts.all(|part| matches!(part, Component::Normal(_))) {
        return Err("outside-library".into());
    }
    Ok(root.join(path))
}

struct OpenDoc {
    path: PathBuf,
    /// Modification time and length: a rewritten file is reopened.
    stamp: (SystemTime, u64),
    pdf: Arc<Pdf>,
}

/// Most recently used last.
static DOCS: Mutex<Vec<OpenDoc>> = Mutex::new(Vec::new());

fn docs() -> MutexGuard<'static, Vec<OpenDoc>> {
    DOCS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The document at `relative`, from the cache when its file is unchanged.
/// Parsing happens outside the lock, so one slow file holds up no other.
fn open_at(root: &Path, relative: &str) -> Result<Arc<Pdf>, String> {
    let path = resolve_in(root, relative)?;
    let meta = std::fs::metadata(&path).map_err(|_| "not-found".to_string())?;
    if !meta.is_file() {
        return Err("not-found".into());
    }
    let stamp = (
        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        meta.len(),
    );
    {
        let mut docs = docs();
        if let Some(at) = docs
            .iter()
            .position(|doc| doc.path == path && doc.stamp == stamp)
        {
            let doc = docs.remove(at);
            let pdf = doc.pdf.clone();
            docs.push(doc);
            return Ok(pdf);
        }
    }
    let bytes = std::fs::read(&path).map_err(|_| "not-found".to_string())?;
    let pdf = guarded(|| Pdf::new(Arc::new(bytes)))?.map_err(|error| {
        match error {
            LoadPdfError::Decryption(_) => "encrypted",
            LoadPdfError::Invalid => "invalid",
        }
        .to_string()
    })?;
    let pdf = Arc::new(pdf);
    let mut docs = docs();
    docs.retain(|doc| doc.path != path);
    docs.push(OpenDoc {
        path,
        stamp,
        pdf: pdf.clone(),
    });
    if docs.len() > OPEN_DOCS {
        docs.remove(0);
    }
    Ok(pdf)
}

/// Runs a hayro call, turning a panic inside it into "render-failed".
fn guarded<T>(work: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(work)).map_err(|_| "render-failed".to_string())
}

fn page_sizes(pdf: &Pdf) -> OpenedPdf {
    let pages = pdf
        .pages()
        .iter()
        .map(|page| {
            let (width, height) = page.render_dimensions();
            PageSize { width, height }
        })
        .collect();
    OpenedPdf { pages }
}

fn page_of(pdf: &Pdf, page: u32) -> Result<&Page<'_>, String> {
    (page as usize)
        .checked_sub(1)
        .and_then(|index| pdf.pages().get(index))
        .ok_or_else(|| "page-out-of-range".to_string())
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn check_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("invalid-size".into());
    }
    if width > MAX_SIDE || height > MAX_SIDE || width as u64 * height as u64 > MAX_PIXELS {
        return Err("too-large".into());
    }
    Ok(())
}

/// Page `page` (1-based) as RGBA8, exactly `width` × `height`, on white.
fn render_page(pdf: &Pdf, page: u32, width: u32, height: u32) -> Result<Vec<u8>, String> {
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
fn scale_to(side: f32, pixels: u32) -> f32 {
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

// ── Links ─────────────────────────────────────────────────────────────────────

fn page_links(pdf: &Pdf, page: u32) -> Result<Vec<Link>, String> {
    let page = page_of(pdf, page)?;
    guarded(|| collect_links(pdf, page))
}

/// The page's link annotations whose target resolves, boxed in rendered-page
/// space (rotation and crop-box origin applied).
fn collect_links(pdf: &Pdf, page: &Page<'_>) -> Vec<Link> {
    let Some(annots) = page.raw().get::<Array<'_>>(b"Annots") else {
        return Vec::new();
    };
    let view = page.initial_transform(true).to_kurbo();
    let mut links = Vec::new();
    for annot in annots.iter::<Dict<'_>>() {
        if !annot
            .get::<Name<'_>>(b"Subtype")
            .is_some_and(|kind| kind.as_ref() == b"Link")
        {
            continue;
        }
        let Some(rect) = annot.get::<PdfRect>(b"Rect") else {
            continue;
        };
        let (mut uri, mut target) = (None, None);
        if let Some(action) = annot.get::<Dict<'_>>(b"A") {
            let kind = action.get::<Name<'_>>(b"S");
            match kind.as_deref() {
                Some(b"URI") => {
                    uri = action
                        .get::<PdfString<'_>>(b"URI")
                        .map(|text| String::from_utf8_lossy(text.as_bytes()).into_owned());
                }
                Some(b"GoTo") => {
                    target = action
                        .get::<Object<'_>>(b"D")
                        .and_then(|dest| dest_page(pdf, dest));
                }
                _ => {}
            }
        } else if let Some(dest) = annot.get::<Object<'_>>(b"Dest") {
            target = dest_page(pdf, dest);
        }
        if uri.is_none() && target.is_none() {
            continue;
        }
        let bounds = view.transform_rect_bbox(Rect::new(rect.x0, rect.y0, rect.x1, rect.y1).abs());
        links.push(Link {
            x: round(bounds.x0),
            y: round(bounds.y0),
            width: round(bounds.width()),
            height: round(bounds.height()),
            uri,
            page: target,
        });
    }
    links
}

/// The 1-based page a destination points at: an explicit array, a dict with
/// `/D`, or a name looked up in the catalog.
fn dest_page(pdf: &Pdf, dest: Object<'_>) -> Option<u32> {
    match dest {
        Object::Array(array) => explicit_dest(pdf, &array),
        Object::Dict(dict) => match dict.get::<Object<'_>>(b"D")? {
            Object::Array(array) => explicit_dest(pdf, &array),
            _ => None,
        },
        Object::Name(name) => named_dest(pdf, name.as_ref()),
        Object::String(name) => named_dest(pdf, name.as_bytes()),
        _ => None,
    }
}

/// `[page /Fit …]`: the page is a reference to a page object, or (in some
/// producers) a 0-based index.
fn explicit_dest(pdf: &Pdf, dest: &Array<'_>) -> Option<u32> {
    let pages = pdf.pages();
    let index = match dest.raw_iter().next()? {
        MaybeRef::Ref(target) => pages
            .iter()
            .position(|page| page.raw().obj_id().map(ObjRef::from) == Some(target))?,
        MaybeRef::NotRef(Object::Number(number)) => usize::try_from(number.as_i64())
            .ok()
            .filter(|index| *index < pages.len())?,
        _ => return None,
    };
    u32::try_from(index + 1).ok()
}

/// A named destination: the catalog's `/Names /Dests` tree, or the older
/// `/Dests` dictionary.
fn named_dest(pdf: &Pdf, name: &[u8]) -> Option<u32> {
    let xref = pdf.xref();
    let catalog = xref.get::<Dict<'_>>(xref.root_id())?;
    let found = catalog
        .get::<Dict<'_>>(b"Names")
        .and_then(|names| names.get::<Dict<'_>>(b"Dests"))
        .and_then(|tree| name_tree_get(&tree, name, 0))
        .or_else(|| catalog.get::<Dict<'_>>(b"Dests")?.get::<Object<'_>>(name))?;
    match found {
        Object::Array(array) => explicit_dest(pdf, &array),
        Object::Dict(dict) => match dict.get::<Object<'_>>(b"D")? {
            Object::Array(array) => explicit_dest(pdf, &array),
            _ => None,
        },
        _ => None,
    }
}

/// A name tree's value for `key`, walking `/Kids` to a bounded depth so a
/// cyclic tree ends.
fn name_tree_get<'a>(node: &Dict<'a>, key: &[u8], depth: u8) -> Option<Object<'a>> {
    if depth > 32 {
        return None;
    }
    if let Some(names) = node.get::<Array<'a>>(b"Names") {
        let mut items = names.iter::<Object<'a>>();
        while let (Some(name), Some(value)) = (items.next(), items.next()) {
            if matches!(&name, Object::String(name) if name.as_bytes() == key) {
                return Some(value);
            }
        }
    }
    node.get::<Array<'a>>(b"Kids")?
        .iter::<Dict<'a>>()
        .find_map(|kid| name_tree_get(&kid, key, depth + 1))
}

/// Points to two decimals: finer than any screen, and a shorter IPC body.
fn round(value: f64) -> f32 {
    ((value * 100.0).round() / 100.0) as f32
}

// ── Text ──────────────────────────────────────────────────────────────────────

fn page_text(pdf: &Pdf, page: u32) -> Result<PageText, String> {
    let page = page_of(pdf, page)?;
    guarded(|| extract_text(page))
}

/// One character on the page and its box, `[x0, y0, x1, y1]` in points, y down.
#[derive(Clone, Debug, PartialEq)]
struct TextGlyph {
    ch: char,
    rect: [f32; 4],
}

/// A hayro device that paints nothing and records each glyph's character and
/// em box in rendered-page space, one flow per writing direction.
struct GlyphCollector {
    /// Text running right, down, left and up: a quarter turn each.
    flows: [Vec<TextGlyph>; 4],
    /// The last filled run: fill-and-stroke text draws the same run twice.
    last_fill: Option<(usize, usize)>,
    /// The rendered page, `(0, 0)` to its render dimensions.
    page: Rect,
}

impl<'a> Device<'a> for GlyphCollector {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_glyph_run(&mut self, run: &GlyphRun<'_, 'a>, props: DrawProps<'a>, mode: &DrawMode) {
        let glyphs = run.glyphs();
        let id = (glyphs.as_ptr() as usize, glyphs.len());
        if matches!(mode, DrawMode::Stroke(_)) {
            if self.last_fill.take() == Some(id) {
                return;
            }
        } else {
            self.last_fill = Some(id);
        }
        for glyph in glyphs {
            self.push(glyph, props.transform * glyph.transform());
        }
    }
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

impl GlyphCollector {
    fn push(&mut self, glyph: &PositionedGlyph<'_>, transform: Affine) {
        let Some(unicode) = glyph.as_unicode() else {
            return;
        };
        let text = match unicode {
            BfString::Char(ch) => ch.to_string(),
            BfString::String(text) => text,
        };
        let chars: Vec<char> = text.chars().filter(|ch| !ch.is_control()).collect();
        if chars.is_empty() {
            return;
        }
        // Glyph space is 1000 units per em; the em box runs from the
        // descender (-200) to the ascender (800) and across the advance.
        let advance = match &**glyph {
            Glyph::Outline(outline) => outline
                .advance_width()
                .filter(|advance| advance.is_finite() && *advance > 0.0)
                .map(f64::from)
                .unwrap_or_else(|| outline.outline().bounding_box().width().max(500.0)),
            Glyph::Type3(_) => 600.0,
        };
        let bounds = transform.transform_rect_bbox(Rect::new(0.0, -200.0, advance, 800.0));
        if ![bounds.x0, bounds.y0, bounds.x1, bounds.y1]
            .iter()
            .all(|v| v.is_finite())
            || bounds.width() > 10_000.0
            || bounds.height() > 10_000.0
        {
            return;
        }
        // Text no reader sees: zero-scale glyphs (LaTeXiT and Keynote embed
        // equation source that way) and glyphs parked off the page (Beamer
        // overlays). Thresholds match the pdfium comparison in app/pdf-bench.
        if !(bounds.area() > 1e-4) || bounds.intersect(self.page).area() <= 0.0 {
            return;
        }
        // The baseline's direction, to the nearest quarter turn (y down).
        let [dx, dy, ..] = transform.as_coeffs();
        let quarter = if dx.abs() >= dy.abs() {
            if dx >= 0.0 {
                0
            } else {
                2
            }
        } else if dy > 0.0 {
            1
        } else {
            3
        };
        // A ligature's characters share its box evenly, along the baseline.
        let n = chars.len() as f64;
        for (i, ch) in chars.into_iter().enumerate() {
            let (from, to) = (i as f64 / n, (i + 1) as f64 / n);
            let rect = match quarter {
                0 => [
                    bounds.x0 + bounds.width() * from,
                    bounds.y0,
                    bounds.x0 + bounds.width() * to,
                    bounds.y1,
                ],
                2 => [
                    bounds.x1 - bounds.width() * to,
                    bounds.y0,
                    bounds.x1 - bounds.width() * from,
                    bounds.y1,
                ],
                1 => [
                    bounds.x0,
                    bounds.y0 + bounds.height() * from,
                    bounds.x1,
                    bounds.y0 + bounds.height() * to,
                ],
                _ => [
                    bounds.x0,
                    bounds.y1 - bounds.height() * to,
                    bounds.x1,
                    bounds.y1 - bounds.height() * from,
                ],
            };
            self.flows[quarter].push(TextGlyph {
                ch,
                rect: rect.map(|v| v as f32),
            });
        }
    }
}

/// The page's text lines in reading order. Each writing direction is laid out
/// on its own, upright, and the flows join by their top-most glyph.
fn extract_text(page: &Page<'_>) -> PageText {
    let (width, height) = page.render_dimensions();
    let cache = InterpreterCache::new();
    let mut context = Context::new(
        page.initial_transform(true).to_kurbo(),
        Rect::new(0.0, 0.0, width as f64, height as f64),
        &cache,
        page.xref(),
        InterpreterSettings::default(),
    );
    let mut collector = GlyphCollector {
        flows: Default::default(),
        last_fill: None,
        page: Rect::new(0.0, 0.0, width as f64, height as f64),
    };
    interpret_page(page, &mut context, &mut collector);

    let mut flows = Vec::new();
    for (quarter, mut glyphs) in collector.flows.into_iter().enumerate() {
        if glyphs.is_empty() {
            continue;
        }
        let top = glyphs
            .iter()
            .map(|glyph| glyph.rect[1])
            .fold(f32::INFINITY, f32::min);
        for glyph in &mut glyphs {
            glyph.rect = to_upright(glyph.rect, quarter, width, height);
        }
        let lines: Vec<TextLine> = layout(glyphs)
            .iter()
            .filter_map(|line| emit_line(line, quarter, width, height))
            .collect();
        flows.push((top, lines));
    }
    flows.sort_by(|a, b| a.0.total_cmp(&b.0));
    PageText {
        lines: flows.into_iter().flat_map(|(_, lines)| lines).collect(),
    }
}

/// A laid-out line (upright frame) as the command's line: its text with the
/// inferred word spaces, its box, and one stop per UTF-16 unit plus the end.
fn emit_line(
    line: &[(TextGlyph, bool)],
    quarter: usize,
    width: f32,
    height: f32,
) -> Option<TextLine> {
    let mut text = String::new();
    let mut stops = Vec::new();
    let mut bounds = line.first()?.0.rect;
    for (k, (glyph, space_before)) in line.iter().enumerate() {
        if *space_before && k > 0 {
            text.push(' ');
            stops.push(line[k - 1].0.rect[2]);
        }
        text.push(glyph.ch);
        stops.extend(std::iter::repeat(glyph.rect[0]).take(glyph.ch.len_utf16()));
        union(&mut bounds, &glyph.rect);
    }
    stops.push(line.last()?.0.rect[2]);
    if text.trim().is_empty() {
        return None;
    }
    let [x0, y0, x1, y1] = from_upright(bounds, quarter, width, height);
    // An upright x back to the page axis the line runs along.
    let along = |u: f32| match quarter {
        2 => width - u,
        3 => height - u,
        _ => u,
    };
    let to = |v: f32| round(v as f64);
    Some(TextLine {
        text,
        x: to(x0),
        y: to(y0),
        width: to(x1 - x0),
        height: to(y1 - y0),
        vertical: quarter % 2 == 1,
        chars: stops.into_iter().map(|u| to(along(u))).collect(),
    })
}

// ── Reading-order layout ──────────────────────────────────────────────────────
//
// Adapted from PdfCraft's `crates/render/src/text.rs`
// (https://github.com/storytold/pdfcraft), used under the MIT License:
//
// Copyright (c) 2026 ArtCraft Team and the PdfCraft contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

/// Page box → the frame where text runs left to right, for text running
/// `quarter` × 90° clockwise from that in a `w` × `h` page (1: downwards).
fn to_upright(r: [f32; 4], quarter: usize, w: f32, h: f32) -> [f32; 4] {
    let [x0, y0, x1, y1] = r;
    match quarter {
        1 => [y0, w - x1, y1, w - x0],
        2 => [w - x1, h - y1, w - x0, h - y0],
        3 => [h - y1, x0, h - y0, x1],
        _ => r,
    }
}

/// The inverse of [`to_upright`].
fn from_upright(r: [f32; 4], quarter: usize, w: f32, h: f32) -> [f32; 4] {
    let [u0, v0, u1, v1] = r;
    match quarter {
        1 => [w - v1, u0, w - v0, u1],
        2 => [w - u1, h - v1, w - u0, h - v0],
        3 => [v0, h - u1, v1, h - u0],
        _ => r,
    }
}

fn is_rtl(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/// Scripts whose combining signs can sit apart from their base (Indic, Thai,
/// Lao, Myanmar, Khmer): never infer a space inside a tight cluster.
fn is_complex(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x0900..=0x0DFF | 0x0E00..=0x0EFF | 0x1000..=0x109F | 0x1780..=0x17FF)
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x3134F | 0x1100..=0x11FF)
}

struct Segment {
    idx: Vec<usize>,
    bbox: [f32; 4],
    h: f32,
}

impl Segment {
    fn cy(&self) -> f32 {
        (self.bbox[1] + self.bbox[3]) / 2.0
    }
}

fn union(a: &mut [f32; 4], b: &[f32; 4]) {
    a[0] = a[0].min(b[0]);
    a[1] = a[1].min(b[1]);
    a[2] = a[2].max(b[2]);
    a[3] = a[3].max(b[3]);
}

/// Upright glyphs (content order) → lines in reading order, each a run of
/// glyphs with a flag for a word space before it.
///
/// 0. Drop fake-bold duplicates: a character redrawn at almost the same place.
/// 1. Segments: consecutive glyphs on one baseline with no column-sized gap;
///    same-baseline segments that nearly touch merge (text drawn out of order).
/// 2. Blocks: segments stacked line-sized apart that overlap horizontally.
/// 3. Block order: take the top-most block, preferring one to its left that
///    overlaps it vertically, so columns read left, then right.
/// 4. Glyphs run left to right; right-to-left runs reverse into logical order.
///    A word space goes where a gap beats the line's typical letter gap.
fn layout(mut glyphs: Vec<TextGlyph>) -> Vec<Vec<(TextGlyph, bool)>> {
    let mut keep = vec![true; glyphs.len()];
    for i in 1..glyphs.len() {
        let g = &glyphs[i];
        let h = (g.rect[3] - g.rect[1]).max(0.1);
        for j in (i.saturating_sub(4)..i).rev() {
            let p = &glyphs[j];
            if keep[j]
                && p.ch == g.ch
                && (p.rect[0] - g.rect[0]).abs() < h * 0.2
                && (p.rect[1] - g.rect[1]).abs() < h * 0.2
            {
                keep[i] = false;
                break;
            }
        }
    }
    let mut kept = keep.iter();
    glyphs.retain(|_| *kept.next().unwrap_or(&true));
    let n = glyphs.len();
    if n == 0 {
        return Vec::new();
    }
    let height = |i: usize| (glyphs[i].rect[3] - glyphs[i].rect[1]).max(0.1);
    let cy = |i: usize| (glyphs[i].rect[1] + glyphs[i].rect[3]) / 2.0;

    // 1. Segments in content order.
    let mut segs: Vec<Segment> = Vec::new();
    for i in 0..n {
        let g = glyphs[i].rect;
        let cont = segs.last().and_then(|s| s.idx.last()).is_some_and(|&p| {
            let ph = height(p).min(height(i));
            let gap = g[0] - glyphs[p].rect[2];
            (cy(i) - cy(p)).abs() < ph * 0.5
                && gap < ph * 3.0
                && g[0] > glyphs[p].rect[0] - ph * 2.0
        });
        match segs.last_mut() {
            Some(s) if cont => {
                s.idx.push(i);
                union(&mut s.bbox, &g);
                s.h = s.h.max(height(i));
            }
            _ => segs.push(Segment {
                idx: vec![i],
                bbox: g,
                h: height(i),
            }),
        }
    }
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for a in 0..segs.len() {
            for b in 0..segs.len() {
                if a == b {
                    continue;
                }
                let (sa, sb) = (&segs[a], &segs[b]);
                let h = sa.h.min(sb.h);
                let gap = (sb.bbox[0] - sa.bbox[2]).max(sa.bbox[0] - sb.bbox[2]);
                if (sa.cy() - sb.cy()).abs() < h * 0.3
                    && (sa.h / sb.h - 1.0).abs() < 0.35
                    && gap < h * 1.2
                {
                    let sb = segs.remove(b);
                    let a = if b < a { a - 1 } else { a };
                    let sa = &mut segs[a];
                    sa.idx.extend(sb.idx);
                    union(&mut sa.bbox, &sb.bbox);
                    sa.h = sa.h.max(sb.h);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    for s in &mut segs {
        s.idx
            .sort_by(|a, b| glyphs[*a].rect[0].total_cmp(&glyphs[*b].rect[0]));
    }

    // 2. Blocks.
    let mut by_top: Vec<usize> = (0..segs.len()).collect();
    by_top.sort_by(|a, b| segs[*a].bbox[1].total_cmp(&segs[*b].bbox[1]));
    let mut blocks: Vec<(Vec<usize>, [f32; 4])> = Vec::new();
    for si in by_top {
        let s = &segs[si];
        let target = blocks.iter().position(|(members, bb)| {
            let Some(&m) = members.last() else {
                return false;
            };
            let last = &segs[m];
            let vgap = s.bbox[1] - last.bbox[3];
            let overlap = s.bbox[2].min(bb[2]) - s.bbox[0].max(bb[0]);
            let minw = (s.bbox[2] - s.bbox[0]).min(bb[2] - bb[0]).max(1.0);
            vgap > -last.h * 0.5
                && vgap < last.h.max(s.h) * 1.1
                && overlap > minw * 0.3
                && (last.h / s.h - 1.0).abs() < 0.6
        });
        match target {
            Some(b) => {
                blocks[b].0.push(si);
                union(&mut blocks[b].1, &s.bbox);
            }
            None => blocks.push((vec![si], s.bbox)),
        }
    }

    // 3. Block order.
    let mut remaining: Vec<usize> = (0..blocks.len()).collect();
    let mut block_order = Vec::with_capacity(blocks.len());
    while let Some(&top) = remaining
        .iter()
        .min_by(|a, b| blocks[**a].1[1].total_cmp(&blocks[**b].1[1]))
    {
        let tb = blocks[top].1;
        let pick = remaining
            .iter()
            .copied()
            .filter(|c| {
                let cb = blocks[*c].1;
                let v = cb[3].min(tb[3]) - cb[1].max(tb[1]);
                cb[2] <= tb[0] + 1.0 && v > 0.5 * (cb[3] - cb[1]).min(tb[3] - tb[1])
            })
            .min_by(|a, b| blocks[*a].1[0].total_cmp(&blocks[*b].1[0]))
            .unwrap_or(top);
        remaining.retain(|r| *r != pick);
        block_order.push(pick);
    }

    // 4. Lines in reading order, with word gaps.
    let mut lines = Vec::new();
    for b in block_order {
        for si in &blocks[b].0 {
            let s = &segs[*si];
            let mut idx = s.idx.clone();
            let rtl = |i: usize| is_rtl(glyphs[i].ch);
            let mut k = 0;
            while k < idx.len() {
                if rtl(idx[k]) {
                    let mut e = k;
                    while e + 1 < idx.len()
                        && (rtl(idx[e + 1])
                            || (glyphs[idx[e + 1]].ch.is_whitespace()
                                && e + 2 < idx.len()
                                && rtl(idx[e + 2])))
                    {
                        e += 1;
                    }
                    idx[k..=e].reverse();
                    k = e + 1;
                } else {
                    k += 1;
                }
            }
            // Word gaps relative to this line's typical letter gap (handles tracking).
            let mut gaps: Vec<f32> = s
                .idx
                .windows(2)
                .map(|w| glyphs[w[1]].rect[0] - glyphs[w[0]].rect[2])
                .collect();
            gaps.sort_by(f32::total_cmp);
            let typical = gaps.get(gaps.len() / 3).copied().unwrap_or(0.0).max(0.0);
            let threshold = (typical + s.h * 0.15).max(s.h * 0.15);
            let mut spaces = vec![false; idx.len()];
            for w in 1..s.idx.len() {
                let (p, c) = (s.idx[w - 1], s.idx[w]);
                let gap = glyphs[c].rect[0] - glyphs[p].rect[2];
                let (pc, cc) = (glyphs[p].ch, glyphs[c].ch);
                let cjk = is_cjk(pc) && is_cjk(cc) && gap < s.h * 0.5;
                let tight_cluster = (is_complex(pc) || is_complex(cc)) && gap < s.h * 0.6;
                if gap > threshold
                    && !cjk
                    && !tight_cluster
                    && !pc.is_whitespace()
                    && !cc.is_whitespace()
                {
                    // The space goes before whichever of the pair reads second
                    // once right-to-left runs are reversed.
                    let at = |g: usize| idx.iter().position(|x| *x == g);
                    if let (Some(pp), Some(cp)) = (at(p), at(c)) {
                        spaces[pp.max(cp)] = true;
                    }
                }
            }
            lines.push(
                idx.iter()
                    .enumerate()
                    .map(|(k, g)| (glyphs[*g].clone(), k > 0 && spaces[k]))
                    .collect(),
            );
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use lopdf::{dictionary, Document, Object as Lo, Stream};

    /// Two US-letter pages. Page 1 says "Hello world" in Helvetica and holds a
    /// URI link, a `/Dest` link to page 2 and a named GoTo link to page 2.
    /// Page 2 is rotated 90° and links back to page 1.
    fn sample_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page1 = doc.new_object_id();
        let page2 = doc.new_object_id();
        let font = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let rect = |x0: i64, y0: i64, x1: i64, y1: i64| -> Lo {
            vec![x0.into(), y0.into(), x1.into(), y1.into()].into()
        };
        let uri = doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(72, 690, 200, 730),
            "A" => dictionary! { "S" => "URI", "URI" => Lo::string_literal("https://example.com/a") },
        });
        let goto = doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(72, 600, 150, 620),
            "Dest" => vec![Lo::Reference(page2), "Fit".into()],
        });
        let named = doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(72, 500, 150, 520),
            "A" => dictionary! { "S" => "GoTo", "D" => Lo::string_literal("second") },
        });
        let back = doc.add_object(dictionary! {
            "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(0, 0, 100, 50),
            "Dest" => vec![Lo::Reference(page1), "Fit".into()],
        });
        let text = doc.add_object(Stream::new(
            dictionary! {},
            b"BT /F1 24 Tf 72 700 Td (Hello world) Tj ET".to_vec(),
        ));
        let empty = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
        let media = rect(0, 0, 612, 792);
        doc.objects.insert(
            page1,
            Lo::Dictionary(dictionary! {
                "Type" => "Page", "Parent" => pages_id, "MediaBox" => media.clone(),
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
                "Contents" => text,
                "Annots" => vec![Lo::Reference(uri), Lo::Reference(goto), Lo::Reference(named)],
            }),
        );
        doc.objects.insert(
            page2,
            Lo::Dictionary(dictionary! {
                "Type" => "Page", "Parent" => pages_id, "MediaBox" => media, "Rotate" => 90,
                "Contents" => empty, "Annots" => vec![Lo::Reference(back)],
            }),
        );
        doc.objects.insert(pages_id, Lo::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Lo::Reference(page1), Lo::Reference(page2)], "Count" => 2,
        }));
        let dests = doc.add_object(dictionary! {
            "Names" => vec![Lo::string_literal("second"), vec![Lo::Reference(page2), "Fit".into()].into()],
        });
        let catalog = doc.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => pages_id,
            "Names" => dictionary! { "Dests" => dests },
        });
        doc.trailer.set("Root", catalog);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    fn sample() -> Pdf {
        Pdf::new(Arc::new(sample_pdf())).unwrap()
    }

    /// One US-letter page drawing `content` with Helvetica as /F1.
    fn one_page(content: &str) -> Pdf {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let stream = doc.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            "Contents" => stream,
        });
        doc.objects.insert(
            pages_id,
            Lo::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![Lo::Reference(page)], "Count" => 1,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        Pdf::new(Arc::new(bytes)).unwrap()
    }

    fn texts(pdf: &Pdf) -> Vec<String> {
        page_text(pdf, 1)
            .unwrap()
            .lines
            .into_iter()
            .map(|line| line.text)
            .collect()
    }

    #[test]
    fn pdf_view_text_drops_zero_scale_glyphs() {
        let pdf = one_page(
            "BT /F1 24 Tf 1 0 0 1 72 700 Tm (Seen) Tj \
             0 0 0 0 72 600 Tm (latexi sha1_base64) Tj \
             1 0 0 1 72 500 Tm 0 Tz (squashed) Tj ET",
        );
        assert_eq!(texts(&pdf), ["Seen"]);
    }

    #[test]
    fn pdf_view_text_drops_glyphs_off_the_page() {
        let pdf = one_page(
            "BT /F1 24 Tf 1 0 0 1 72 700 Tm (Seen) Tj \
             1 0 0 1 -3000 600 Tm (overlay) Tj \
             1 0 0 1 72 3000 Tm (above) Tj ET",
        );
        assert_eq!(texts(&pdf), ["Seen"]);
    }

    #[test]
    fn pdf_view_paths_stay_inside_the_library_roots() {
        let root = Path::new("/data");
        assert_eq!(
            resolve_in(root, "courses/X/files/a.pdf"),
            Ok(PathBuf::from("/data/courses/X/files/a.pdf"))
        );
        assert!(resolve_in(root, "lectures/a.pdf").is_ok());
        assert!(resolve_in(root, "agents/a.pdf").is_ok());
        for bad in [
            "",
            "/etc/passwd",
            "../a.pdf",
            "courses/../../a.pdf",
            "data/a.pdf",
            "oculus.db",
        ] {
            assert_eq!(
                resolve_in(root, bad),
                Err("outside-library".to_string()),
                "{bad}"
            );
        }
    }

    #[test]
    fn pdf_view_opens_through_the_cache_and_reports_errors() {
        let root = Scratch::new("pdf-view-open");
        std::fs::create_dir_all(root.join("courses/X")).unwrap();
        std::fs::write(root.join("courses/X/a.pdf"), sample_pdf()).unwrap();
        std::fs::write(root.join("courses/X/bad.pdf"), b"not a pdf").unwrap();

        let pdf = open_at(&root, "courses/X/a.pdf").unwrap();
        let again = open_at(&root, "courses/X/a.pdf").unwrap();
        assert!(
            Arc::ptr_eq(&pdf, &again),
            "an unchanged file comes from the cache"
        );
        let sizes = page_sizes(&pdf).pages;
        assert_eq!(sizes.len(), 2);
        assert_eq!((sizes[0].width, sizes[0].height), (612.0, 792.0));
        assert_eq!(
            (sizes[1].width, sizes[1].height),
            (792.0, 612.0),
            "rotation swaps the sides"
        );

        assert_eq!(
            open_at(&root, "courses/X/missing.pdf").err().as_deref(),
            Some("not-found")
        );
        assert_eq!(
            open_at(&root, "courses/X/bad.pdf").err().as_deref(),
            Some("invalid")
        );
        assert_eq!(
            open_at(&root, "../a.pdf").err().as_deref(),
            Some("outside-library")
        );
        assert_eq!(
            page_text(&pdf, 3).err().as_deref(),
            Some("page-out-of-range")
        );
        assert_eq!(
            page_text(&pdf, 0).err().as_deref(),
            Some("page-out-of-range")
        );
    }

    #[test]
    fn pdf_view_renders_exactly_the_requested_size() {
        let pdf = sample();
        for (width, height) in [(300, 388), (612, 792), (1001, 1297), (97, 13)] {
            let rgba = render_page(&pdf, 1, width, height).unwrap();
            assert_eq!(
                rgba.len(),
                (width * height * 4) as usize,
                "{width}x{height}"
            );
            assert!(rgba.chunks_exact(4).all(|px| px[3] == 255), "opaque");
        }
        let rgba = render_page(&pdf, 1, 612, 792).unwrap();
        let dark = rgba.chunks_exact(4).filter(|px| px[0] < 128).count();
        assert!(dark > 100, "the text paints: {dark} dark pixels");
        // The text sits at x 72..~200, baseline y 92 from the top: nothing
        // to its right on that row band.
        let px = |x: usize, y: usize| rgba[(y * 612 + x) * 4];
        assert!((70..100).any(|y| (72..200).any(|x| px(x, y) < 128)));
        assert!((70..100).all(|y| (400..612).all(|x| px(x, y) == 255)));

        assert_eq!(
            render_page(&pdf, 1, 0, 10).err().as_deref(),
            Some("invalid-size")
        );
        assert_eq!(
            render_page(&pdf, 1, 8193, 10).err().as_deref(),
            Some("too-large")
        );
        assert_eq!(
            render_page(&pdf, 1, 8000, 8000).err().as_deref(),
            Some("too-large")
        );
    }

    #[test]
    fn pdf_view_a_panic_inside_hayro_fails_only_its_request() {
        assert_eq!(
            guarded(|| -> u8 { panic!("malformed") }),
            Err("render-failed".to_string())
        );
        assert_eq!(guarded(|| 7), Ok(7));
    }

    #[test]
    fn pdf_view_scale_truncates_to_the_requested_pixels() {
        for side in [612.0f32, 792.0, 595.28, 841.89, 1.0, 13.7] {
            for pixels in [1u32, 13, 97, 300, 1001, 1600, 4096, 8192] {
                let scale = scale_to(side, pixels);
                assert_eq!((side * scale) as u16 as u32, pixels, "{side} -> {pixels}");
            }
        }
    }

    #[test]
    fn pdf_view_text_reads_hello_world_with_stops() {
        let pdf = sample();
        let text = page_text(&pdf, 1).unwrap();
        assert_eq!(text.lines.len(), 1, "{:?}", text.lines);
        let line = &text.lines[0];
        assert_eq!(line.text, "Hello world");
        assert!(!line.vertical);
        assert_eq!(line.chars.len(), line.text.encode_utf16().count() + 1);
        assert!(
            line.chars.windows(2).all(|w| w[0] <= w[1]),
            "{:?}",
            line.chars
        );
        assert!((line.chars[0] - 72.0).abs() < 0.5, "{:?}", line.chars);
        assert!((line.x - 72.0).abs() < 0.5);
        // Em box of a 24pt font with its baseline 92pt from the top.
        assert!((line.y - (92.0 - 19.2)).abs() < 0.5, "{}", line.y);
        assert!((line.height - 24.0).abs() < 0.5, "{}", line.height);
        assert!((line.x + line.width - line.chars[line.chars.len() - 1]).abs() < 0.01);
        assert!(page_text(&pdf, 2).unwrap().lines.is_empty());
    }

    #[test]
    fn pdf_view_resolves_links() {
        let pdf = sample();
        let links = page_links(&pdf, 1).unwrap();
        assert_eq!(links.len(), 3, "{links:?}");
        assert_eq!(links[0].uri.as_deref(), Some("https://example.com/a"));
        assert_eq!(
            (links[0].x, links[0].y, links[0].width, links[0].height),
            (72.0, 62.0, 128.0, 40.0)
        );
        assert_eq!((links[1].page, links[1].uri.as_deref()), (Some(2), None));
        assert_eq!(links[2].page, Some(2), "named destination");
        // Page 2 is rotated 90° clockwise: its bottom-left corner box
        // lands at the rendered page's top-left.
        let back = page_links(&pdf, 2).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].page, Some(1));
        assert_eq!(
            (back[0].x, back[0].y, back[0].width, back[0].height),
            (0.0, 0.0, 50.0, 100.0)
        );
    }

    #[test]
    fn pdf_view_layout_reads_columns_and_word_gaps() {
        fn word(v: &mut Vec<TextGlyph>, s: &str, x: f32, y: f32, advance: f32) {
            for (i, ch) in s.chars().enumerate() {
                let x0 = x + i as f32 * advance;
                v.push(TextGlyph {
                    ch,
                    rect: [x0, y, x0 + 6.0, y + 10.0],
                });
            }
        }
        let text = |v: Vec<TextGlyph>| -> Vec<String> {
            layout(v)
                .iter()
                .map(|line| {
                    line.iter()
                        .map(|(g, space)| {
                            if *space {
                                format!(" {}", g.ch)
                            } else {
                                g.ch.to_string()
                            }
                        })
                        .collect()
                })
                .collect()
        };
        let mut v = Vec::new();
        word(&mut v, "Left1", 10.0, 10.0, 6.0);
        word(&mut v, "Right1", 200.0, 10.0, 6.0);
        word(&mut v, "Left2", 10.0, 24.0, 6.0);
        word(&mut v, "Right2", 200.0, 24.0, 6.0);
        assert_eq!(text(v), ["Left1", "Left2", "Right1", "Right2"]);

        let mut v = Vec::new();
        word(&mut v, "CHAPTER", 10.0, 10.0, 9.0);
        word(&mut v, "FOUR", 10.0 + 7.0 * 9.0 + 5.0, 10.0, 9.0);
        assert_eq!(text(v), ["CHAPTER FOUR"]);

        // Drawn right word first: the space still lands between the words.
        let mut v = Vec::new();
        word(&mut v, "two", 34.0, 10.0, 6.0);
        word(&mut v, "one", 10.0, 10.0, 6.0);
        assert_eq!(text(v), ["one two"]);

        let mut v = Vec::new();
        for (i, ch) in "Bold".chars().enumerate() {
            let x = 10.0 + i as f32 * 7.0;
            v.push(TextGlyph {
                ch,
                rect: [x, 10.0, x + 6.0, 20.0],
            });
            v.push(TextGlyph {
                ch,
                rect: [x + 0.3, 10.0, x + 6.3, 20.0],
            });
        }
        assert_eq!(text(v), ["Bold"]);
    }

    /// Times a real slide deck from the live library (read-only):
    /// `cargo test pdf_view_bench -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn pdf_view_bench_a_real_deck() {
        fn find(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    find(&path, out);
                } else if path.extension().is_some_and(|ext| ext == "pdf") {
                    out.push(path);
                }
            }
        }
        let mut found = Vec::new();
        find(&crate::paths::data_dir().join("courses"), &mut found);
        found.sort();
        let wanted = std::env::var("PDF_VIEW_BENCH").ok();
        let deck = found
            .iter()
            .filter(|path| {
                wanted
                    .as_deref()
                    .map_or(true, |w| path.to_string_lossy().contains(w))
            })
            .find_map(|path| {
                let pdf = Pdf::new(Arc::new(std::fs::read(path).ok()?)).ok()?;
                let page = pdf.pages().first()?;
                let (w, h) = page.render_dimensions();
                (pdf.pages().len() >= 10 && w > h).then(|| (path.clone(), pdf))
            })
            .expect("a landscape PDF of 10+ pages under courses/");
        let (path, pdf) = deck;
        println!("deck: {} ({} pages)", path.display(), pdf.pages().len());
        let start = std::time::Instant::now();
        let bytes = std::fs::read(&path).unwrap();
        let reopened = Pdf::new(Arc::new(bytes)).unwrap();
        println!("open: {:.1} ms", start.elapsed().as_secs_f64() * 1e3);
        drop(reopened);

        let pages = 1..=pdf.pages().len().min(10) as u32;
        let mut render_ms = Vec::new();
        let mut text_ms = Vec::new();
        for page in pages.clone() {
            let (w, h) = pdf.pages()[page as usize - 1].render_dimensions();
            let height = (1600.0 * h / w).round() as u32;
            let start = std::time::Instant::now();
            let rgba = render_page(&pdf, page, 1600, height).unwrap();
            render_ms.push(start.elapsed().as_secs_f64() * 1e3);
            assert_eq!(rgba.len(), (1600 * height * 4) as usize);
            if let Some(dir) = std::env::var_os("PDF_VIEW_BENCH_OUT") {
                let image = image::RgbaImage::from_raw(1600, height, rgba).unwrap();
                image
                    .save(Path::new(&dir).join(format!("page-{page}.png")))
                    .unwrap();
            }
            let start = std::time::Instant::now();
            let text = page_text(&pdf, page).unwrap();
            text_ms.push(start.elapsed().as_secs_f64() * 1e3);
            if page == 1 {
                println!(
                    "page 1 at 1600x{height}, {} lines; first: {:?}",
                    text.lines.len(),
                    text.lines.first().map(|l| &l.text)
                );
            }
            if let Some(dir) = std::env::var_os("PDF_VIEW_BENCH_OUT") {
                let lines: Vec<&str> = text.lines.iter().map(|line| line.text.as_str()).collect();
                std::fs::write(
                    Path::new(&dir).join(format!("page-{page}.txt")),
                    lines.join("\n"),
                )
                .unwrap();
            }
        }
        println!("render ms (fresh cache) per page: {render_ms:.1?}");
        println!("text ms per page: {text_ms:.1?}");

        // The same pages through one shared RenderCache, as a per-thread
        // cache would see them.
        let cache = RenderCache::new();
        let mut shared_ms = Vec::new();
        for page in pages {
            let page = &pdf.pages()[page as usize - 1];
            let (w, h) = page.render_dimensions();
            let height = (1600.0 * h / w).round() as u32;
            let settings = PixmapSettings {
                x_scale: scale_to(w, 1600),
                y_scale: scale_to(h, height),
                bg_color: WHITE,
            };
            let start = std::time::Instant::now();
            let _ = hayro::render(
                page,
                &cache,
                &InterpreterSettings::default(),
                &RenderSettings::default(),
                &settings,
            );
            shared_ms.push(start.elapsed().as_secs_f64() * 1e3);
        }
        println!("render ms (shared cache) per page: {shared_ms:.1?}");
    }
}
