//! Turning Canvas's JSON into [`CalendarEvent`] rows.

use super::{CalendarEvent, KIND_CLASS};

use std::collections::HashSet;

/// Parents → occurrences, deduplicated.
pub(super) fn flatten_events(
    raw: &[serde_json::Value],
    sections: &HashSet<String>,
) -> Vec<CalendarEvent> {
    let mut out: Vec<CalendarEvent> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push = |e: CalendarEvent, out: &mut Vec<CalendarEvent>| {
        if seen.insert(e.id.clone()) {
            out.push(e);
        }
    };

    for ev in raw {
        let children: Vec<&serde_json::Value> = ev["child_events"]
            .as_array()
            .map(|c| c.iter().collect())
            .unwrap_or_default();

        if children.is_empty() {
            // A `hidden` parent is meant to be drawn as its children.
            if ev["hidden"].as_bool().unwrap_or(false) {
                continue;
            }
            if let Some(row) = event_row(ev) {
                push(row, &mut out);
            }
            continue;
        }

        // Keep this student's sections; if none match, keep all rather than
        // drop the course's whole timetable.
        let mine: Vec<&&serde_json::Value> = children
            .iter()
            .filter(|c| {
                c["context_code"]
                    .as_str()
                    .is_some_and(|code| sections.contains(code))
            })
            .collect();
        if mine.is_empty() {
            for c in &children {
                if let Some(row) = event_row(c) {
                    push(row, &mut out);
                }
            }
        } else {
            for c in &mine {
                if let Some(row) = event_row(c) {
                    push(row, &mut out);
                }
            }
        }
    }
    out
}

pub(super) fn event_row(ev: &serde_json::Value) -> Option<CalendarEvent> {
    // A cancelled or deleted occurrence keeps its row in the API response.
    if ev["workflow_state"].as_str() == Some("deleted") {
        return None;
    }
    let start = ev["start_at"].as_str()?;
    Some(CalendarEvent {
        id: context_id(ev, "event")?,
        kind: KIND_CLASS.to_string(),
        title: clean_title(
            ev["title"].as_str().unwrap_or("Untitled"),
            ev["context_name"].as_str().unwrap_or(""),
        ),
        start_at: start.to_string(),
        end_at: ev["end_at"].as_str().map(str::to_string),
        all_day: ev["all_day"].as_bool().unwrap_or(false),
        location: ev["location_name"]
            .as_str()
            .map(clean_location)
            .filter(|s| !s.is_empty()),
        url: ev["html_url"].as_str().map(str::to_string),
        description: markdown_of(ev["description"].as_str()),
    })
}

/// "IT Project (COMP30022_2026_SM2) (Tutorial 1 (16))" → "Tutorial 1 (16)":
/// strip the course-name prefix and one outer pair of parens.
pub(super) fn clean_title(title: &str, context_name: &str) -> String {
    let mut t = title.trim();
    if !context_name.is_empty() {
        if let Some(rest) = t.strip_prefix(context_name) {
            t = rest.trim();
        }
    }
    if let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        // Only when that pair is outermost — "(a) and (b)" survives.
        if balanced(inner) {
            t = inner.trim();
        }
    }
    if t.is_empty() {
        title.trim().to_string()
    } else {
        t.to_string()
    }
}

fn balanced(s: &str) -> bool {
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

/// Drop the room capacity: "PAR-192-L2-L108-Laby Theatre (210)" → "… Theatre".
pub(super) fn clean_location(raw: &str) -> String {
    let t = raw.trim();
    let Some(open) = t.rfind(" (") else {
        return t.to_string();
    };
    let tail = &t[open + 2..];
    match tail.strip_suffix(')') {
        Some(n) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => {
            t[..open].trim().to_string()
        }
        _ => t.to_string(),
    }
}

/// Events carry a numeric id, assignments a prefixed string; normalise both to
/// a prefixed key so the kinds cannot collide.
pub(super) fn context_id(v: &serde_json::Value, prefix: &str) -> Option<String> {
    match &v["id"] {
        serde_json::Value::Number(n) => Some(format!("{prefix}_{n}")),
        serde_json::Value::String(s) if s.contains('_') => Some(s.clone()),
        serde_json::Value::String(s) => Some(format!("{prefix}_{s}")),
        _ => None,
    }
}

pub(super) fn markdown_of(html: Option<&str>) -> Option<String> {
    let md = crate::pages::md::to_markdown(html?, &Default::default());
    (!md.trim().is_empty()).then_some(md)
}
