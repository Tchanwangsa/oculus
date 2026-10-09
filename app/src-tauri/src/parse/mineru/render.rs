//! Turning MinerU's `content_list.json` into page records, for both backends.
//!
//! This decides what the markdown *says*, so a subtle change here silently
//! rewrites the library rather than failing. The tests pin the exact output
//! the existing library was rendered with.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::parse::{ParseError, ParsePage};

/// A header/footer on at least this fraction of a window's pages is template
/// furniture, not content.
const BOILERPLATE_PAGE_RATIO: f64 = 0.5;

/// Below this many pages the ratio means nothing.
const BOILERPLATE_MIN_PAGES: i64 = 4;

/// Equations may also carry an `img_path`, but are rendered as LaTeX.
const IMAGE_TYPES: [&str; 3] = ["image", "chart", "table"];

/// Crops render at ~1.5x: 20k px² rejects small template furniture while
/// retaining the smallest real figure in the measured deck (58k px²).
const MIN_IMAGE_AREA: u64 = 20_000;

/// Boilerplate is detected in fixed 64-page windows. **Bug-compatibility, not
/// a heuristic**: changing it rewrites markdown already in the library with no
/// version bump (`PARSER_VERSION` guards shape, not text).
const RENDER_GROUP_PAGES: i64 = 64;

// ── Content-list accessors ───────────────────────────────────────────────────
//
// Untyped JSON from a backend we do not control: every read has a default.

fn kind_of(item: &Value) -> &str {
    item.get("type").and_then(Value::as_str).unwrap_or("")
}

fn text_of(item: &Value) -> &str {
    item.get("text").and_then(Value::as_str).unwrap_or("")
}

fn page_idx(item: &Value) -> i64 {
    item.get("page_idx").and_then(Value::as_i64).unwrap_or(0)
}

/// `img_path`, when non-empty: an empty one falls through to `table_body`.
fn img_path(item: &Value) -> Option<&str> {
    item.get("img_path")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
}

/// Python-style truthiness: `text_level: 0` is level-less, `3` is a heading.
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

/// The link is the basename, so the archive's layout never leaks.
fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The caption/footnote list, space-joined and trimmed.
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

/// The comparison form for boilerplate: whitespace-collapsed and lowercased.
fn norm(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

// ── Rendering one item ───────────────────────────────────────────────────────

/// Convert one content-list entry to a markdown block, or drop it.
///
/// The branch order is load-bearing; two interactions look like bugs and are
/// preserved, because changing either rewrites the library's markdown:
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

    // MinerU's equation text carries its own `$$` delimiters.
    if kind == "equation" {
        let text = text_of(item).trim();
        return (!text.is_empty()).then(|| text.to_string());
    }

    if IMAGE_TYPES.contains(&kind) {
        let caption = join_parts(item, &format!("{kind}_caption"));
        let footnote = join_parts(item, &format!("{kind}_footnote"));

        let block = match img_path(item) {
            // Dropped by the size filter, caption with it.
            Some(path) if dropped.contains(&basename(path)) => String::new(),
            Some(path) => format!("![{caption}]({images_rel}/{})", basename(path)),
            None => item
                .get("table_body")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
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

    // One fixed level: MinerU's `text_level` numbers are per-page guesses.
    if kind == "header" || truthy(item.get("text_level")) {
        return Some(format!("## {text}"));
    }
    if kind == "footer" {
        return None;
    }
    Some(text.to_string())
}

// ── Ordering ─────────────────────────────────────────────────────────────────

/// Put headers first while preserving MinerU's body reading order. Every
/// non-header shares one key, so the sort must be stable (`sort_by`, never
/// `sort_unstable_by`).
fn sort_key(item: &Value) -> (u8, f64, f64) {
    if kind_of(item) != "header" {
        return (1, 0.0, 0.0);
    }
    let bbox = item
        .get("bbox")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty());
    let at = |index: usize| {
        bbox.and_then(|values| values.get(index))
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    };
    (0, at(1), at(0))
}

fn compare(left: &Value, right: &Value) -> Ordering {
    let (a, b) = (sort_key(left), sort_key(right));
    a.0.cmp(&b.0)
        // A NaN coordinate compares equal rather than panicking.
        .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
        .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(Ordering::Equal))
}

// ── Boilerplate ──────────────────────────────────────────────────────────────

