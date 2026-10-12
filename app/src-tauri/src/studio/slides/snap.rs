//! Moves a crop's edges off ink, so no line of text or figure is sliced.
//! A probe render around the region gives, per row and column, whether it
//! holds ink (a pixel far from the page's background). Each edge then:
//!
//! * where the blocks' box itself cuts through ink (a box too tight, a
//!   caption under it), extends out to the next blank row, within
//!   [`LIMIT`] of the page; failing that, pulls back to the blank before it;
//! * where ink lies in the pad, extends over it when it is a figure's
//!   caption or label (within [`NEAR_PT`], owned by no other block, nearer
//!   the picked blocks than any other), else pulls back before it;
//! * else keeps the pad.

use serde::{Deserialize, Serialize};

use super::geometry::{round4, Frac};
use super::raster::Raster;

/// How far an edge may move out past the blocks' box, as a fraction of the
/// page's side.
pub const LIMIT: f64 = 0.06;
/// About two lines of text: ink this near a figure's box is its caption.
pub const NEAR_PT: f64 = 34.0;
/// A channel this far from the background is ink; less is anti-aliasing.
const TOLERANCE: u8 = 40;
/// The probe's pixels at most, and its scale bounds in px a point.
const PROBE_PIXELS: f64 = 3_000_000.0;
const PROBE_SCALE: (f64, f64) = (0.5, 3.0);

/// What happened to one edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    /// The pad, with no ink in it.
    Pad,
    /// Pulled back to the blank before ink.
    Trim,
    /// Moved out to take in the ink it would have cut.
    Extend,
    /// Ink at the edge with no blank row within reach either way.
    Cut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edges {
    pub top: Edge,
    pub right: Edge,
    pub bottom: Edge,
    pub left: Edge,
}

/// What the snap knows about the page.
pub struct Context<'a> {
    /// Points, as rendered.
    pub page: (f64, f64),
    /// The picked blocks' union (or the asked-for box).
    pub wanted: Frac,
    /// `wanted` with its pad.
    pub padded: Frac,
    /// Every block on the page, and whether it is picked.
    pub blocks: &'a [(Frac, bool)],
    /// The pick holds a figure, chart or table, so a caption may join it.
    pub figure: bool,
}

/// The region a probe renders: the pad and [`LIMIT`] round `wanted`.
pub fn probe_region(ctx: &Context<'_>) -> Frac {
    let (w, p) = (ctx.wanted, ctx.padded);
    [
        (p[0].min(w[0] - LIMIT)).max(0.0),
        (p[1].min(w[1] - LIMIT)).max(0.0),
        (p[2].max(w[2] + LIMIT)).min(1.0),
        (p[3].max(w[3] + LIMIT)).min(1.0),
    ]
}

/// The probe's pixel size for `region`.
pub fn probe_pixels(region: &Frac, page: (f64, f64)) -> (u32, u32) {
    let (w, h) = (
        (region[2] - region[0]) * page.0,
        (region[3] - region[1]) * page.1,
    );
    let s = (PROBE_PIXELS / (w * h))
        .sqrt()
        .clamp(PROBE_SCALE.0, PROBE_SCALE.1);
    (
        ((w * s).round() as u32).max(1),
        ((h * s).round() as u32).max(1),
    )
}

/// The page's background: the commonest colour round the probe's border
/// (outside the blocks, so a large figure is not taken for it), to 3 bits a
/// channel.
fn background(r: &Raster) -> [u8; 3] {
    let (w, h) = (r.width as usize, r.height as usize);
    let mut counts = std::collections::HashMap::<[u8; 3], usize>::new();
    let mut add = |x: usize, y: usize| {
        let i = (y * w + x) * 3;
        let px = [r.rgb[i], r.rgb[i + 1], r.rgb[i + 2]];
        *counts.entry(px.map(|c| c & 0xf8)).or_default() += 1;
    };
    for x in 0..w {
        add(x, 0);
        add(x, h - 1);
    }
    for y in 0..h {
        add(0, y);
        add(w - 1, y);
    }
    let mut best: Vec<_> = counts.into_iter().collect();
    best.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    best.first().map_or([255; 3], |(c, _)| c.map(|v| v | 4))
}

