//! Pulling a course's events and due dates from the Canvas API.

use super::parse::{context_id, flatten_events, markdown_of};
use super::{CalendarEvent, KIND_DUE};

use std::collections::HashSet;

use crate::sources::canvas::Canvas;

/// Every dated item for one course, sorted by start. Either half failing still
/// yields the other.
pub fn fetch(canvas: &Canvas, course_id: i64) -> Result<Vec<CalendarEvent>, String> {
    let sections = my_section_codes(canvas, course_id);
    let mut out = Vec::new();

    match fetch_events(canvas, course_id, &sections) {
        Ok(events) => out.extend(events),
        Err(e) => eprintln!("[oculus] calendar: course {course_id} events: {e}"),
    }
    match fetch_due(canvas, course_id) {
        Ok(due) => out.extend(due),
        Err(e) => eprintln!("[oculus] calendar: course {course_id} due dates: {e}"),
    }

    out.sort_by(|a, b| a.start_at.cmp(&b.start_at));
    Ok(out)
}

/// The `course_section_<id>` codes this user is enrolled in
/// (`include[]=sections` returns the caller's own). Empty means unknown:
/// callers then keep every child.
fn my_section_codes(canvas: &Canvas, course_id: i64) -> HashSet<String> {
    let mut out = HashSet::new();
    let Ok(v) = canvas.get_json(&format!("/api/v1/courses/{course_id}?include[]=sections")) else {
        return out;
    };
    for s in v["sections"].as_array().into_iter().flatten() {
        if let Some(id) = s["id"].as_i64() {
            out.insert(format!("course_section_{id}"));
        }
    }
    out
}

fn fetch_events(
    canvas: &Canvas,
    course_id: i64,
    sections: &HashSet<String>,
) -> Result<Vec<CalendarEvent>, String> {
    // Ask for the sections as well as the course: a section occurrence may come
    // nested or top-level, so request both and deduplicate by id.
    let contexts: String = std::iter::once(format!("&context_codes[]=course_{course_id}"))
        .chain(sections.iter().map(|c| format!("&context_codes[]={c}")))
        .collect();

    // Canvas's docs spell it both ways; an unknown parameter is ignored.
    let raw = canvas.get_all(&format!(
        "/api/v1/calendar_events?type=event&all_events=true&per_page=100\
         {contexts}&include[]=child_events&includes[]=child_events"
    ))?;
    Ok(flatten_events(&raw, sections))
}

/// Assignments and quizzes as calendar items; undated ones are dropped.
fn fetch_due(canvas: &Canvas, course_id: i64) -> Result<Vec<CalendarEvent>, String> {
    let raw = canvas.get_all(&format!(
        "/api/v1/calendar_events?type=assignment&all_events=true&per_page=100\
         &context_codes[]=course_{course_id}"
    ))?;

    Ok(raw
        .iter()
        .filter_map(|a| {
            let due = a["assignment"]["due_at"]
                .as_str()
                .or_else(|| a["start_at"].as_str())?;
            Some(CalendarEvent {
                id: context_id(a, "assignment")?,
                kind: KIND_DUE.to_string(),
                title: a["title"].as_str().unwrap_or("Untitled").to_string(),
                start_at: due.to_string(),
                end_at: None,
                all_day: false,
                location: None,
                // The event's own `html_url` is the assignment index.
                url: a["assignment"]["html_url"]
                    .as_str()
                    .or_else(|| a["html_url"].as_str())
                    .map(str::to_string),
                description: markdown_of(a["description"].as_str()),
            })
        })
        .collect())
}
