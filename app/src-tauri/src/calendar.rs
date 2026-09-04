//! Canvas calendar: scheduled classes and dated coursework.
//!
//! This is the answer to "when are my classes?" — Canvas's calendar API is
//! where a course's timetabled events live (`type=event`), and where every
//! assignment and quiz surfaces as a dated item (`type=assignment`). Both come
//! back through the same endpoint, so one module fetches both and flattens
//! them into one row shape the app and the CLI store identically.
//!
//! Two things about the shape of Canvas's answer drive the code below.
//!
//! **A repeating class is not one event.** Canvas expands a series server-side
//! into an event per occurrence, so there is no recurrence rule to interpret —
//! a semester of lectures is simply a lot of rows. `all_events=true` is what
//! gets the whole semester in one walk instead of guessing a date window that
//! would clip the first or last week.
//!
//! **A class with sections is a parent plus children.** When staff schedule an
//! event per tutorial section, Canvas returns one parent spanning them all with
//! the real, per-section occurrences as `child_events`. Rendering the parent
//! would show every section's tutorial as if the student attended all of them,
//! so children replace their parent — filtered to the sections this user is
//! actually enrolled in when that is knowable (see [`my_section_codes`]).

use std::collections::HashSet;

use crate::canvas::Canvas;

/// One dated thing, already flattened: no parents, no recurrence, no sections.
/// `start_at`/`end_at` are Canvas's ISO8601 UTC strings, stored verbatim and
/// rendered in local time by the frontend.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct CalendarEvent {
    /// Canvas's own context id (`event_123`, `assignment_456`) — stable across
    /// syncs, which is what makes the upsert an upsert.
    pub id: String,
    /// `class` for a scheduled event, `due` for a deadline.
    pub kind: String,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub all_day: bool,
    pub location: Option<String>,
    /// The Canvas page for this item, for the "Open in Canvas" affordance.
    pub url: Option<String>,
    /// Event description, converted from Canvas HTML to markdown.
    pub description: Option<String>,
}

pub const KIND_CLASS: &str = "class";
pub const KIND_DUE: &str = "due";

/// Every dated item for one course: scheduled events first, then deadlines.
///
/// Errors from either half are contained — a course with the calendar tab
/// disabled still yields its assignment due dates, and vice versa.
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

/// The `course_section_<id>` codes this user is enrolled in.
///
/// `include[]=sections` on the courses API returns the *calling user's*
/// sections, not the course's — which is exactly the filter needed to keep
/// other tutorials' times off this student's calendar. An empty set means
/// "unknown", and callers must then keep every child rather than none.
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
    // Ask about the sections as well as the course. Canvas returns a
    // section-scoped occurrence either way — nested under its parent, or
    // top-level when its own context was requested — and which one you get has
    // varied. Requesting both and deduplicating by id is what makes the answer
    // the same either way.
    let contexts: String = std::iter::once(format!("&context_codes[]=course_{course_id}"))
        .chain(sections.iter().map(|c| format!("&context_codes[]={c}")))
        .collect();

    // `include[]` and `includes[]` are both spelled in Canvas's own docs for
    // this endpoint depending on where you look; an unknown query parameter is
    // ignored, so sending both is cheaper than being wrong.
    let raw = canvas.get_all(&format!(
        "/api/v1/calendar_events?type=event&all_events=true&per_page=100\
         {contexts}&include[]=child_events&includes[]=child_events"
    ))?;
    Ok(flatten_events(&raw, sections))
}

/// Parents → occurrences, deduplicated. Pure, so the section rule is testable
/// without a Canvas round trip.
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
            // `hidden` marks a parent Canvas expects to be drawn as its
            // children instead. With no children in hand there is nothing to
            // draw it as, so drawing the span would be worse than skipping.
            if ev["hidden"].as_bool().unwrap_or(false) {
                continue;
            }
            if let Some(row) = event_row(ev) {
                push(row, &mut out);
            }
            continue;
        }

        // Section-scoped occurrences replace the parent that spans them. Keep
        // only this student's sections; if none match — sections unknown, or
        // the children are scoped some other way — keep all of them rather
        // than silently dropping the course's whole timetable.
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