/// Ink per row (`rows`) or per column, over `span` of the other axis.
fn profile(r: &Raster, bg: [u8; 3], rows: bool, span: (usize, usize)) -> Vec<bool> {
    let (w, h) = (r.width as usize, r.height as usize);
    let ink = |x: usize, y: usize| {
        let i = (y * w + x) * 3;
        (0..3).any(|c| r.rgb[i + c].abs_diff(bg[c]) > TOLERANCE)
    };
    let (n, m) = if rows { (h, w) } else { (w, h) };
    let (lo, hi) = (span.0.min(m), span.1.min(m));
    (0..n)
        .map(|a| (lo..hi).any(|b| if rows { ink(b, a) } else { ink(a, b) }))
        .collect()
}

/// One edge with the outside at higher indices: `b` is the blocks' boundary
/// (rows `b..` are outside), `e` the padded one, `limit` the furthest an
/// extension may reach; `caption(i, k)` says whether ink in rows `i..k` may
/// be taken in. Returns the new boundary.
fn solve(
    ink: &[bool],
    b: usize,
    e: usize,
    limit: usize,
    caption: impl Fn(usize, usize) -> bool,
) -> (usize, Edge) {
    let n = ink.len();
    let b = b.min(n);
    let (e, limit) = (e.clamp(b, n), limit.clamp(b, n));
    let pad = e - b;
    // Ink on both sides of the boundary, past a row of anti-aliasing.
    let crossing = b > 0 && b + 1 < n && ink[b - 1] && ink[b] && ink[b + 1];
    if !crossing {
        return band(ink, b, e, limit, &caption, Edge::Pad);
    }
    if let Some(k) = (b..limit).find(|&k| !ink[k]) {
        return band(ink, k, (k + pad).min(n), limit, &caption, Edge::Extend);
    }
    if let Some(j) = (0..b).rev().find(|&j| !ink[j]) {
        let start = (0..=j).rev().find(|&r| ink[r]).map_or(0, |r| r + 1);
        return (start + (j + 2 - start) / 2, Edge::Trim);
    }
    (b, Edge::Cut)
}

/// The pad `b..e` past a blank boundary `b`: kept when blank, else moved
/// out over a caption (then as much pad as fits before the next ink) or
/// pulled back to halfway to the ink. `kept` is what to report when the
/// boundary stays where the caller put it.
fn band(
    ink: &[bool],
    b: usize,
    e: usize,
    limit: usize,
    caption: &impl Fn(usize, usize) -> bool,
    kept: Edge,
) -> (usize, Edge) {
    let n = ink.len();
    let limit = limit.max(b).min(n);
    let next_ink = |from: usize| (from..n).find(|&k| ink[k]).unwrap_or(n);
    let Some(i) = (b..e).find(|&k| ink[k]) else {
        return (e, kept);
    };
    match (i..limit).find(|&k| !ink[k]) {
        Some(k) if caption(i, k) => (k + (e - b).min((next_ink(k) - k) / 2), Edge::Extend),
        _ if kept == Edge::Extend => (b + (i - b) / 2, Edge::Extend),
        _ => (b + (i - b) / 2, Edge::Trim),
    }
}

