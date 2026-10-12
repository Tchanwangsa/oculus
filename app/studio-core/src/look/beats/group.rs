//! Changes into lines: one entity's span changes that start together are
//! one change (`move+resize`), then the same change at the same sample on
//! keys that differ only in a trailing number (`d12`, `d13`, `d14`) is one
//! line, `d12-d14`, with the group's place or destinations.

use super::{Change, centre};
use std::collections::BTreeMap;

/// One line: its first and last sample, event, who and detail.
pub(super) struct Line {
    pub start: usize,
    pub end: usize,
    pub event: String,
    pub who: String,
    pub detail: String,
}

/// A group: its sample, its first member's rank, its event and stem.
type Key = (usize, usize, String, String);

/// One line per group.
pub(super) fn group(changes: Vec<Change>, order: &[String]) -> Vec<Line> {
    let rank = |name: &str| order.iter().position(|n| n == name).unwrap_or(usize::MAX);
    let mut groups: BTreeMap<Key, Vec<Change>> = BTreeMap::new();
    let mut index: BTreeMap<(usize, String, String), Key> = BTreeMap::new();
    for c in together(changes) {
        let stem = numbered(&c.name).0.unwrap_or(&c.name).to_owned();
        let event = c.events.join("+");
        // Members join the group of their stem at this sample, which is
        // ranked by its first member so lines keep the scene's order.
        let key = index
            .entry((c.start, event.clone(), stem.clone()))
            .or_insert_with(|| (c.start, rank(&c.name), event, stem))
            .clone();
        groups.entry(key).or_default().push(c);
    }
    groups
        .into_iter()
        .map(|((start, _, event, _), mut members)| {
            members.sort_by_key(|c| (numbered(&c.name).1, rank(&c.name)));
            let end = members.iter().map(|c| c.end).max().unwrap_or(start);
            let who = ranges(&members.iter().map(|c| c.name.as_str()).collect::<Vec<_>>());
            let detail = members[0]
                .events
                .iter()
                .map(|e| detail(e, &members))
                .filter(|d| !d.is_empty())
                .collect::<Vec<_>>()
                .join("; ");
            Line {
                start,
                end,
                event,
                who,
                detail,
            }
        })
        .collect()
}

/// One entity's span changes that start at the same sample, merged; a
/// span starting as it appears is part of the appearing (a draw-in or a
/// pop), so the appear runs to its end and reports where it settles.
fn together(changes: Vec<Change>) -> Vec<Change> {
    let point = |e: &str| matches!(e, "appear" | "vanish");
    let mut out: Vec<Change> = Vec::new();
    for c in changes {
        let into = out.iter_mut().find(|o| {
            !point(c.events[0]) && o.name == c.name && o.start == c.start && o.events[0] != "vanish"
        });
        match into {
            Some(o) => {
                if o.events[0] != "appear" {
                    o.events.extend(c.events);
                }
                if c.end > o.end {
                    o.end = c.end;
                    o.to = c.to;
                }
            }
            None => out.push(c),
        }
    }
    out
}

/// The name split as `(stem, number)` when it ends in digits after a
/// non-empty stem.
fn numbered(name: &str) -> (Option<&str>, Option<u64>) {
    let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if stem.is_empty() || stem.len() == name.len() {
        return (None, None);
    }
    (Some(stem), name[stem.len()..].parse().ok())
}

/// `d0-d3,d5`: runs of consecutive numbers, by their own spelling.
fn ranges(names: &[&str]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < names.len() {
        let mut j = i;
        while j + 1 < names.len() {
            match (numbered(names[j]).1, numbered(names[j + 1]).1) {
                (Some(a), Some(b)) if b == a + 1 => j += 1,
                _ => break,
            }
        }
        parts.push(if j > i {
            format!("{}-{}", names[i], names[j])
        } else {
            names[i].to_owned()
        });
        i = j + 1;
    }
    parts.join(",")
}

fn detail(event: &str, members: &[Change]) -> String {
    let point = |p: vello_cpu::kurbo::Point| format!("({:.0},{:.0})", p.x, p.y);
    let one = members.len() == 1;
    let first = &members[0];
    let last = &members[members.len() - 1];
    match event {
        "appear" if one => format!(
            "at {} {:.0}x{:.0}",
            point(centre(first.to.bbox)),
            first.to.bbox.width(),
            first.to.bbox.height()
        ),
        "appear" => {
            let all = members
                .iter()
                .map(|c| c.to.bbox)
                .reduce(|a, b| a.union(b))
                .unwrap_or(first.to.bbox);
            format!("(centre {})", point(centre(all)))
        }
        "move" if one => format!(
            "{} → {}",
            point(centre(first.from.bbox)),
            point(centre(first.to.bbox))
        ),
        "move" => format!(
            "→ {}…{}",
            point(centre(first.to.bbox)),
            point(centre(last.to.bbox))
        ),
        "resize" => format!(
            "{:.0}x{:.0} → {:.0}x{:.0}",
            first.from.bbox.width(),
            first.from.bbox.height(),
            first.to.bbox.width(),
            first.to.bbox.height()
        ),
        "rewrite" if one => match (&first.from.math, &first.to.math) {
            (Some((a, _)), Some((b, _))) => pieces(a, b),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// A Math's pieces shown after and not before (`+val`), and before and
/// not after (`-add`), at most eight.
fn pieces(before: &[String], after: &[String]) -> String {
    let added = after
        .iter()
        .filter(|p| !before.contains(p))
        .map(|p| format!("+{p}"));
    let gone = before
        .iter()
        .filter(|p| !after.contains(p))
        .map(|p| format!("-{p}"));
    let all: Vec<String> = added.chain(gone).collect();
    let more = all.len().saturating_sub(8);
    let mut s = all.into_iter().take(8).collect::<Vec<_>>().join(" ");
    if more > 0 {
        s.push_str(&format!(" (+{more} more)"));
    }
    s
}
