use super::super::blocks::{find, page_blocks, plain, snippet, Found, SNIPPET_CHARS};
use crate::parse::{ParseBlock, ParseOutput, ParsePage};

fn page(page_no: u32, items: &[(&str, [f32; 4], &str)]) -> ParsePage {
    let mut markdown = String::new();
    let mut blocks = Vec::new();
    for (kind, bbox, text) in items {
        if !markdown.is_empty() {
            markdown.push_str("\n\n");
        }
        let start = markdown.encode_utf16().count() as u32;
        markdown.push_str(text);
        let end = markdown.encode_utf16().count() as u32;
        blocks.push(ParseBlock {
            kind: (*kind).to_owned(),
            bbox: *bbox,
            start,
            end,
        });
    }
    ParsePage {
        page_no,
        markdown,
        blocks,
    }
}

fn record(pages: Vec<ParsePage>) -> ParseOutput {
    ParseOutput {
        pdf: "x.pdf".into(),
        mode: "quality".into(),
        parser_version: 3,
        page_count: pages.len() as u32,
        pages,
        backend: None,
        image_count: 0,
    }
}

const B: [f32; 4] = [0.1, 0.1, 0.9, 0.2];
const LOW: [f32; 4] = [0.1, 0.5, 0.9, 0.6];

#[test]
fn a_block_reads_as_one_line_of_plain_text() {
    assert_eq!(plain("## Gradient  descent\n"), "Gradient descent");
    assert_eq!(
        plain("![(b) Backward Secrecy Figure 1](doc_images/x.jpg)"),
        "(b) Backward Secrecy Figure 1"
    );
    assert_eq!(plain("![](doc_images/x.jpg)"), "[image]");
    assert_eq!(plain("<table><tr><td>a</td><td>b</td></tr></table>"), "a b");
    assert_eq!(
        plain("Unger et al.<sup>1</sup> show"),
        "Unger et al. 1 show"
    );
    let long = "word ".repeat(40);
    let s = snippet(long.trim());
    assert_eq!(s.chars().count(), SNIPPET_CHARS);
    assert!(s.ends_with('…'));
}

#[test]
fn spans_are_utf16_offsets() {
    let r = record(vec![page(
        1,
        &[("text", B, "naïve 𝑥 step"), ("text", LOW, "after")],
    )]);
    let blocks = page_blocks(&r, 1);
    assert_eq!(blocks[0].text, "naïve 𝑥 step");
    assert_eq!(blocks[1].text, "after");
    assert!(page_blocks(&r, 2).is_empty());
}

#[test]
fn a_figure_beats_a_mention_in_the_text() {
    let r = record(vec![
        page(1, &[("text", B, "As Figure 1 shows, secrecy holds.")]),
        page(
            2,
            &[
                ("image", B, "![(a) Forward](i/a.jpg)"),
                (
                    "image",
                    LOW,
                    "![(b) Backward Figure 1 - Forward vs Backward](i/b.jpg)",
                ),
            ],
        ),
    ]);
    assert_eq!(
        find(&r, None, "figure 1").unwrap(),
        Found {
            page: 2,
            blocks: vec![1]
        }
    );
}

#[test]
fn a_caption_brings_its_figure() {
    let r = record(vec![page(
        1,
        &[
            ("text", B, "# Gradient descent"),
            ("image", [0.1, 0.25, 0.9, 0.7], "![](i/loss.jpg)"),
            (
                "text",
                [0.1, 0.72, 0.9, 0.78],
                "Figure 3: Each step moves against the slope.",
            ),
            ("text", [0.1, 0.8, 0.9, 0.9], "The step size matters."),
        ],
    )]);
    assert_eq!(
        find(&r, Some(1), "Figure 3").unwrap(),
        Found {
            page: 1,
            blocks: vec![1, 2]
        }
    );
    // Text with no figure beside it is taken alone.
    assert_eq!(
        find(&r, Some(1), "step size").unwrap(),
        Found {
            page: 1,
            blocks: vec![3]
        }
    );
}

#[test]
fn a_tie_or_a_miss_lists_what_there_is() {
    let r = record(vec![page(
        1,
        &[
            ("table", B, "<table><tr><td>loss</td></tr></table>"),
            ("chart", LOW, "![loss curve](i/c.jpg)"),
        ],
    )]);
    let err = find(&r, Some(1), "loss").unwrap_err();
    assert!(err.contains("matches 2 blocks"), "{err}");
    assert!(err.contains("page 1 block 0 (table): loss"), "{err}");
    assert!(err.contains("page 1 block 1 (chart): loss curve"), "{err}");
    let err = find(&r, Some(1), "momentum").unwrap_err();
    assert!(err.contains("no block on page 1"), "{err}");
    assert!(err.contains("oculus studio blocks"), "{err}");
    assert!(find(&r, None, "  ").is_err());
}
