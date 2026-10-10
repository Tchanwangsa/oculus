//! Reading-order layout of a page's glyphs into lines.
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

/// One character on the page and its box, `[x0, y0, x1, y1]` in points, y down.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TextGlyph {
    pub(super) ch: char,
    pub(super) rect: [f32; 4],
}

/// Page box → the frame where text runs left to right, for text running
/// `quarter` × 90° clockwise from that in a `w` × `h` page (1: downwards).
pub(super) fn to_upright(r: [f32; 4], quarter: usize, w: f32, h: f32) -> [f32; 4] {
    let [x0, y0, x1, y1] = r;
    match quarter {
        1 => [y0, w - x1, y1, w - x0],
        2 => [w - x1, h - y1, w - x0, h - y0],
        3 => [h - y1, x0, h - y0, x1],
        _ => r,
    }
}

/// The inverse of [`to_upright`].
pub(super) fn from_upright(r: [f32; 4], quarter: usize, w: f32, h: f32) -> [f32; 4] {
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

pub(super) fn union(a: &mut [f32; 4], b: &[f32; 4]) {
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
pub(super) fn layout(mut glyphs: Vec<TextGlyph>) -> Vec<Vec<(TextGlyph, bool)>> {
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
