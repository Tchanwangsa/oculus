//! A sample's items gathered into what a viewer would name: one entity per
//! key (a keyed node and its unkeyed descendants), per Math (all its
//! pieces), and per TSX line for nodes with no key near them. Only what is
//! shown counts: painted, visible, not empty, and not a background.

use super::items::Item;
use super::limits::{BACKGROUND, VISIBLE};
use super::sample::Sample;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hasher};
use vello_cpu::kurbo::Rect;

#[derive(Clone, Debug)]
pub(crate) struct Entity {
    pub name: String,
    /// How a warning names it: `` `eq` `` or `Path at ClockMod.tsx:192:9`.
    pub label: String,
    /// The union of its shown nodes, frame px.
    pub bbox: Rect,
    /// Its most opaque shown node's effective opacity.
    pub opacity: f64,
    pub ink: u64,
    pub form: u64,
    /// A Math: the pieces shown (their keys under the Math's, else their
    /// order) and how many it has.
    pub math: Option<(Vec<String>, usize)>,
    /// Its shown nodes' boxes, for the stray-mark check.
    pub boxes: Vec<Rect>,
    /// Every node of it is under a camera (`Item::framed`).
    pub framed: bool,
}

/// Whether an item is shown content.
pub(crate) fn shown(item: &Item, frame: (f64, f64)) -> bool {
    let area = item.bbox.width().max(0.0) * item.bbox.height().max(0.0);
    item.paints && item.opacity > VISIBLE && area > 0.0 && area < BACKGROUND * frame.0 * frame.1
}

/// The entities of `sample`, in the order they are first painted.
pub(crate) fn entities(sample: &Sample, frame: (f64, f64), component: &str) -> Vec<Entity> {
    let mut out: Vec<Entity> = Vec::new();
    let mut at: HashMap<&str, usize> = HashMap::new();
    let mut hashes: Vec<(DefaultHasher, DefaultHasher)> = Vec::new();
    for item in &sample.items {
        if !shown(item, frame) {
            continue;
        }
        let i = *at.entry(item.entity.as_str()).or_insert_with(|| {
            out.push(Entity {
                name: item.entity.clone(),
                label: label(item, component),
                bbox: item.bbox,
                opacity: 0.0,
                ink: 0,
                form: 0,
                math: None,
                boxes: Vec::new(),
                framed: true,
            });
            hashes.push((DefaultHasher::new(), DefaultHasher::new()));
            out.len() - 1
        });
        let e = &mut out[i];
        e.bbox = e.bbox.union(item.bbox);
        e.boxes.push(item.bbox);
        e.opacity = e.opacity.max(item.opacity);
        e.framed &= item.framed;
        hashes[i].0.write_u64(item.ink);
        hashes[i].1.write_u64(item.form);
        if let Some(m) = item.math {
            let total = sample.items[m].pieces.unwrap_or(0);
            let piece = match &item.key {
                Some(k) => k
                    .strip_prefix(&format!("{}/", item.entity))
                    .unwrap_or(k)
                    .to_owned(),
                None => format!("#{}", e.math.as_ref().map_or(0, |(p, _)| p.len())),
            };
            hashes[i].1.write(piece.as_bytes());
            let (pieces, _) = e.math.get_or_insert_with(|| (Vec::new(), total));
            pieces.push(piece);
        }
    }
    for (e, (ink, form)) in out.iter_mut().zip(hashes) {
        e.ink = ink.finish();
        e.form = form.finish();
    }
    out
}

fn label(item: &Item, component: &str) -> String {
    if item.keyed {
        return format!("`{}`", item.entity);
    }
    match item.loc {
        Some((line, col)) => format!("{} at {component}.tsx:{line}:{col}", item.kind.name()),
        None => format!("a {} with no key", item.kind.name()),
    }
}
