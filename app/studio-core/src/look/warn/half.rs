//! Everything on screen kept to one half of the frame, small, for a while:
//! the other half sits empty.

use super::super::entities::Entity;
use super::super::limits::{HALF_SIZE, HALF_SLACK, HALF_SPAN};
use super::super::sample::SceneLook;
use super::super::words::while_said;
use super::{Warning, span};
use vello_cpu::kurbo::Rect;

pub(super) fn check(look: &SceneLook, shown: &[Vec<Entity>], out: &mut Vec<Warning>) {
    let (w, h) = look.size;
    let sides: Vec<Option<(&'static str, Rect)>> =
        shown.iter().map(|e| side(e, look.size)).collect();
    let samples = &look.samples;
    let mut i = 0;
    while i < samples.len() {
        let Some((which, first)) = sides[i] else {
            i += 1;
            continue;
        };
        let mut union = first;
        let mut j = i;
        while let Some(Some((next, b))) = sides.get(j + 1)
            && *next == which
        {
            union = union.union(*b);
            j += 1;
        }
        let (a, b) = (samples[i].t, samples[j].t);
        if b - a > HALF_SPAN {
            let (along, lo, hi, of) = match which {
                "left" | "right" => ("x", union.x0, union.x1, w),
                _ => ("y", union.y0, union.y1, h),
            };
            out.push(Warning {
                kind: "half_frame",
                noun: "half-frame span",
                short: format!("{which} {}", span(a, b)),
                message: format!(
                    "everything on screen sits in the {which} half ({along} {lo:.0}-{hi:.0} of {of:.0} px) from {a:.1} s to {b:.1} s{}; the {} half is empty: use the whole frame",
                    while_said(&look.words, a, b),
                    opposite(which)
                ),
            });
        }
        i = j + 1;
    }
}

/// The half the shown content keeps to, and its box, when it is small.
fn side(entities: &[Entity], (w, h): (f64, f64)) -> Option<(&'static str, Rect)> {
    let frame = Rect::new(0.0, 0.0, w, h);
    let b = entities
        .iter()
        .map(|e| e.bbox.intersect(frame))
        .filter(|b| b.area() > 0.0)
        .reduce(|a, b| a.union(b))?;
    let (sx, sy) = (HALF_SLACK * w, HALF_SLACK * h);
    let narrow = b.width() < HALF_SIZE * w;
    let short = b.height() < HALF_SIZE * h;
    if narrow && b.x1 <= w / 2.0 + sx {
        return Some(("left", b));
    }
    if narrow && b.x0 >= w / 2.0 - sx {
        return Some(("right", b));
    }
    if h > w && short && b.y1 <= h / 2.0 + sy {
        return Some(("top", b));
    }
    if h > w && short && b.y0 >= h / 2.0 - sy {
        return Some(("bottom", b));
    }
    None
}

fn opposite(side: &str) -> &'static str {
    match side {
        "left" => "right",
        "right" => "left",
        "top" => "bottom",
        _ => "top",
    }
}
