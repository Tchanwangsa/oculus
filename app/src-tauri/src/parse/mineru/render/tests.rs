use super::item::render_item;
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

/// The span `[start, end)` of `markdown`, in UTF-16 code units as JS
/// `String.slice` reads it.
fn slice16(markdown: &str, start: u32, end: u32) -> String {
    let units: Vec<u16> = markdown.encode_utf16().collect();
    String::from_utf16(&units[start as usize..end as usize]).unwrap()
}

/// The per-page join `render` did before it tracked blocks: the oracle for
/// "block tracking never changes the markdown".
fn joined_without_blocks(content: &[Value], page_idx_wanted: i64) -> String {
    let mut items: Vec<&Value> = content
        .iter()
        .filter(|item| page_idx(item) == page_idx_wanted)
        .collect();
    items.sort_by(|left, right| compare(left, right));
    let none = HashSet::new();
    items
        .iter()
        .filter_map(|item| render_item(item, "deck_images", &none, &none))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn mixed_page() -> Vec<Value> {
    vec![
        json!({"type": "text", "text": "Δx → 0 as n → ∞", "page_idx": 0,
               "bbox": [80, 110, 920, 190]}),
        json!({"type": "header", "text": "Week 3 · Limits", "page_idx": 0,
               "bbox": [50, 20, 600, 60]}),
        json!({"type": "footer", "text": "Page 3", "page_idx": 0, "bbox": [0, 950, 100, 990]}),
        json!({"type": "text", "text": "Definition", "text_level": 1, "page_idx": 0,
               "bbox": [80, 200, 400, 230]}),
        json!({"type": "equation", "text": "$$\\mathbb{E}[X] = 𝔼 😀$$", "page_idx": 0,
               "bbox": [100, 240, 900, 300]}),
        json!({"type": "text", "text": "   ", "page_idx": 0, "bbox": [0, 0, 10, 10]}),
        json!({"type": "table", "page_idx": 0, "bbox": [80, 320, 920, 600],
               "table_body": "<table><tr><td>ψ</td></tr></table>",
               "table_footnote": ["Source: notes"]}),
        json!({"type": "text", "text": "No box at all", "page_idx": 0}),
        json!({"type": "text", "text": "A flat box", "page_idx": 0, "bbox": [80, 700, 920, 700]}),
        json!({"type": "image", "page_idx": 0, "bbox": [100, 720, 900, 1100],
               "img_path": "images/fig.png", "image_caption": ["Figure 1"],
               "image_footnote": ["Read left to right"]}),
    ]
}

/// Tracking blocks leaves each page's markdown byte-identical to the
/// plain join.
#[test]
fn block_tracking_never_changes_the_markdown() {
    let content = mixed_page();
    let pages = rendered(&content, 1);
    assert_eq!(pages[0].markdown, joined_without_blocks(&content, 0));
    assert_eq!(
        pages[0].markdown,
        "## Week 3 · Limits\n\nΔx → 0 as n → ∞\n\n## Definition\n\n\
         $$\\mathbb{E}[X] = 𝔼 😀$$\n\n\
         <table><tr><td>ψ</td></tr></table>\n\nSource: notes\n\n\
         No box at all\n\nA flat box\n\n\
         ![Figure 1](deck_images/fig.png)\n\nRead left to right"
    );
}

/// Offsets are UTF-16 code units — astral characters count two — and each
/// span is exactly its item's text, separators excluded.
#[test]
fn blocks_carry_page_fractions_and_utf16_offsets() {
    let pages = rendered(&mixed_page(), 1);
    let page = &pages[0];
    let spans: Vec<(&str, String)> = page
        .blocks
        .iter()
        .map(|block| {
            (
                block.kind.as_str(),
                slice16(&page.markdown, block.start, block.end),
            )
        })
        .collect();
    assert_eq!(
        spans,
        vec![
            ("header", "## Week 3 · Limits".to_string()),
            ("text", "Δx → 0 as n → ∞".to_string()),
            ("text", "## Definition".to_string()),
            ("equation", "$$\\mathbb{E}[X] = 𝔼 😀$$".to_string()),
            (
                "table",
                "<table><tr><td>ψ</td></tr></table>\n\nSource: notes".to_string()
            ),
            (
                "image",
                "![Figure 1](deck_images/fig.png)\n\nRead left to right".to_string()
            ),
        ]
    );

    let header = &page.blocks[0];
    assert_eq!(header.start, 0);
    assert_eq!(header.bbox, [0.05, 0.02, 0.6, 0.06]);
    // "Δx → 0 as n → ∞" is 15 UTF-16 units but 22 bytes.
    let text = &page.blocks[1];
    assert_eq!(text.start, header.end + 2);
    assert_eq!(text.end - text.start, 15);
    assert_eq!(text.bbox, [0.08, 0.11, 0.92, 0.19]);
    // 𝔼 and 😀 are astral: two units each.
    let equation = &page.blocks[3];
    assert_eq!(
        equation.end - equation.start,
        "$$\\mathbb{E}[X] = 𝔼 😀$$".chars().count() as u32 + 2
    );
    // Past the page edge clamps to it.
    assert_eq!(page.blocks[5].bbox, [0.1, 0.72, 0.9, 1.0]);
    // The last block ends where the markdown does.
    assert_eq!(
        page.blocks[5].end as usize,
        page.markdown.encode_utf16().count()
    );
}

/// Lifting headers to the top moves each block with its own span.
#[test]
fn the_header_sort_keeps_each_block_with_its_span() {
    let content = vec![
        json!({"type": "text", "text": "First", "page_idx": 0, "bbox": [0, 500, 1000, 520]}),
        json!({"type": "header", "text": "Lower", "page_idx": 0, "bbox": [5, 40, 9, 50]}),
        json!({"type": "text", "text": "Second", "page_idx": 0, "bbox": [0, 600, 1000, 620]}),
        json!({"type": "header", "text": "Upper", "page_idx": 0, "bbox": [1, 10, 9, 20]}),
    ];
    let pages = rendered(&content, 1);
    let page = &pages[0];
    assert_eq!(page.markdown, "## Upper\n\n## Lower\n\nFirst\n\nSecond");
    let pairs: Vec<(String, f32)> = page
        .blocks
        .iter()
        .map(|block| {
            (
                slice16(&page.markdown, block.start, block.end),
                block.bbox[1],
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("## Upper".to_string(), 0.01),
            ("## Lower".to_string(), 0.04),
            ("First".to_string(), 0.5),
            ("Second".to_string(), 0.6),
        ]
    );
}

/// Only rendered items get blocks: boilerplate, footers, empties and a
/// size-filtered crop have none, and neither does a blank page.
#[test]
fn dropped_items_have_no_block() {
    let root = Scratch::new("render-blocks");
    let source = root.join("source");
    let out = root.join("deck_images");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("logo.png"), png_header(100, 100)).unwrap();

    let boxed = [100, 100, 900, 200];
    let mut content: Vec<Value> = (0..4)
        .map(|page| json!({"type": "header", "text": "Running head", "page_idx": page, "bbox": boxed}))
        .collect();
    content.extend([
        json!({"type": "footer", "text": "Slide 1", "page_idx": 0, "bbox": boxed}),
        json!({"type": "text", "text": "", "page_idx": 0, "bbox": boxed}),
        json!({"type": "equation", "text": "  ", "page_idx": 0, "bbox": boxed}),
        json!({"type": "image", "page_idx": 0, "img_path": "images/logo.png",
               "image_caption": ["Logo"], "bbox": boxed}),
        json!({"type": "text", "text": "Kept", "page_idx": 0, "bbox": [100, 300, 900, 400]}),
        json!({"type": "text", "text": "Only on page two", "page_idx": 1, "bbox": [0, 0, 1000]}),
    ]);
    let (pages, _) = render(&content, 4, &source, &out, "deck_images").unwrap();

    assert_eq!(pages[0].markdown, "Kept");
    assert_eq!(
        pages[0].blocks,
        vec![ParseBlock {
            kind: "text".into(),
            bbox: [0.1, 0.3, 0.9, 0.4],
            start: 0,
            end: 4,
        }]
    );
    // Rendered, but a three-number bbox is no box.
    assert_eq!(pages[1].markdown, "Only on page two");
    assert!(pages[1].blocks.is_empty());
    // Furniture-only and blank pages.
    assert!(pages[2].markdown.is_empty() && pages[2].blocks.is_empty());
    assert!(pages[3].markdown.is_empty() && pages[3].blocks.is_empty());
}

/// An image and the footnote hanging under it are one item, so one block.
#[test]
fn an_image_and_its_footnote_are_one_block() {
    let content = vec![json!({
        "type": "chart", "page_idx": 0, "bbox": [0, 0, 500, 500],
        "img_path": "images/c.png", "chart_footnote": ["Explaining prose"],
    })];
    let pages = rendered(&content, 1);
    assert_eq!(pages[0].blocks.len(), 1);
    let block = &pages[0].blocks[0];
    assert_eq!(block.kind, "chart");
    assert_eq!(
        slice16(&pages[0].markdown, block.start, block.end),
        "![](deck_images/c.png)\n\nExplaining prose"
    );
}

/// The degenerate-box and non-number cases of `page_box`.
#[test]
fn only_a_real_box_is_kept() {
    let boxed = |bbox: Value| page_box(&json!({ "bbox": bbox }));
    assert_eq!(boxed(json!([0, 0, 1000, 1000])), Some([0.0, 0.0, 1.0, 1.0]));
    assert_eq!(
        boxed(json!([-50, 10.5, 2000, 20])),
        Some([0.0, 0.0105, 1.0, 0.02])
    );
    assert_eq!(boxed(json!([500, 0, 500, 100])), None);
    assert_eq!(boxed(json!([0, 300, 100, 200])), None);
    assert_eq!(boxed(json!([1200, 0, 1500, 100])), None);
    assert_eq!(boxed(json!([0, 0, "100", 100])), None);
    assert_eq!(boxed(json!([0, 0, 100, 100, 5])), None);
    assert_eq!(boxed(json!(null)), None);
    assert_eq!(page_box(&json!({})), None);
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
