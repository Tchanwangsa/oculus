//! A two-page lecture PDF made with lopdf, and the `.pages.json` a parse of
//! it would write. Page 1 is a 720x405 pt slide: a title, bullets, a figure
//! (a loss curve with steps down it, on a framed panel) and its caption as
//! its own block. Page 2 is 400x300 pt turned by `/Rotate 90`, with a red
//! square whose place on the rendered page is known.

use std::path::Path;

use lopdf::{dictionary, Document, Object, Stream};
use serde_json::json;

/// The figure panel on page 1, as fractions of the page.
pub const FIGURE: [f64; 4] = [0.4583, 0.1975, 0.9583, 0.7901];
/// The caption under it.
pub const CAPTION: [f64; 4] = [0.4722, 0.8148, 0.9444, 0.8642];
/// The red square on page 2 as rendered: unrotated it is x 20-80, y 200-280
/// on a 400x300 page; turned a quarter clockwise, (x, y) lands at (y, x).
pub const RED: [f64; 4] = [200.0 / 300.0, 20.0 / 400.0, 280.0 / 300.0, 80.0 / 400.0];

/// page 1's content, PDF coordinates (origin bottom left).
fn slide() -> String {
    let mut s = String::new();
    // Title and bullets.
    s.push_str("BT /F2 30 Tf 36 345 Td (Gradient descent) Tj ET\n");
    for (i, line) in [
        "- Start anywhere on the loss",
        "- Step against the gradient",
        "- Smaller steps near the minimum",
        "- Stop when the slope flattens",
    ]
    .iter()
    .enumerate()
    {
        s.push_str(&format!(
            "BT /F1 15 Tf 36 {} Td ({line}) Tj ET\n",
            285 - 30 * i
        ));
    }
    // The figure's panel: a pale fill and a grey frame.
    s.push_str("0.96 0.97 1 rg 330 85 360 240 re f\n");
    s.push_str("0.55 0.57 0.6 RG 1.5 w 330 85 360 240 re S\n");
    // Axes.
    s.push_str("0.2 0.22 0.25 RG 2 w 360 110 m 670 110 l S 360 110 m 360 305 l S\n");
    // The loss: y = 110 + (x - 520)^2 / 130, drawn as a polyline.
    s.push_str("0.37 0.42 0.82 RG 3 w ");
    for k in 0..=60 {
        let x = 375.0 + 290.0 * f64::from(k) / 60.0;
        let y = 115.0 + (x - 520.0).powi(2) / 130.0;
        let op = if k == 0 { "m" } else { "l" };
        s.push_str(&format!("{x:.2} {y:.2} {op} "));
    }
    s.push_str("S\n");
    // Steps down the curve: orange squares joined by thin lines.
    let xs = [390.0, 440.0, 480.0, 505.0, 516.0];
    s.push_str("0.95 0.55 0.15 RG 1.5 w ");
    for (k, x) in xs.iter().enumerate() {
        let y = 115.0 + (x - 520.0f64).powi(2) / 130.0;
        let op = if k == 0 { "m" } else { "l" };
        s.push_str(&format!("{x:.2} {y:.2} {op} "));
    }
    s.push_str("S\n0.95 0.55 0.15 rg ");
    for x in xs {
        let y = 115.0 + (x - 520.0f64).powi(2) / 130.0;
        s.push_str(&format!("{:.2} {:.2} 8 8 re f ", x - 4.0, y - 4.0));
    }
    s.push('\n');
    // Axis labels and the caption.
    s.push_str("0.2 0.22 0.25 rg BT /F1 12 Tf 600 95 Td (weight) Tj ET\n");
    s.push_str("BT /F1 12 Tf 336 300 Td (loss) Tj ET\n");
    s.push_str("BT /F3 13 Tf 345 63 Td (Figure 3: Each step moves against the slope.) Tj ET\n");
    // A footer, which a parse drops.
    s.push_str("0.5 0.5 0.5 rg BT /F1 9 Tf 36 18 Td (Lecture 4 - Optimisation) Tj ET\n");
    s
}

