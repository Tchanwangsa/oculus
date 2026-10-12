//! What is left on screen at the scene's end that looks like debris: a
//! Math down to a piece or two of a rewrite (a stray `= .`), or a tiny
//! mark with nothing near it. Judged on the last sample, once the last
//! word is said.

use super::super::entities::Entity;
use super::super::limits::{FRAGMENT_PIECES, FRAGMENT_SHARE, ORPHAN_GAP, ORPHAN_PX, TAIL};
use super::super::sample::SceneLook;
use super::{Warning, secs};
use vello_cpu::kurbo::Rect;

pub(super) fn check(look: &SceneLook, shown: &[Vec<Entity>], out: &mut Vec<Warning>) {
    let (Some(last), Some(entities)) = (look.samples.last(), shown.last()) else {
        return;
    };
    let said = look.words.last().map_or(0.0, |w| w.end);
    let from = (look.length - TAIL).max(said);
    if last.t < from {
        return;
    }
    let at = secs(last.t);
    for e in entities {
        if let Some((pieces, total)) = &e.math
            && pieces.len() <= FRAGMENT_PIECES
            && (pieces.len() as f64) < FRAGMENT_SHARE * *total as f64
        {
            let names: Vec<String> = pieces.iter().map(|p| format!("`{p}`")).collect();
            out.push(Warning {
                kind: "fragment",
                noun: "fragment",
                short: format!("{} {} of {total} pieces", e.label, pieces.len()),
                message: format!(
                    "{} ends showing {} of its {total} pieces ({}) at {at}: a leftover of a rewrite; hide the whole Math or finish it",
                    e.label,
                    pieces.len(),
                    names.join(", ")
                ),
            });
            continue;
        }
        let stray = e.boxes.iter().find(|b| {
            b.width() < ORPHAN_PX
                && b.height() < ORPHAN_PX
                && !entities
                    .iter()
                    .flat_map(|o| o.boxes.iter())
                    .any(|o| !std::ptr::eq(o, *b) && gap(**b, *o) <= ORPHAN_GAP)
        });
        if let Some(b) = stray {
            out.push(Warning {
                kind: "fragment",
                noun: "fragment",
                short: format!("{} {:.0}x{:.0} px", e.label, b.width(), b.height()),
                message: format!(
                    "{} is a {:.0}x{:.0} px mark at ({:.0}, {:.0}) with nothing within {ORPHAN_GAP:.0} px at {at}: a stray piece left on screen",
                    e.label,
                    b.width(),
                    b.height(),
                    b.center().x,
                    b.center().y
                ),
            });
        }
    }
}

/// The distance between two boxes; 0 when they touch or overlap.
fn gap(a: Rect, b: Rect) -> f64 {
    let dx = (b.x0 - a.x1).max(a.x0 - b.x1).max(0.0);
    let dy = (b.y0 - a.y1).max(a.y0 - b.y1).max(0.0);
    dx.hypot(dy)
}
