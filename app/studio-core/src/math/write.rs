//! `<Write>`: each piece's outline is stroked on, then its fill fades in
//! as the stroke fades (manim's DrawBorderThenFill), the pieces of a group
//! one after another in reading order (`lag`). Groups are the formula's
//! `\k` groups, each with its own progress, so each can be cued on its own
//! word; a piece in no listed group writes with the nearest listed group
//! before it in reading order, else with the first one. A rule wipes on
//! from the left; a stroked path (`\cancel`) is drawn on.

use super::{Formula, Shape};
use crate::easing::in_out_cubic;
use crate::scene::{Color, Fill, Inset, LineCap, LineJoin, Node, StrokeSpec, WriteSpec};

/// Each item's own progress, 0 (not started) to 1 (written).
pub fn plan(f: &Formula, w: &WriteSpec) -> Result<Vec<f32>, String> {
    let index = |name: &str| {
        w.groups
            .iter()
            .position(|(g, _)| g.as_deref() == Some(name))
    };
    let whole = w.groups.iter().position(|(g, _)| g.is_none());
    let mut assigned: Vec<Option<usize>> = Vec::with_capacity(f.items.len());
    if let Some(i) = whole {
        assigned.resize(f.items.len(), Some(i));
    } else {
        for (g, _) in &w.groups {
            let name = g.as_deref().unwrap_or_default();
            if !f.items.iter().any(|it| it.groups.iter().any(|x| x == name)) {
                return Err(unknown(name, f));
            }
        }
        let mut current = None;
        for item in &f.items {
            if let Some(i) = item.groups.iter().find_map(|g| index(g)) {
                current = Some(i);
            }
            assigned.push(current);
        }
        let first = assigned.iter().flatten().next().copied();
        for a in &mut assigned {
            a.get_or_insert_with(|| first.unwrap_or(0));
        }
    }
    let mut out = vec![0.0; f.items.len()];
    for (gi, (_, p)) in w.groups.iter().enumerate() {
        let members: Vec<usize> = (0..f.items.len())
            .filter(|&i| assigned[i] == Some(gi))
            .collect();
        let n = members.len();
        let lag = w.lag.unwrap_or((4.0 / (n as f32 + 1.0)).min(0.2));
        let total = n.saturating_sub(1) as f32 * lag + 1.0;
        for (k, &i) in members.iter().enumerate() {
            out[i] = (p * total - k as f32 * lag).clamp(0.0, 1.0);
        }
    }
    Ok(out)
}

fn unknown(name: &str, f: &Formula) -> String {
    let mut names: Vec<&str> = f
        .items
        .iter()
        .flat_map(|it| it.groups.iter().map(String::as_str))
        .collect();
    names.sort_unstable();
    names.dedup();
    let have = if names.is_empty() {
        "it has no \\k groups; give a number to write it whole".to_owned()
    } else {
        format!("its groups are {}", names.join(", "))
    };
    format!("<Write at>: `{name}` is not a \\k group of this formula; {have}")
}

/// A piece's node at progress `u`, in `colour`, with an outline `stroke`
/// em wide.
pub fn apply(n: &mut Node, shape: &Shape, colour: Color, u: f32, stroke: f64) {
    if u >= 1.0 {
        return;
    }
    if u <= 0.0 {
        n.style.opacity = Some(0.0);
        return;
    }
    let draw = in_out_cubic((2.0 * u).min(1.0));
    let fill = in_out_cubic((2.0 * u - 1.0).max(0.0));
    match shape {
        Shape::Rule => {
            n.style.clip_inset = Some(Inset([0.0, 1.0 - in_out_cubic(u), 0.0, 0.0]));
        }
        Shape::Path {
            stroke: Some(_), ..
        } => {
            if let Some(p) = &mut n.path {
                p.trim = Some((0.0, f64::from(in_out_cubic(u))));
            }
        }
        Shape::Glyph(_) | Shape::Path { stroke: None, .. } => {
            if let Some(p) = &mut n.path {
                p.fill = Some(Fill::Solid(colour.with_alpha_mul(fill)));
                p.stroke = Some(StrokeSpec {
                    width: stroke,
                    color: colour.with_alpha_mul(1.0 - fill),
                    cap: LineCap::Round,
                    join: LineJoin::Round,
                    dash: Vec::new(),
                    dash_offset: 0.0,
                });
                p.trim = Some((0.0, f64::from(draw)));
            }
        }
    }
}