/// Writes the PDF to `path` and its `.pages.json` beside it.
pub fn write(path: &Path) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = |doc: &mut Document, base: &str| {
        doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => base,
        })
    };
    let f1 = font(&mut doc, "Helvetica");
    let f2 = font(&mut doc, "Helvetica-Bold");
    let f3 = font(&mut doc, "Helvetica-Oblique");
    let resources = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => f1, "F2" => f2, "F3" => f3 },
    });
    let one = doc.add_object(Stream::new(dictionary! {}, slide().into_bytes()));
    let two = doc.add_object(Stream::new(
        dictionary! {},
        b"1 0 0 rg 20 200 60 80 re f\n".to_vec(),
    ));
    let page = |doc: &mut Document, contents, size: [i64; 2], rotate: i64| {
        doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), size[0].into(), size[1].into()],
            "Resources" => resources,
            "Contents" => contents,
            "Rotate" => rotate,
        })
    };
    let p1 = page(&mut doc, one, [720, 405], 0);
    let p2 = page(&mut doc, two, [400, 300], 90);
    let kids: Vec<Object> = vec![p1.into(), p2.into()];
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => 2 }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();

    let blocks: [(&str, [f64; 4], &str); 4] = [
        ("text", [0.05, 0.07, 0.45, 0.16], "# Gradient descent"),
        (
            "text",
            [0.05, 0.24, 0.45, 0.53],
            "- Start anywhere on the loss\n- Step against the gradient\n- Smaller steps near the minimum\n- Stop when the slope flattens",
        ),
        ("image", FIGURE, "![](slides_images/loss.jpg)"),
        ("text", CAPTION, "Figure 3: Each step moves against the slope."),
    ];
    let mut markdown = String::new();
    let mut spans = Vec::new();
    for (kind, bbox, text) in blocks {
        if !markdown.is_empty() {
            markdown.push_str("\n\n");
        }
        let start = markdown.encode_utf16().count();
        markdown.push_str(text);
        let end = markdown.encode_utf16().count();
        spans.push(json!({ "kind": kind, "bbox": bbox, "start": start, "end": end }));
    }
    let record = json!({
        "pdf": path.file_name().unwrap().to_string_lossy(),
        "mode": "quality",
        "parser_version": 3,
        "page_count": 2,
        "pages": [
            { "page_no": 1, "markdown": markdown, "blocks": spans },
            { "page_no": 2, "markdown": "" },
        ],
    });
    std::fs::write(
        crate::parse::pages_path(path),
        serde_json::to_string_pretty(&record).unwrap(),
    )
    .unwrap();
}

/// One block of a page's record: kind, box, markdown.
pub type Block = (&'static str, [f64; 4], &'static str);

/// A PDF of 600x400 pt pages, each a content stream (Helvetica is `/F1`)
/// with its record's blocks, and the `.pages.json` beside it.
pub fn write_pages(path: &Path, pages: &[(String, Vec<Block>)]) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let f1 = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let resources = doc.add_object(dictionary! { "Font" => dictionary! { "F1" => f1 } });
    let mut kids: Vec<Object> = Vec::new();
    let mut records = Vec::new();
    for (n, (content, blocks)) in pages.iter().enumerate() {
        let stream = doc.add_object(Stream::new(dictionary! {}, content.clone().into_bytes()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 600.into(), 400.into()],
            "Resources" => resources,
            "Contents" => stream,
        });
        kids.push(page.into());
        let mut markdown = String::new();
        let mut spans = Vec::new();
        for (kind, bbox, text) in blocks {
            if !markdown.is_empty() {
                markdown.push_str("\n\n");
            }
            let start = markdown.encode_utf16().count();
            markdown.push_str(text);
            let end = markdown.encode_utf16().count();
            spans.push(json!({ "kind": kind, "bbox": bbox, "start": start, "end": end }));
        }
        records.push(json!({ "page_no": n + 1, "markdown": markdown, "blocks": spans }));
    }
    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    doc.save(path).unwrap();
    let record = json!({
        "pdf": path.file_name().unwrap().to_string_lossy(),
        "mode": "quality",
        "parser_version": 3,
        "page_count": count,
        "pages": records,
    });
    std::fs::write(
        crate::parse::pages_path(path),
        serde_json::to_string_pretty(&record).unwrap(),
    )
    .unwrap();
}
