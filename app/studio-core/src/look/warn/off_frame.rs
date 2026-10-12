//! Something shown past the frame's edge and staying there: settled (not
//! sliding in or out) for two samples or more, after the transition in,
//! and not cropped on purpose by a zooming camera.

use super::super::entities::Entity;
use super::super::limits::{OFF_FRAME_PX, SETTLED_PX};
use super::super::sample::SceneLook;
use super::super::words::while_said;
use super::{Warning, span};
use std::collections::HashMap;
use vello_cpu::kurbo::Rect;

struct Run {
    label: String,
    from: f64,
    to: f64,
    samples: usize,
    edge: &'static str,
    past: f64,
    last: Rect,
}

pub(super) fn check(look: &SceneLook, shown: &[Vec<Entity>], out: &mut Vec<Warning>) {
    let mut open: HashMap<String, Run> = HashMap::new();
    let mut done: Vec<Run> = Vec::new();
    for (s, entities) in look.samples.iter().zip(shown) {
        let mut still_open: HashMap<String, Run> = HashMap::new();
        if s.t >= look.head {
            for e in entities.iter().filter(|e| !e.framed) {
                let Some((edge, past)) = beyond(e.bbox, look.size) else {
                    continue;
                };
                let run = match open.remove(&e.name) {
                    Some(mut r) if moved(r.last, e.bbox) <= SETTLED_PX => {
                        r.to = s.t;
                        r.samples += 1;
                        if past > r.past {
                            (r.edge, r.past) = (edge, past);
                        }
                        r.last = e.bbox;
                        r
                    }
                    other => {
                        done.extend(other);
                        Run {
                            label: e.label.clone(),
                            from: s.t,
                            to: s.t,
                            samples: 1,
                            edge,
                            past,
                            last: e.bbox,
                        }
                    }
                };
                still_open.insert(e.name.clone(), run);
            }
        }
        done.extend(open.into_values());
        open = still_open;
    }
    done.extend(open.into_values());
    done.retain(|r| r.samples >= 2);
    done.sort_by(|a, b| a.from.total_cmp(&b.from).then(a.label.cmp(&b.label)));
    for r in done {
        out.push(Warning {
            kind: "off_frame",
            noun: "off-frame node",
            short: format!("{} {}", r.label, span(r.from, r.to)),
            message: format!(
                "{} is {:.0} px past the {} edge from {:.1} s to {:.1} s{}; keep it inside the frame",
                r.label,
                r.past,
                r.edge,
                r.from,
                r.to,
                while_said(&look.words, r.from, r.to)
            ),
        });
    }
}

/// The edge `b` is furthest past, and by how much, when more than
/// `OFF_FRAME_PX`.
fn beyond(b: Rect, (w, h): (f64, f64)) -> Option<(&'static str, f64)> {
    [
        ("left", -b.x0),
        ("top", -b.y0),
        ("right", b.x1 - w),
        ("bottom", b.y1 - h),
    ]
    .into_iter()
    .filter(|(_, px)| *px > OFF_FRAME_PX)
    .max_by(|a, b| a.1.total_cmp(&b.1))
}

fn moved(a: Rect, b: Rect) -> f64 {
    [b.x0 - a.x0, b.y0 - a.y0, b.x1 - a.x1, b.y1 - a.y1]
        .into_iter()
        .map(f64::abs)
        .fold(0.0, f64::max)
}
