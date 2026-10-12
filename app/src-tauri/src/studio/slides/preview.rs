//! A page with each block's box and index drawn on it, for an agent to look
//! at before it picks one (`oculus studio blocks --png`). The digits are a
//! 3x5 bitmap font, so no font is loaded.

use super::blocks::{is_figure, PageBlock};
use super::raster::Raster;

/// Pixels a font cell is drawn at.
const CELL: u32 = 4;
/// The box outline's width.
const LINE: u32 = 3;

/// Rows of three bits, top first, for 0-9.
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 1, 1],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

const FIGURE: [u8; 3] = [0x5e, 0x6a, 0xd2];
const TEXT: [u8; 3] = [0x8a, 0x8f, 0x98];
const LABEL_INK: [u8; 3] = [0xff, 0xff, 0xff];

/// Draws `blocks` onto `page` (the whole page rendered): figures in indigo,
/// the rest in grey, each with its index in a tab at its top left.
pub fn annotate(page: &mut Raster, blocks: &[PageBlock]) {
    let (w, h) = (f64::from(page.width), f64::from(page.height));
    for b in blocks {
        let color = if is_figure(&b.kind) { FIGURE } else { TEXT };
        let x0 = (b.bbox[0].clamp(0.0, 1.0) * w) as u32;
        let y0 = (b.bbox[1].clamp(0.0, 1.0) * h) as u32;
        let x1 = (b.bbox[2].clamp(0.0, 1.0) * w) as u32;
        let y1 = (b.bbox[3].clamp(0.0, 1.0) * h) as u32;
        outline(page, (x0, y0, x1, y1), color);
        label(page, x0, y0, &b.index.to_string(), color);
    }
}

fn fill(page: &mut Raster, (x0, y0, x1, y1): (u32, u32, u32, u32), color: [u8; 3]) {
    for y in y0..y1.min(page.height) {
        for x in x0..x1.min(page.width) {
            let i = ((y * page.width + x) * 3) as usize;
            page.rgb[i..i + 3].copy_from_slice(&color);
        }
    }
}

fn outline(page: &mut Raster, (x0, y0, x1, y1): (u32, u32, u32, u32), color: [u8; 3]) {
    fill(page, (x0, y0, x1, y0 + LINE), color);
    fill(page, (x0, y1.saturating_sub(LINE), x1, y1), color);
    fill(page, (x0, y0, x0 + LINE, y1), color);
    fill(page, (x1.saturating_sub(LINE), y0, x1, y1), color);
}

fn label(page: &mut Raster, x: u32, y: u32, text: &str, color: [u8; 3]) {
    let digits: Vec<usize> = text
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|d| usize::from(d - b'0'))
        .collect();
    let pad = CELL;
    let width = digits.len() as u32 * 4 * CELL - CELL + 2 * pad;
    let height = 5 * CELL + 2 * pad;
    fill(page, (x, y, x + width, y + height), color);
    for (k, d) in digits.iter().enumerate() {
        let left = x + pad + k as u32 * 4 * CELL;
        for (row, bits) in DIGITS[*d].iter().enumerate() {
            for col in 0..3u32 {
                if bits & (4 >> col) != 0 {
                    let cx = left + col * CELL;
                    let cy = y + pad + row as u32 * CELL;
                    fill(page, (cx, cy, cx + CELL, cy + CELL), LABEL_INK);
                }
            }
        }
    }
}
