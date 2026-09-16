//! Turning MinerU's `content_list.json` into page records — a behaviour-exact
//! port of `sidecar/mineru_render.py`.
//!
//! This module decides what the markdown *says*, so it is the one place in the
//! parse path where a subtle divergence would silently rewrite the library
//! rather than fail. Every rule below was ported against the Python by
//! differential test, and the tests at the bottom pin the Python's own output
//! so the pinning outlives the sidecar's deletion.
//!
//! Nothing here knows whether the content list came from the cloud API or a
//! local parse server: both backends emit the same legacy content-list shape,
//! which is exactly why the rendering lives on this side of the seam.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::parse::{ParseError, ParsePage};

/// A header/footer repeated on at least this fraction of a window's pages is
/// template furniture — a running head, a unit code, a university name — not
/// content. Measured per document rather than hardcoded, because every faculty
/// has its own.
const BOILERPLATE_PAGE_RATIO: f64 = 0.5;

/// Below this many pages the ratio means nothing: on a 2-page handout a title
/// that happens to appear twice is not furniture, it is the title.
const BOILERPLATE_MIN_PAGES: i64 = 4;

/// Equations may also carry an `img_path`, but are rendered as LaTeX.
const IMAGE_TYPES: [&str; 3] = ["image", "chart", "table"];

/// Crops render at ~1.5x: 20k px² rejects small template furniture while
/// retaining the smallest real figure in the measured deck (58k px²).
const MIN_IMAGE_AREA: u64 = 20_000;

/// Boilerplate is detected in fixed 64-page windows, and **this number is
/// bug-compatibility, not a heuristic.**
///
/// It preserves the legacy parser's boilerplate decisions independently of how
/// the backend happened to chunk the document: cloud tasks are 200 pages, the
/// old local pipeline's memory-retry chunks were 16, and all of them must
/// render identically. Changing 64 changes the markdown of documents already
/// in the library — with no version bump to signal it, because
/// `PARSER_VERSION` guards the artifact *shape*, not the text.
const RENDER_GROUP_PAGES: i64 = 64;

// ── Content-list accessors ───────────────────────────────────────────────────
//
// The content list is untyped JSON from a backend we do not control, so every
// read has a default and none of them can fail. The Python used `dict.get`
// with the same defaults; a missing key must produce a rendering decision, not
// an error.

fn kind_of(item: &Value) -> &str {
    item.get("type").and_then(Value::as_str).unwrap_or("")
}

fn text_of(item: &Value) -> &str {
    item.get("text").and_then(Value::as_str).unwrap_or("")
}

fn page_idx(item: &Value) -> i64 {
    item.get("page_idx").and_then(Value::as_i64).unwrap_or(0)
}

/// `img_path`, when it is a non-empty string. The Python tested it for
/// truthiness, so an empty path falls through to `table_body` rather than
/// producing `![](prefix/)`.
fn img_path(item: &Value) -> Option<&str> {
    item.get("img_path").and_then(Value::as_str).filter(|path| !path.is_empty())
}

/// Python truthiness, which is what `item.get("text_level")` was tested for:
/// `text_level: 0` is a level-less item, `text_level: 3` is a heading.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64().is_none_or(|n| n != 0.0),
        Some(Value::String(text)) => !text.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(fields)) => !fields.is_empty(),
    }
}

