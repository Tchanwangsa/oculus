//! `beats`: what changes on screen, as text, from the same samples as the
//! warnings. Per entity (a key, a Math, or a TSX line for nodes with no
//! key near them): appears, vanishes, moves, resizes, recolours, rewrites.
//! A change that runs over consecutive samples is one beat with a start
//! and an end; beats of numbered keys at the same moment are one line
//! (`group`). Each `[pause]` is a line of its own at its start.

mod group;

use super::entities::{Entity, entities};
use super::limits::{MOVE_PX, SIZE_PX};
use super::sample::SceneLook;
use super::words;
use crate::project::CuedPause;
use serde::Serialize;
use std::collections::HashMap;
use vello_cpu::kurbo::{Point, Rect};

/// One line of the timeline.
#[derive(Clone, Debug, Serialize)]
pub struct Beat {
    /// Scene seconds it starts and ends (the same for an appear or
    /// vanish).
    pub t: f64,
    pub end: f64,
    /// `appear`, `vanish`, or what changes over the span, joined by `+`:
    /// `move`, `resize`, `recolour`, `rewrite` (`move+resize`).
    pub event: String,
    /// The entity, or a range of numbered keys (`d12-d14`).
    pub who: String,
    /// Where it lands, what it became.
    pub detail: String,
    /// A few words said at `t`, quoted, or empty.
    pub words: String,
}

impl Beat {
    /// ` 6.20  move     d12-d14  → (1180,420)…(1180,660)  "… stands up"`
    pub fn line(&self) -> String {
        let mut s = format!("{:6.2}  {:<8} {}", self.t, self.event, self.who);
        if !self.detail.is_empty() {
            s.push_str("  ");
            s.push_str(&self.detail);
        }
        if !self.words.is_empty() {
            s.push_str("  ");
            s.push_str(&self.words);
        }
        s
    }
}

/// One entity's change in progress or done, before grouping. Its changes
/// over a span that start together are one change with several events.
#[derive(Clone, Debug)]
pub(crate) struct Change {
    pub start: usize,
    pub end: usize,
    pub events: Vec<&'static str>,
    pub name: String,
    pub from: Entity,
    pub to: Entity,
}

impl SceneLook {
    /// The timeline, in time order.
    pub fn beats(&self) -> Vec<Beat> {
        let shown: Vec<HashMap<String, Entity>> = self
            .samples
            .iter()
            .map(|s| {
                entities(s, self.size, &self.component)
                    .into_iter()
                    .map(|e| (e.name.clone(), e))
                    .collect()
            })
            .collect();
        let mut order: Vec<String> = Vec::new();
        for frame in &self.samples {
            for item in &frame.items {
                if !order.contains(&item.entity) {
                    order.push(item.entity.clone());
                }
            }
        }
        let mut changes: Vec<Change> = Vec::new();
        for name in &order {
            changes.extend(changes_of(name, &shown));
        }
        let times: Vec<f64> = self.samples.iter().map(|s| s.t).collect();
        let mut beats: Vec<(usize, Beat)> = group::group(changes, &order)
            .into_iter()
            .map(
                |group::Line {
                     start,
                     end,
                     event,
                     who,
                     mut detail,
                 }| {
                    let (t, until) = (times[start], times[end]);
                    if until - t >= 0.3 {
                        let over = format!("over {:.1} s", until - t);
                        detail = [detail, over].join(" ").trim().to_owned();
                    }
                    (
                        start,
                        Beat {
                            t,
                            end: until,
                            event,
                            who,
                            detail,
                            words: words::at(&self.words, t),
                        },
                    )
                },
            )
            .collect();
        beats.sort_by_key(|(start, _)| *start);
        let mut out: Vec<Beat> = beats.into_iter().map(|(_, b)| b).collect();
        for p in &self.pauses {
            let at = out.partition_point(|b| b.t <= p.start);
            out.insert(at, self.pause_beat(p));
        }
        out
    }

    /// A `[pause]` as a line: `  4.10  pause    1.0 s  "… lands on one."`.
    fn pause_beat(&self, p: &CuedPause) -> Beat {
        let before = p
            .after
            .map(|i| words::ending(&self.words, i))
            .unwrap_or_default();
        Beat {
            t: p.start,
            end: p.end,
            event: "pause".to_owned(),
            who: format!("{:.1} s", p.end - p.start),
            detail: String::new(),
            words: before,
        }
    }
}

/// Every change of entity `name` across the samples.
fn changes_of(name: &str, shown: &[HashMap<String, Entity>]) -> Vec<Change> {
    let mut out: Vec<Change> = Vec::new();
    let mut open: Vec<Change> = Vec::new();
    for (k, frame) in shown.iter().enumerate() {
        let now = frame.get(name);
        let before = k.checked_sub(1).and_then(|p| shown[p].get(name));
        let point = |event, e: &Entity| Change {
            start: k,
            end: k,
            events: vec![event],
            name: name.to_owned(),
            from: e.clone(),
            to: e.clone(),
        };
        match (before, now) {
            (None, Some(e)) => out.push(point("appear", e)),
            (Some(e), None) => {
                out.append(&mut open);
                out.push(point("vanish", e));
            }
            (Some(a), Some(b)) => {
                let moving = [
                    ("move", centre(a.bbox).distance(centre(b.bbox)) > 0.5),
                    ("resize", size_moved(a.bbox, b.bbox) > 0.5),
                    ("recolour", a.ink != b.ink),
                    ("rewrite", a.form != b.form),
                ];
                for (event, on) in moving {
                    let at = open.iter().position(|c| c.events[0] == event);
                    match (at, on) {
                        (Some(i), true) => {
                            open[i].end = k;
                            open[i].to = b.clone();
                        }
                        (None, true) => open.push(Change {
                            start: k - 1,
                            end: k,
                            events: vec![event],
                            name: name.to_owned(),
                            from: a.clone(),
                            to: b.clone(),
                        }),
                        (Some(i), false) => out.push(open.remove(i)),
                        (None, false) => {}
                    }
                }
            }
            (None, None) => {}
        }
    }
    out.append(&mut open);
    out.retain(|c| match c.events[0] {
        "move" => centre(c.from.bbox).distance(centre(c.to.bbox)) > MOVE_PX,
        "resize" => size_moved(c.from.bbox, c.to.bbox) > SIZE_PX,
        _ => true,
    });
    out
}

pub(crate) fn centre(r: Rect) -> Point {
    r.center()
}

fn size_moved(a: Rect, b: Rect) -> f64 {
    (a.width() - b.width())
        .abs()
        .max((a.height() - b.height()).abs())
}