/// The normalised header/footer strings repeated across most of a window's
/// pages — counted by **distinct** page, not occurrence.
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
/// Items must carry **absolute** `page_idx` values, or the 64-page windows
/// would restart at every task boundary.
///
/// `source_images` is the extracted result's `images` directory; `images_dir`
/// the staging directory; `images_rel` the link prefix. Returns the page
/// records and the number of images copied.
pub fn render(
    content_list: &[Value],
    total_pages: u32,
    source_images: &Path,
    images_dir: &Path,
    images_rel: &str,
) -> Result<(Vec<ParsePage>, u32), ParseError> {
    // 1. Which crops the markdown will reference, deduped and in stable order.
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
            // Named but not shipped: not counted, though its item still links.
            if !source.is_file() {
                continue;
            }
            if too_small(&source) {
                dropped.insert(name.clone());
                continue;
            }
            fs::copy(&source, images_dir.join(name)).map_err(|e| {
                ParseError::Io(format!(
                    "copy {} -> {}: {e}",
                    source.display(),
                    images_dir.display()
                ))
            })?;
            image_count += 1;
        }
    }

    // 2. Detect boilerplate per 64-page window, against that window's own
    //    page count (the last is short).
    let mut windows: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
    for item in content_list {
        windows
            .entry(page_idx(item).div_euclid(RENDER_GROUP_PAGES))
            .or_default()
            .push(item);
    }

    let mut by_page: BTreeMap<i64, String> = BTreeMap::new();
    for (window, items) in windows {
        let window_pages = RENDER_GROUP_PAGES.min(total_pages as i64 - window * RENDER_GROUP_PAGES);
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
                // Windows partition by page: no page is written twice.
                by_page.insert(page_no, blocks.join("\n\n"));
            }
        }
    }

    // 3. Blank and furniture-only pages still need records: `page_no` is the
    //    citation join key.
    for page_no in 1..=i64::from(total_pages) {
        by_page.entry(page_no).or_default();
    }

    let pages = by_page
        .into_iter()
        // A broken offset below the first page has nowhere to attach.
        .filter(|(page_no, _)| *page_no >= 1)
        .map(|(page_no, markdown)| ParsePage {
            page_no: page_no as u32,
            markdown,
        })
        .collect();
    Ok((pages, image_count))
}

/// Is this crop too small to be a real figure? Reads the header only. An
/// unreadable image **fails open**: losing a figure is worse than keeping
/// furniture.
fn too_small(path: &Path) -> bool {
    match imagesize::size(path) {
        Ok(size) => (size.width as u64) * (size.height as u64) < MIN_IMAGE_AREA,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;
    use serde_json::json;

    /// Every expected value here is the output the library's existing
    /// markdown was rendered with. Do not "fix" one to match new code.
    fn rendered(content: &[Value], total_pages: u32) -> Vec<ParsePage> {
        let nowhere = Path::new("/nonexistent-source-images");
        let (pages, count) = render(content, total_pages, nowhere, nowhere, "deck_images").unwrap();
        assert_eq!(count, 0);
        pages
    }

    fn markdown(pages: &[ParsePage]) -> Vec<&str> {
        pages.iter().map(|page| page.markdown.as_str()).collect()
    }

    /// A header on pages 0-31 hits the first window's 32-page threshold and is
    /// stripped; one on page 129 lands in a 2-page window and is kept.
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

    /// One page short of the threshold keeps the header (the ratio is `>=`).
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
        // Dropped by the footer rule, not by boilerplate detection.
        assert_eq!(markdown(&rendered(&content, 3)), vec!["", "", ""]);
    }

    /// Stable ordering with a header after body text, plus the image block and
    /// blank-page handling.
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

    /// Equations are passed through untouched.
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
        assert_eq!(
            markdown(&rendered(&content, 1)),
            vec!["## Learning outcomes"]
        );
    }

    /// The size filter, end to end: the undersized crop is dropped along with
    /// its caption, the large one is copied, and a name the backend never
    /// shipped is skipped without being counted.
    #[test]
    fn undersized_crops_are_dropped_with_their_captions() {
        let root = Scratch::new("render-images");
        let source = root.join("source");
        let out = root.join("deck_images");
        fs::create_dir_all(&source).unwrap();
        // Minimal PNG headers: 300x200 (kept), 100x100 (dropped).
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
                // Named but never shipped: still links.
                "![Absent](deck_images/missing.png)",
            ]
        );
    }

    /// Two items pointing at the same crop copy it once and both link it.
    #[test]
    fn a_duplicate_crop_name_is_copied_once() {
        let root = Scratch::new("render-dupe");
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
    }

    /// Whitespace and case do not save a running head from detection.
    #[test]
    fn boilerplate_matching_collapses_whitespace_and_case() {
        let mut content: Vec<Value> = (0..2)
            .map(|page| json!({"type": "header", "text": "MAST20004  Probability", "page_idx": page}))
            .collect();
        content.extend((2..4).map(
            |page| json!({"type": "header", "text": "mast20004\n probability", "page_idx": page}),
        ));
        content.push(json!({"type": "text", "text": "Mast20004 Probability", "page_idx": 0}));
        // Every spelling goes, including a plain text item with the same key.
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
