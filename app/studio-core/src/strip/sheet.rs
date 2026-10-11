//! The sheet's geometry: tiles `cols` a row between gutters, a label bar
//! under each (`labels`), and the frames copied in pixel for pixel.

use super::bad;
use super::labels::{Label, Painter};
use crate::error::Result;
use crate::render::Frame;

/// Around and between tiles, in sheet px.
const GUTTER: u32 = 8;
/// The largest sheet, in pixels (256 MB of RGBA).
const MAX_PIXELS: u64 = 64 << 20;

pub struct Sheet {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub tile: (u32, u32),
    pub cols: usize,
}

/// The sheet's size for `n` tiles, or an error when it is too large.
pub fn fits(tile: (u32, u32), n: usize, cols: usize, bar: u32) -> Result<(u32, u32)> {
    let cols = cols.clamp(1, n.max(1));
    let rows = n.div_ceil(cols) as u64;
    let width = u64::from(GUTTER) + cols as u64 * (u64::from(tile.0) + u64::from(GUTTER));
    let height = u64::from(GUTTER) + rows * (u64::from(tile.1) + u64::from(bar + GUTTER));
    if width > u64::from(u16::MAX) || height > u64::from(u16::MAX) || width * height > MAX_PIXELS {
        return Err(bad(format!(
            "the sheet would be {width}x{height} px; lower the scale or the number of frames"
        )));
    }
    Ok((width as u32, height as u32))
}

pub fn draw(frames: &[Frame], labels: &[Label], cols: usize) -> Result<Sheet> {
    let first = frames
        .first()
        .ok_or_else(|| bad("no frames to tile".to_owned()))?;
    let (tw, th) = (first.width, first.height);
    let mut painter = Painter::new();
    let bars = painter.bars(labels, tw)?;
    let cols = cols.min(frames.len());
    let (width, height) = fits((tw, th), frames.len(), cols, bars.height)?;
    let cell = (tw + GUTTER, th + bars.height + GUTTER);
    let origin = |i: usize| {
        let (c, r) = ((i % cols) as u32, (i / cols) as u32);
        (GUTTER + c * cell.0, GUTTER + r * cell.1)
    };
    let under: Vec<(u32, u32)> = (0..frames.len())
        .map(|i| (origin(i).0, origin(i).1 + th))
        .collect();
    let mut rgba = painter.paint((width, height), &under, labels, tw, bars)?;
    let row = tw as usize * 4;
    for (i, f) in frames.iter().enumerate() {
        let (x, y) = origin(i);
        for (dy, src) in f.rgba.chunks_exact(row).enumerate() {
            let at = ((y as usize + dy) * width as usize + x as usize) * 4;
            rgba[at..at + row].copy_from_slice(src);
        }
    }
    Ok(Sheet {
        rgba,
        width,
        height,
        tile: (tw, th),
        cols,
    })
}
