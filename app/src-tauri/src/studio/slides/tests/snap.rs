//! A crop never slices ink: a caption under a figure's box is taken in
//! whole, another block's text under it is left out whole, and a box that
//! cuts a line moves out past it.

use super::super::snap::Edge;
use super::super::{make, Request, Select};
use super::fixture::{write_pages, Block};
use crate::test_support::Scratch;

/// A grey figure, x 100-500 pt and 50-250 pt from the top of a 600x400 page.
const FIGURE: &str = "0.55 0.57 0.6 rg 100 150 400 200 re f\n";
const FIGURE_BOX: [f64; 4] = [100.0 / 600.0, 50.0 / 400.0, 500.0 / 600.0, 250.0 / 400.0];

/// A 12 pt line whose baseline is `top` pt from the page's top.
fn line(top: f64, text: &str) -> String {
    format!(
        "0 0 0 rg BT /F1 12 Tf 110 {} Td ({text}) Tj ET\n",
        400.0 - top
    )
}

struct Page {
    _scratch: Scratch,
    data: std::path::PathBuf,
}

const SOURCE: &str = "courses/T/files/figs.pdf";

fn pages() -> Page {
    let scratch = Scratch::new("slide-snap");
    let pdf = scratch.join(SOURCE);
    std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
    let caption = format!(
        "{FIGURE}{}",
        line(266.0, "Figure 2: the caption under the box")
    );
    let paragraph = format!(
        "{FIGURE}{}{}",
        line(266.0, "An unrelated paragraph that starts right under it"),
        line(280.0, "and runs on to a second line of its own.")
    );
    let figure: Block = (
        "image",
        FIGURE_BOX,
        "![Figure 2: the caption under the box](i/f.jpg)",
    );
    let below: Block = (
        "text",
        [100.0 / 600.0, 256.0 / 400.0, 500.0 / 600.0, 284.0 / 400.0],
        "An unrelated paragraph that starts right under it and runs on to a second line of its own.",
    );
    // The box ends at 262 pt, through the caption's line.
    let tight: Block = (
        "image",
        [100.0 / 600.0, 50.0 / 400.0, 500.0 / 600.0, 262.0 / 400.0],
        "![](i/f.jpg)",
    );
    write_pages(
        &pdf,
        &[
            (caption.clone(), vec![figure]),
            (paragraph, vec![figure, below]),
            (caption, vec![tight]),
        ],
    );
    let data = scratch.to_path_buf();
    std::fs::create_dir_all(data.join("video/assets")).unwrap();
    Page {
        _scratch: scratch,
        data,
    }
}

fn crop(p: &Page, page: u32, exact: bool) -> (super::super::Made, image::RgbImage) {
    let video = p.data.join("video");
    let made = make(&Request {
        video: &video,
        data_dir: &p.data,
        source: SOURCE,
        page: Some(page),
        select: Select::Blocks(vec![0]),
        name: Some(format!("p{page}{}", if exact { "x" } else { "" })),
        pad: 0.02,
        width: Some(800),
        replace: false,
        exact,
    })
    .unwrap();
    let png = image::open(&made.png).unwrap().to_rgb8();
    (made, png)
}

/// Rows and columns of ink (far from white) on the image's four edges.
fn inked_edges(img: &image::RgbImage) -> Vec<&'static str> {
    let ink = |x: u32, y: u32| img.get_pixel(x, y).0.iter().any(|c| *c < 215);
    let (w, h) = img.dimensions();
    let mut out = Vec::new();
    if (0..w).any(|x| ink(x, 0)) {
        out.push("top");
    }
    if (0..w).any(|x| ink(x, h - 1)) {
        out.push("bottom");
    }
    if (0..h).any(|y| ink(0, y)) {
        out.push("left");
    }
    if (0..h).any(|y| ink(w - 1, y)) {
        out.push("right");
    }
    out
}

#[test]
fn a_caption_under_the_box_is_taken_in_whole() {
    let p = pages();
    let (made, img) = crop(&p, 1, false);
    let edges = made.sidecar.edges.unwrap();
    assert_eq!(edges.bottom, Edge::Extend, "{edges:?}");
    // The caption's descenders end near 269 pt.
    assert!(
        made.sidecar.bbox[3] * 400.0 > 269.0,
        "{:?}",
        made.sidecar.bbox
    );
    assert!(inked_edges(&img).is_empty(), "{:?}", inked_edges(&img));
    // --exact keeps the pad, which slices the caption's top.
    let (exact, img) = crop(&p, 1, true);
    assert!(exact.sidecar.edges.is_none() && exact.sidecar.exact);
    assert_eq!(inked_edges(&img), ["bottom"]);
}

#[test]
fn another_blocks_text_under_the_box_is_left_out_whole() {
    let p = pages();
    let (made, img) = crop(&p, 2, false);
    let edges = made.sidecar.edges.unwrap();
    assert_eq!(edges.bottom, Edge::Trim, "{edges:?}");
    // The paragraph's ascenders start near 257 pt.
    let bottom = made.sidecar.bbox[3] * 400.0;
    assert!((250.0..257.0).contains(&bottom), "{bottom}");
    assert!(inked_edges(&img).is_empty(), "{:?}", inked_edges(&img));
}

#[test]
fn a_box_that_cuts_a_line_moves_out_past_it() {
    let p = pages();
    let (made, img) = crop(&p, 3, false);
    assert_eq!(made.sidecar.edges.unwrap().bottom, Edge::Extend);
    assert!(
        made.sidecar.bbox[3] * 400.0 > 269.0,
        "{:?}",
        made.sidecar.bbox
    );
    assert!(inked_edges(&img).is_empty(), "{:?}", inked_edges(&img));
}
