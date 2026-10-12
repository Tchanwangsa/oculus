//! Each node's own progress. A scope's leaves (childless nodes that paint)
//! are numbered in reading order: A's in tree order, then B's arrivals in
//! theirs, a matched B leaf taking its partner's number. A node's window is
//! the span of the numbers below it (a matched pair's, both sides'). With
//! `lag`, piece i starts `i · lag` piece-durations in and the scope runs
//! `(n - 1) · lag + 1` of them, manim's `lag_ratio`; a window of several
//! pieces runs from its first's start to its last's end. The window's
//! linear progress then goes through the scope's `ease`.

use super::flat::Tree;
use super::matching::Matching;
use crate::scene::Timing;
use std::collections::HashMap;

type Window = Option<(u32, u32)>;

/// Eased progress per slot of A and of B; a matched pair gets one value.
pub fn clocks(a: &Tree, b: &Tree, m: &Matching) -> (Vec<f32>, Vec<f32>) {
    let mut count: HashMap<Option<u32>, u32> = HashMap::new();
    let mut number = |scope: Option<u32>| {
        let c = count.entry(scope).or_default();
        *c += 1;
        *c - 1
    };
    let num_a: Vec<Option<u32>> = a
        .slots
        .iter()
        .map(|s| s.leaf.then(|| number(s.scope)))
        .collect();
    let win_a = windows(a, num_a);
    let num_b: Vec<Option<u32>> = b
        .slots
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if !s.leaf {
                return None;
            }
            let partner = m.b_to_a[i].and_then(|j| win_a[j]);
            Some(partner.map_or_else(|| number(s.scope), |w| w.0))
        })
        .collect();
    let win_b = windows(b, num_b);
    let at = |t: &Tree, i: usize, w: Window| {
        let n = count.get(&t.slots[i].scope).copied().unwrap_or(0);
        let (p, timing) = t.clock(i);
        local(*p, timing, n, w)
    };
    let pa = (0..a.slots.len())
        .map(|i| at(a, i, union(win_a[i], m.a_to_b[i].and_then(|j| win_b[j]))))
        .collect();
    let pb = (0..b.slots.len())
        .map(|i| at(b, i, union(win_b[i], m.b_to_a[i].and_then(|j| win_a[j]))))
        .collect();
    (pa, pb)
}

/// Each node's span of leaf numbers in its own scope; slots are in
/// preorder, so children come after their parent.
fn windows(t: &Tree, num: Vec<Option<u32>>) -> Vec<Window> {
    let mut w: Vec<Window> = num.into_iter().map(|n| n.map(|x| (x, x))).collect();
    for i in (0..t.slots.len()).rev() {
        for &k in &t.slots[i].kids {
            if t.slots[k].scope == t.slots[i].scope {
                w[i] = union(w[i], w[k]);
            }
        }
    }
    w
}

fn union(a: Window, b: Window) -> Window {
    match (a, b) {
        (Some(x), Some(y)) => Some((x.0.min(y.0), x.1.max(y.1))),
        (x, None) | (None, x) => x,
    }
}

/// A window's eased progress at the scope's `p`, among `n` numbers. A `p`
/// past 0..1 (an overshooting easing) carries every window past it too.
pub fn local(p: f32, t: &Timing, n: u32, w: Window) -> f32 {
    let u = match w {
        Some((lo, hi)) if t.lag > 0.0 && n > 1 => {
            let total = (n - 1) as f32 * t.lag + 1.0;
            let span = (hi - lo) as f32 * t.lag + 1.0;
            ((p * total - lo as f32 * t.lag) / span).clamp(p.min(0.0), p.max(1.0))
        }
        _ => p,
    };
    t.ease.apply(u)
}
