//! How far anything moves between two sets of laid-out layers: each node's
//! transformed bounds, matched across the two by `key` (anywhere in the
//! tree, as the morph matches) else by its path of child indices from the
//! nearest keyed ancestor (`look::walk`). Nodes paint skips (opacity 0)
//! do not count.

use crate::layout::LNode;
use crate::look::walk;
use std::collections::HashMap;
use vello_cpu::kurbo::Rect;

/// The largest corner move of any node present in both, in layout px.
pub fn displacement(a: &[LNode], b: &[LNode]) -> f64 {
    let (ba, bb) = (bounds(a), bounds(b));
    ba.iter()
        .filter_map(|(id, ra)| {
            let rb = bb.get(id)?;
            let d = [rb.x0 - ra.x0, rb.y0 - ra.y0, rb.x1 - ra.x1, rb.y1 - ra.y1];
            Some(d.into_iter().map(f64::abs).fold(0.0, f64::max))
        })
        .fold(0.0, f64::max)
}

fn bounds(layers: &[LNode]) -> HashMap<String, Rect> {
    let mut out = HashMap::new();
    walk(layers, &(), &mut |v, _| {
        out.entry(v.id.to_owned()).or_insert_with(|| v.bounds());
        Some(())
    });
    out
}