/// `Path(value).name` — the link written into the markdown is the basename, so
/// the backend's own directory layout inside its result archive never leaks.
fn basename(path: &str) -> String {
    Path::new(path).file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `" ".join(item[key] or []).strip()`, for the caption/footnote lists.
fn join_parts(item: &Value, key: &str) -> String {
    let Some(parts) = item.get(key).and_then(Value::as_array) else {
        return String::new();
    };
    parts
        .iter()
        .map(|part| part.as_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// The comparison form for boilerplate: whitespace-collapsed and lowercased,
/// so a running head that reflows or changes case across pages still counts as
/// the same string.
fn norm(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

// ── Rendering one item ───────────────────────────────────────────────────────

/// Convert one content-list entry to a markdown block, or drop it.
///
/// The order of these branches is load-bearing; two of the interactions look
/// like bugs and are deliberately preserved, because changing either rewrites
/// markdown that is already in the library:
///
/// * a `table` carrying **both** `img_path` and `table_body` loses the HTML
///   body — the image wins, and the caption survives only as alt text;
/// * a `footer` with a truthy `text_level` is caught by the heading branch
///   before the footer branch and becomes a `##` heading rather than being
///   dropped.
fn render_item(
    item: &Value,
    images_rel: &str,
    dropped: &HashSet<String>,
    boilerplate: &HashSet<String>,
) -> Option<String> {
    let kind = kind_of(item);

    // MinerU emits its own `$$` delimiters inside the equation text; wrapping
    // it again here produced `$$$$` in the rendered markdown.
    if kind == "equation" {
        let text = text_of(item).trim();
        return (!text.is_empty()).then(|| text.to_string());
    }

    if IMAGE_TYPES.contains(&kind) {
        let caption = join_parts(item, &format!("{kind}_caption"));
        let footnote = join_parts(item, &format!("{kind}_footnote"));

        let block = match img_path(item) {
            // Dropped by the size filter: the caption goes with it, since a
            // caption with nothing above it reads as a stray line of prose.
            Some(path) if dropped.contains(&basename(path)) => String::new(),
            Some(path) => format!("![{caption}]({images_rel}/{})", basename(path)),
            None => item.get("table_body").and_then(Value::as_str).unwrap_or("").trim().to_string(),
        };

        let joined = [block, footnote]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        return (!joined.is_empty()).then_some(joined);
    }

    let text = text_of(item).trim();
    if text.is_empty() || boilerplate.contains(&norm(text)) {
        return None;
    }

    // One fixed heading level whatever `text_level`'s number is: MinerU's
    // levels are per-page guesses and produced a document whose heading
    // hierarchy jumped around between slides.
    if kind == "header" || truthy(item.get("text_level")) {
        return Some(format!("## {text}"));
    }
    if kind == "footer" {
        return None;
    }
    Some(text.to_string())
}

// ── Ordering ─────────────────────────────────────────────────────────────────

/// Put headers first while preserving MinerU's body reading order.
///
/// Every non-header gets the *same* key, so the sort must be stable — that is
/// the only thing keeping the backend's reading order for body content. Rust's
/// `sort_by` is stable; `sort_unstable_by` here would shuffle prose.
fn sort_key(item: &Value) -> (u8, f64, f64) {
    if kind_of(item) != "header" {
        return (1, 0.0, 0.0);
    }
    let bbox = item.get("bbox").and_then(Value::as_array).filter(|values| !values.is_empty());
    let at = |index: usize| {
        bbox.and_then(|values| values.get(index)).and_then(Value::as_f64).unwrap_or(0.0)
    };
    (0, at(1), at(0))
}

fn compare(left: &Value, right: &Value) -> Ordering {
    let (a, b) = (sort_key(left), sort_key(right));
    a.0.cmp(&b.0)
        // A NaN coordinate is not orderable; treating it as equal keeps the
        // stable order rather than panicking on a malformed bbox.
        .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
        .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
}

// ── Boilerplate ──────────────────────────────────────────────────────────────

/// The normalised header/footer strings repeated across most of a window's
/// pages. Counts **distinct** pages, not occurrences: a footer printed three
/// times on one slide is not on three pages.
fn find_boilerplate(items: &[&Value], window_pages: i64) -> HashSet<String> {
    if window_pages < BOILERPLATE_MIN_PAGES {
        return HashSet::new();
    }

    let mut pages_with: HashMap<String, HashSet<i64>> = HashMap::new();
    for item in items {
        let kind = kind_of(item);
        if kind != "header" && kind != "footer" {
            continue;
        }
        let key = norm(text_of(item));
        if !key.is_empty() {
            pages_with.entry(key).or_default().insert(page_idx(item));
        }
    }

    let threshold = window_pages as f64 * BOILERPLATE_PAGE_RATIO;
    pages_with
        .into_iter()
        .filter(|(_, pages)| pages.len() as f64 >= threshold)
        .map(|(key, _)| key)
        .collect()
}

// ── The renderer ─────────────────────────────────────────────────────────────

/// Render a backend result and copy only the referenced, useful image crops.
///
/// `content_list` items must already carry **absolute** `page_idx` values —
/// a backend that split the document into tasks rebases them before calling,
/// because the 64-page windowing below is defined over the whole document and
/// would otherwise restart at every task boundary.
///
/// `source_images` is the `images` directory inside the extracted result;
/// `images_dir` is the staging directory to copy the keepers into; and
/// `images_rel` is the link prefix written into the markdown. Returns the page
/// records and the number of images actually copied.
pub fn render(
    content_list: &[Value],
    total_pages: u32,
    source_images: &Path,
    images_dir: &Path,
    images_rel: &str,
) -> Result<(Vec<ParsePage>, u32), ParseError> {
    // 1. Which crops the markdown will reference. A `BTreeSet` both dedupes
    //    and gives the sorted copy order the Python had, so a run over the
    //    same result touches the disk in the same order twice.
    let wanted: BTreeSet<String> = content_list
        .iter()
        .filter(|item| IMAGE_TYPES.contains(&kind_of(item)))
        .filter_map(|item| img_path(item).map(basename))
        .collect();

    let mut dropped: HashSet<String> = HashSet::new();
    let mut image_count = 0u32;
    if !wanted.is_empty() && source_images.is_dir() {
        fs::create_dir_all(images_dir)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", images_dir.display())))?;
        for name in &wanted {
            let source = source_images.join(name);
            // A crop the backend named but did not ship is skipped silently
            // and not counted; its item still renders a link, which is what
            // the Python did and what the markdown on disk already contains.
            if !source.is_file() {
                continue;
            }
            if too_small(&source) {
                dropped.insert(name.clone());
                continue;
            }
            fs::copy(&source, images_dir.join(name)).map_err(|e| {
                ParseError::Io(format!("copy {} -> {}: {e}", source.display(), images_dir.display()))
            })?;
            image_count += 1;
        }
    }

    // 2. Partition into fixed 64-page windows and detect boilerplate inside
    //    each one, against that window's own page count — the last window is
    //    short, and a short window skips detection entirely.
    let mut windows: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
    for item in content_list {
        windows.entry(page_idx(item).div_euclid(RENDER_GROUP_PAGES)).or_default().push(item);
    }

    let mut by_page: BTreeMap<i64, String> = BTreeMap::new();
    for (window, items) in windows {
        let window_pages =
            RENDER_GROUP_PAGES.min(total_pages as i64 - window * RENDER_GROUP_PAGES);
        let boilerplate = find_boilerplate(&items, window_pages);

        let mut per_page: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
        for item in items {
            per_page.entry(page_idx(item) + 1).or_default().push(item);
        }
        for (page_no, mut page_items) in per_page {
            page_items.sort_by(|left, right| compare(left, right));
            let blocks: Vec<String> = page_items
                .iter()
                .filter_map(|item| render_item(item, images_rel, &dropped, &boilerplate))
                .collect();
            if !blocks.is_empty() {
                // Windows partition by page, so no two of them can write the
                // same page number.
                by_page.insert(page_no, blocks.join("\n\n"));
            }
        }
    }

    // 3. Blank and furniture-only pages still need records: `page_no` is the
    //    citation and deep-link join key, and a hole would misalign every page
    //    after it. `ParseOutput::new` gap-fills too; that is belt-and-braces,
    //    not a reason to hand it holes.
    for page_no in 1..=i64::from(total_pages) {
        by_page.entry(page_no).or_default();
    }

    let pages = by_page
        .into_iter()
        // A backend with a broken offset could hand us a page_idx below the
        // first page; those records have nowhere to attach, and the seam would
        // drop them anyway.
        .filter(|(page_no, _)| *page_no >= 1)
        .map(|(page_no, markdown)| ParsePage { page_no: page_no as u32, markdown })
        .collect();
    Ok((pages, image_count))
}

/// Is this crop too small to be a real figure?
///
/// `imagesize` reads the dimensions out of the file header rather than
/// decoding the pixels, which is the whole reason PIL is not missed here.
/// An unreadable image **fails open** — losing a real figure is worse than
/// keeping a piece of template furniture.
fn too_small(path: &Path) -> bool {
    match imagesize::size(path) {
        Ok(size) => (size.width as u64) * (size.height as u64) < MIN_IMAGE_AREA,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Every expected value in this module was produced by running
    /// `sidecar/mineru_render.py` on the identical input and inlining what it
    /// returned. When the sidecar is deleted these become the only record of
    /// what the library's existing markdown was rendered by.
    fn rendered(content: &[Value], total_pages: u32) -> Vec<ParsePage> {
        let nowhere = Path::new("/nonexistent-source-images");
        let (pages, count) = render(content, total_pages, nowhere, nowhere, "deck_images").unwrap();
        assert_eq!(count, 0);
        pages
    }

    fn markdown(pages: &[ParsePage]) -> Vec<&str> {
        pages.iter().map(|page| page.markdown.as_str()).collect()
    }

    /// The 64-page windowing, pinned from `sidecar/test_mineru_render.py`: a
    /// header on pages 0-31 hits exactly the 32-page threshold of the first
    /// full window and is stripped, while an identical-shaped header on page
    /// 129 lands in a 2-page window where detection is skipped entirely.
    #[test]
    fn boilerplate_is_detected_per_64_page_window() {
        let mut content: Vec<Value> = (0..32)
            .map(|page| json!({"type": "header", "text": "Repeated section", "page_idx": page}))
            .collect();
        content.push(json!({"type": "header", "text": "Last title", "page_idx": 129}));

        let pages = rendered(&content, 130);
        assert_eq!(pages.len(), 130);
        assert!(pages[..32].iter().all(|page| page.markdown.is_empty()));
        assert_eq!(pages[129].markdown, "## Last title");
        assert_eq!(pages[129].page_no, 130);
    }

    /// One page short of the window's threshold keeps the header: the ratio is
    /// `>=`, and 31 of 64 pages is furniture nobody asked us to remove.
    #[test]
    fn one_page_short_of_the_threshold_is_not_boilerplate() {
        let content: Vec<Value> = (0..31)
            .map(|page| json!({"type": "header", "text": "Repeated section", "page_idx": page}))
            .collect();
        let pages = rendered(&content, 130);
        assert_eq!(pages[0].markdown, "## Repeated section");
        assert_eq!(pages[30].markdown, "## Repeated section");
    }

    /// A window under four pages skips detection, however repetitive it is.
    #[test]
    fn a_short_document_never_detects_boilerplate() {
        let content: Vec<Value> = (0..3)
            .map(|page| json!({"type": "footer", "text": "University", "page_idx": page}))
            .collect();
        // Footers are dropped anyway; what this pins is that they are dropped
        // by the footer rule and not by boilerplate detection.
        assert_eq!(markdown(&rendered(&content, 3)), vec!["", "", ""]);
    }

    /// Stable ordering with a header after body text, plus the image block and
    /// blank-page handling — the Python's second pinned test, minus the image
    /// copy (covered separately).
    #[test]
    fn headers_lead_the_page_and_blank_pages_keep_records() {
        let content = vec![
            json!({"type": "text", "text": "Body", "page_idx": 0}),
            json!({"type": "header", "text": "Title", "page_idx": 0, "bbox": [0, 10, 1, 20]}),
            json!({
                "type": "chart",
                "page_idx": 1,
                "img_path": "images/chart.png",
                "chart_caption": ["A chart"],
                "chart_footnote": ["Explaining prose"],
            }),
            json!({"type": "footer", "text": "University", "page_idx": 0}),
            json!({"type": "footer", "text": "University", "page_idx": 1}),
            json!({"type": "footer", "text": "University", "page_idx": 2}),
            json!({"type": "footer", "text": "University", "page_idx": 3}),
        ];
        assert_eq!(
            markdown(&rendered(&content, 4)),
            vec![
                "## Title\n\nBody",
                "![A chart](deck_images/chart.png)\n\nExplaining prose",
                "",
                "",
            ]
        );
    }

    /// Body order is MinerU's; only headers are lifted, and they sort by
    /// `bbox[1]` then `bbox[0]` among themselves.
    #[test]
    fn body_keeps_the_backends_reading_order() {
        let content = vec![
            json!({"type": "text", "text": "First", "page_idx": 0}),
            json!({"type": "header", "text": "Lower", "page_idx": 0, "bbox": [5, 40, 9, 50]}),
            json!({"type": "text", "text": "Second", "page_idx": 0}),
            json!({"type": "header", "text": "Upper right", "page_idx": 0, "bbox": [9, 10, 9, 20]}),
            json!({"type": "header", "text": "Upper left", "page_idx": 0, "bbox": [1, 10, 9, 20]}),
        ];
        assert_eq!(
            markdown(&rendered(&content, 1)),
            vec!["## Upper left\n\n## Upper right\n\n## Lower\n\nFirst\n\nSecond"]
        );
    }

    /// Equations are passed through untouched — MinerU ships its own `$$`.
    #[test]
    fn equations_are_not_wrapped() {
        let content = vec![
            json!({"type": "equation", "text": "$$E = mc^2$$", "page_idx": 0}),
            json!({"type": "equation", "text": "   ", "page_idx": 0}),
        ];
        assert_eq!(markdown(&rendered(&content, 1)), vec!["$$E = mc^2$$"]);
    }

    /// The two preserved oddities: an image wins over a table body, and a
    /// footer with a `text_level` becomes a heading.
    #[test]
    fn the_inherited_oddities_are_preserved() {
        let content = vec![
            json!({
                "type": "table",
                "page_idx": 0,
                "img_path": "images/t.png",
                "table_body": "<table><tr><td>1</td></tr></table>",
                "table_caption": ["Table 1"],
            }),
            json!({"type": "footer", "text": "Slide 4", "text_level": 1, "page_idx": 1}),
            json!({"type": "footer", "text": "Slide 5", "text_level": 0, "page_idx": 2}),
        ];
        assert_eq!(
            markdown(&rendered(&content, 3)),
            vec!["![Table 1](deck_images/t.png)", "## Slide 4", ""]
        );
    }

    /// No `img_path` falls back to the raw HTML body; captions are joined with
    /// a space and footnotes hang below with a blank line.
    #[test]
    fn a_table_without_a_crop_renders_its_html_body() {
        let content = vec![json!({
            "type": "table",
            "page_idx": 0,
            "table_body": "  <table><tr><td>1</td></tr></table>  ",
            "table_caption": ["Table", "1"],
            "table_footnote": ["Source: nowhere"],
        })];
        assert_eq!(
            markdown(&rendered(&content, 1)),
            vec!["<table><tr><td>1</td></tr></table>\n\nSource: nowhere"]
        );
    }

    /// Text-bearing types with an empty or whitespace-only body vanish, and a
    /// truthy `text_level` on a plain text item promotes it.
    #[test]
    fn empty_text_vanishes_and_text_level_promotes() {
        let content = vec![
            json!({"type": "text", "text": "  ", "page_idx": 0}),
            json!({"type": "text", "text": "Learning outcomes", "text_level": 2, "page_idx": 0}),
            json!({"type": "text", "page_idx": 0}),
        ];
        assert_eq!(markdown(&rendered(&content, 1)), vec!["## Learning outcomes"]);
    }

    /// The size filter, end to end: the undersized crop is dropped along with
    /// its caption, the large one is copied, and a name the backend never
    /// shipped is skipped without being counted.
    #[test]
    fn undersized_crops_are_dropped_with_their_captions() {
        let root = std::env::temp_dir().join(format!(
            "oculus-render-images-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let source = root.join("source");
        let out = root.join("deck_images");
        fs::create_dir_all(&source).unwrap();
        // Minimal PNG headers: 300x200 = 60000 px² (kept), 100x100 = 10000
        // px² (dropped). `imagesize` reads the IHDR and never decodes.
        fs::write(source.join("big.png"), png_header(300, 200)).unwrap();
        fs::write(source.join("small.png"), png_header(100, 100)).unwrap();

        let content = vec![
            json!({"type": "image", "page_idx": 0, "img_path": "images/big.png",
                   "image_caption": ["Figure 1"]}),
            json!({"type": "image", "page_idx": 1, "img_path": "images/small.png",
                   "image_caption": ["Logo"], "image_footnote": ["Faculty mark"]}),
            json!({"type": "image", "page_idx": 2, "img_path": "images/missing.png",
                   "image_caption": ["Absent"]}),
        ];
        let (pages, count) = render(&content, 3, &source, &out, "deck_images").unwrap();

        assert_eq!(count, 1);
        assert!(out.join("big.png").is_file());
        assert!(!out.join("small.png").exists());
        assert_eq!(
            markdown(&pages),
            vec![
                "![Figure 1](deck_images/big.png)",
                // Caption gone with the crop; the footnote is prose and stays.
                "Faculty mark",
                // A crop the backend named but never shipped still links: the
                // markdown on disk already reads this way.
                "![Absent](deck_images/missing.png)",
            ]
        );

        fs::remove_dir_all(&root).ok();
    }

    /// Two items pointing at the same crop copy it once and both link it.
    #[test]
    fn a_duplicate_crop_name_is_copied_once() {
        let root = std::env::temp_dir()
            .join(format!("oculus-render-dupe-{}", std::process::id()));
        let source = root.join("source");
        let out = root.join("deck_images");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("fig.png"), png_header(400, 400)).unwrap();

        let content = vec![
            json!({"type": "image", "page_idx": 0, "img_path": "a/fig.png"}),
            json!({"type": "image", "page_idx": 1, "img_path": "b/fig.png"}),
        ];
        let (pages, count) = render(&content, 2, &source, &out, "deck_images").unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            markdown(&pages),
            vec!["![](deck_images/fig.png)", "![](deck_images/fig.png)"]
        );

        fs::remove_dir_all(&root).ok();
    }

    /// Whitespace and case do not save a running head from detection.
    #[test]
    fn boilerplate_matching_collapses_whitespace_and_case() {
        let mut content: Vec<Value> = (0..2)
            .map(|page| json!({"type": "header", "text": "MAST20004  Probability", "page_idx": page}))
            .collect();
        content.extend((2..4).map(|page| {
            json!({"type": "header", "text": "mast20004\n probability", "page_idx": page})
        }));
        content.push(json!({"type": "text", "text": "Mast20004 Probability", "page_idx": 0}));
        // 4 distinct pages out of 4 — over the threshold, so every spelling
        // goes, including the plain text item that normalises to the same key.
        assert_eq!(markdown(&rendered(&content, 4)), vec!["", "", "", ""]);
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
        bytes.extend_from_slice(&13u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0]);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes
    }
}
