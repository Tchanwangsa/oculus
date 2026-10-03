//! Canvas calendar: timetabled classes (`type=event`) and dated coursework
//! (`type=assignment`), flattened into one row shape. See `docs/calendar.md`.
//!
//! - Canvas expands a repeating class server-side into one event per
//!   occurrence; `all_events=true` fetches the whole semester.
//! - A sectioned class is a parent spanning every section plus per-section
//!   `child_events`: children replace the parent, filtered to this user's
//!   sections when known ([`my_section_codes`]).

use std::collections::HashSet;

use crate::canvas::Canvas;

/// One dated item. `start_at`/`end_at` are Canvas's UTC strings, verbatim.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CalendarEvent {
    /// `event_123` / `assignment_456` — stable across syncs, the upsert key.
    pub id: String,
    /// `class` for a scheduled event, `due` for a deadline.
    pub kind: String,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
    pub location: Option<String>,
    pub url: Option<String>,
    /// Markdown, converted from Canvas HTML.
    pub description: Option<String>,
}

pub const KIND_CLASS: &str = "class";
pub const KIND_DUE: &str = "due";

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

/// Parents → occurrences, deduplicated.
fn flatten_events(raw: &[serde_json::Value], sections: &HashSet<String>) -> Vec<CalendarEvent> {
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

fn event_row(ev: &serde_json::Value) -> Option<CalendarEvent> {
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
fn clean_title(title: &str, context_name: &str) -> String {
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
fn clean_location(raw: &str) -> String {
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
fn context_id(v: &serde_json::Value, prefix: &str) -> Option<String> {
    match &v["id"] {
        serde_json::Value::Number(n) => Some(format!("{prefix}_{n}")),
        serde_json::Value::String(s) if s.contains('_') => Some(s.clone()),
        serde_json::Value::String(s) => Some(format!("{prefix}_{s}")),
        _ => None,
    }
}

fn markdown_of(html: Option<&str>) -> Option<String> {
    let md = crate::md::to_markdown(html?, &Default::default());
    (!md.trim().is_empty()).then_some(md)
}

// ── Tauri command ─────────────────────────────────────────────────────────────

/// Fetch one course's calendar; the frontend's `upsertCalendarEvents` stores it.
#[tauri::command]
pub async fn calendar_sync_events(
    canvas_course_id: i64,
) -> Result<Vec<CalendarEvent>, String> {
    crate::blocking::run(move || {
        let canvas = Canvas::open(&crate::paths::data_dir());
        if !canvas.has_session() {
            return Err("Not signed in to Canvas.".to_string());
        }
        fetch(&canvas, canvas_course_id)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(json: &str) -> serde_json::Value {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn numeric_and_prefixed_ids_both_normalise() {
        assert_eq!(context_id(&ev(r#"{"id":12}"#), "event").unwrap(), "event_12");
        assert_eq!(
            context_id(&ev(r#"{"id":"assignment_9"}"#), "assignment").unwrap(),
            "assignment_9"
        );
        assert!(context_id(&ev(r#"{}"#), "event").is_none());
    }

    #[test]
    fn timetabled_titles_lose_the_course_prefix_and_keep_the_group() {
        assert_eq!(
            clean_title(
                "IT Project (COMP30022_2026_SM2) (Tutorial 1 (16))",
                "IT Project (COMP30022_2026_SM2)"
            ),
            "Tutorial 1 (16)"
        );
        assert_eq!(
            clean_title(
                "Models of Computation (COMP30026_2026_SM2) (Lecture 2 (1))",
                "Models of Computation (COMP30026_2026_SM2)"
            ),
            "Lecture 2 (1)"
        );
        assert_eq!(
            clean_title("Week 9 Tutorial Wireshark Monday 12pm", "Information Security"),
            "Week 9 Tutorial Wireshark Monday 12pm"
        );
        assert_eq!(clean_title("(a) then (b)", ""), "(a) then (b)");
        assert_eq!(clean_title("Course X", "Course X"), "Course X");
    }

    #[test]
    fn venues_lose_the_capacity_and_keep_the_room() {
        assert_eq!(
            clean_location("PAR-192-L2-L108-Laby Theatre (210)"),
            "PAR-192-L2-L108-Laby Theatre"
        );
        assert_eq!(
            clean_location("X-Departmentally Organised Venue"),
            "X-Departmentally Organised Venue"
        );
        assert_eq!(clean_location("Room 4 (online)"), "Room 4 (online)");
    }

    #[test]
    fn undated_and_deleted_events_are_dropped() {
        assert!(event_row(&ev(r#"{"id":1,"title":"x"}"#)).is_none());
        assert!(event_row(&ev(
            r#"{"id":1,"start_at":"2026-08-10T01:00:00Z","workflow_state":"deleted"}"#
        ))
        .is_none());
    }

    #[test]
    fn children_replace_their_parent_filtered_to_my_sections() {
        let raw = ev(r#"[{
            "id": 1, "title": "Tutorial", "start_at": "2026-08-10T01:00:00Z",
            "child_events": [
              {"id": 2, "title": "Tutorial 01", "start_at": "2026-08-10T01:00:00Z",
               "context_code": "course_section_11"},
              {"id": 3, "title": "Tutorial 02", "start_at": "2026-08-10T03:00:00Z",
               "context_code": "course_section_22"}
            ]
        }]"#);
        let mine: HashSet<String> = ["course_section_22".to_string()].into_iter().collect();

        let kept = flatten_events(raw.as_array().unwrap(), &mine);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].title, "Tutorial 02");
        assert_eq!(kept[0].id, "event_3");
    }

    #[test]
    fn unknown_sections_keep_every_child_not_none() {
        let raw = ev(r#"[{
            "id": 1, "start_at": "2026-08-10T01:00:00Z",
            "child_events": [
              {"id": 2, "title": "A", "start_at": "2026-08-10T01:00:00Z",
               "context_code": "course_section_11"},
              {"id": 3, "title": "B", "start_at": "2026-08-10T03:00:00Z",
               "context_code": "course_section_22"}
            ]
        }]"#);
        let kept = flatten_events(raw.as_array().unwrap(), &HashSet::new());
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn the_same_occurrence_arriving_twice_is_kept_once() {
        let raw = ev(r#"[
          {"id": 1, "start_at": "2026-08-10T01:00:00Z", "hidden": true,
           "child_events": [
             {"id": 2, "title": "Tutorial 02", "start_at": "2026-08-10T01:00:00Z",
              "context_code": "course_section_22"}]},
          {"id": 2, "title": "Tutorial 02", "start_at": "2026-08-10T01:00:00Z",
           "context_code": "course_section_22", "parent_event_id": 1}
        ]"#);
        let mine: HashSet<String> = ["course_section_22".to_string()].into_iter().collect();
        let kept = flatten_events(raw.as_array().unwrap(), &mine);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn a_hidden_parent_with_no_children_in_hand_is_skipped() {
        let raw = ev(r#"[{"id": 1, "title": "Tutorial", "hidden": true,
                          "start_at": "2026-08-10T01:00:00Z"}]"#);
        assert!(flatten_events(raw.as_array().unwrap(), &HashSet::new()).is_empty());
    }

    #[test]
    fn a_childless_event_is_kept_as_itself() {
        let raw = ev(r#"[{"id": 7, "title": "Lecture", "start_at": "2026-08-10T01:00:00Z",
                          "end_at": "2026-08-10T02:00:00Z", "location_name": "  Alice Hoy 210 "}]"#);
        let kept = flatten_events(raw.as_array().unwrap(), &HashSet::new());
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].location.as_deref(), Some("Alice Hoy 210"));
        assert_eq!(kept[0].kind, KIND_CLASS);
    }
}