/// `region` (the probe's, as fractions of the page) and the probe `r`:
/// the crop with every edge off ink, and what moved.
pub fn snap(r: &Raster, region: &Frac, ctx: &Context<'_>) -> (Frac, Edges) {
    let bg = background(r);
    let (w, h) = (f64::from(r.width), f64::from(r.height));
    let (rw, rh) = (region[2] - region[0], region[3] - region[1]);
    let to_x =
        |f: f64| (((f - region[0]) / rw * w).round().max(0.0) as usize).min(r.width as usize);
    let to_y =
        |f: f64| (((f - region[1]) / rh * h).round().max(0.0) as usize).min(r.height as usize);
    let fx = |p: usize| round4(region[0] + p as f64 / w * rw);
    let fy = |p: usize| round4(region[1] + p as f64 / h * rh);
    let (wd, pd) = (ctx.wanted, ctx.padded);
    let keep = |e: Edge, padded: f64, moved: f64| if e == Edge::Pad { padded } else { moved };
    let near_x = NEAR_PT / ctx.page.0 / rw * w;
    let near_y = NEAR_PT / ctx.page.1 / rh * h;
    let lim_x = (LIMIT / rw * w).round() as usize;
    let lim_y = (LIMIT / rh * h).round() as usize;

    // Ink in rows [a, b) of a span may join when the pick is a figure, it is
    // near, and the nearest block to it (by gap along the axis) is picked.
    let owner_ok = |lo: f64, hi: f64, span: (f64, f64), vertical: bool, gap_px: f64, near: f64| {
        if !ctx.figure || gap_px > near {
            return false;
        }
        let mut best: Option<(f64, bool)> = None;
        for (b, picked) in ctx.blocks {
            let (along, across) = if vertical {
                ((b[1], b[3]), (b[0], b[2]))
            } else {
                ((b[0], b[2]), (b[1], b[3]))
            };
            if across.1 <= span.0 || across.0 >= span.1 {
                continue;
            }
            let gap = (lo - along.1).max(along.0 - hi).max(0.0);
            let better = match best {
                None => true,
                Some((g, p)) => gap < g || (gap == g && *picked && !p),
            };
            if better {
                best = Some((gap, *picked));
            }
        }
        best.is_none_or(|(_, picked)| picked)
    };

    // Top and bottom over the padded columns.
    let cols = (to_x(pd[0]), to_x(pd[2]));
    let rows_ink = profile(r, bg, true, cols);
    let n = rows_ink.len();
    let xspan = (pd[0], pd[2]);
    let (bottom, eb) = {
        let b = to_y(wd[3]);
        solve(&rows_ink, b, to_y(pd[3]), b + lim_y, |i, k| {
            owner_ok(fy(i), fy(k), xspan, true, (i - b) as f64, near_y)
        })
    };
    let rev: Vec<bool> = rows_ink.iter().rev().copied().collect();
    let (top, et) = {
        let b = n - to_y(wd[1]);
        let (k, e) = solve(&rev, b, n - to_y(pd[1]), b + lim_y, |i, k| {
            owner_ok(fy(n - k), fy(n - i), xspan, true, (i - b) as f64, near_y)
        });
        (n - k, e)
    };

    // Left and right over the rows just decided.
    let cols_ink = profile(r, bg, false, (top, bottom));
    let m = cols_ink.len();
    let yspan = (fy(top), fy(bottom));
    let (right, er) = {
        let b = to_x(wd[2]);
        solve(&cols_ink, b, to_x(pd[2]), b + lim_x, |i, k| {
            owner_ok(fx(i), fx(k), yspan, false, (i - b) as f64, near_x)
        })
    };
    let rev: Vec<bool> = cols_ink.iter().rev().copied().collect();
    let (left, el) = {
        let b = m - to_x(wd[0]);
        let (k, e) = solve(&rev, b, m - to_x(pd[0]), b + lim_x, |i, k| {
            owner_ok(fx(m - k), fx(m - i), yspan, false, (i - b) as f64, near_x)
        });
        (m - k, e)
    };
    (
        // An edge left at its pad keeps the exact padded value, not the
        // probe's pixel rounding of it.
        [
            keep(el, pd[0], fx(left)),
            keep(et, pd[1], fy(top)),
            keep(er, pd[2], fx(right)),
            keep(eb, pd[3], fy(bottom)),
        ],
        Edges {
            top: et,
            right: er,
            bottom: eb,
            left: el,
        },
    )
}