/// Assignments and quizzes as calendar items. Undated ones come back too (the
/// calendar API includes them under `all_events`) and are dropped here — a
/// deadline with no date has nowhere to sit on a calendar.
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
                // The event's own `html_url` is the course's assignment
                // *index*; the nested assignment carries the deep link.
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

/// Timetabled events arrive titled with the whole course name in front of the
/// activity — "IT Project (COMP30022_2026_SM2) (Tutorial 1 (16))". On a
/// calendar that already colours and labels by subject that prefix is pure
/// noise, so it goes, leaving "Tutorial 1 (16)". The trailing group number
/// stays: it is which tutorial you are actually in.
///
/// Staff-created one-off events ("Week 9 Tutorial Wireshark") carry no prefix
/// and are left exactly as written.
fn clean_title(title: &str, context_name: &str) -> String {
    let mut t = title.trim();
    if !context_name.is_empty() {
        if let Some(rest) = t.strip_prefix(context_name) {
            t = rest.trim();
        }
    }
    // What is left is usually the activity wrapped in one pair of parens.
    if let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        // Only unwrap when that pair is the outermost one — "(a) and (b)"
        // must survive intact.
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

/// Venue strings carry a room capacity nobody needs —
/// "PAR-192-L2-L108-Laby Theatre (210)". The building and room codes stay:
/// they are how you find the room.
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

/// Canvas ids arrive as a number for events and as an already-prefixed string
/// (`"assignment_123"`) for assignments. Normalise both to one prefixed key so
/// the two kinds can never collide in the table.
fn context_id(v: &serde_json::Value, prefix: &str) -> Option<String> {
    match &v["id"] {
        serde_json::Value::Number(n) => Some(format!("{prefix}_{n}")),
        serde_json::Value::String(s) if s.contains('_') => Some(s.clone()),
        serde_json::Value::String(s) => Some(format!("{prefix}_{s}")),
        _ => None,
    }
}

/// Descriptions are Canvas HTML bodies like any other, so they go through the
/// same converter — the calendar renders them with the app's markdown stack.
fn markdown_of(html: Option<&str>) -> Option<String> {
    let md = crate::md::to_markdown(html?, &Default::default());
    (!md.trim().is_empty()).then_some(md)
}

// ── Tauri command ─────────────────────────────────────────────────────────────

/// Fetch one course's calendar. The frontend owns the write, exactly as it does
/// for lectures: this returns rows, `upsertCalendarEvents` stores them.
#[tauri::command]
pub async fn calendar_sync_events(
    app: tauri::AppHandle,
    canvas_course_id: i64,
) -> Result<Vec<CalendarEvent>, String> {
    let dir = {
        use tauri::Manager;
        app.path().app_data_dir().map_err(|e| e.to_string())?
    };
    let canvas = Canvas::open(&dir);
    if !canvas.has_session() {
        return Err("Not signed in to Canvas.".to_string());
    }
    fetch(&canvas, canvas_course_id)
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
        // Real UniMelb shapes.
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
        // A staff-written one-off has no prefix to strip.
        assert_eq!(
            clean_title("Week 9 Tutorial Wireshark Monday 12pm", "Information Security"),
            "Week 9 Tutorial Wireshark Monday 12pm"
        );
        // Unwrapping must not eat text that merely starts and ends with parens.
        assert_eq!(clean_title("(a) then (b)", ""), "(a) then (b)");
        // Nothing left after stripping falls back to the original.
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
        // A trailing parenthetical that is not a number stays put.
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
        // Requesting the course and its sections can return a section event
        // both nested under its parent and on its own.
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
