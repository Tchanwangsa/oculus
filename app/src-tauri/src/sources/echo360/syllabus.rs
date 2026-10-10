//! A course's finished recordings, from the Echo360 syllabus API.

use super::media::download_url;
use super::{Lecture, Session, TRIM_SECS};

/// Every past lesson with a finished recording — one still processing has a
/// media id but its download 404s.
pub fn syllabus(session: &Session) -> Result<Vec<Lecture>, String> {
    let url = format!(
        "https://echo360.net.au/section/{}/syllabus",
        session.section_id
    );
    let raw = ureq::get(&url)
        .set("Cookie", &session.cookie_header())
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let items = body["data"].as_array().ok_or("syllabus: no data array")?;

    let mut out = Vec::new();
    // Lectures the syllabus had no file lists for; nonzero is worth a warning.
    let mut probed = 0usize;
    for item in items {
        let lesson = &item["lesson"];
        let inner = &lesson["lesson"];

        if !lesson["hasVideo"].as_bool().unwrap_or(false)
            || !lesson["medias"][0]["isAvailable"]
                .as_bool()
                .unwrap_or(false)
            || lesson["medias"][0]["isProcessing"]
                .as_bool()
                .unwrap_or(true)
            || !lesson["isPast"].as_bool().unwrap_or(false)
        {
            continue;
        }
        let media_id = lesson["medias"][0]["id"].as_str().unwrap_or("").to_string();
        if media_id.is_empty() {
            continue;
        }
        let raw_dur = duration_between(
            lesson["captureStartedAt"].as_str().unwrap_or(""),
            lesson["captureEndedAt"].as_str().unwrap_or(""),
        );
        let lesson_id = inner["id"].as_str().unwrap_or("").to_string();
        let has_second_source = match second_source_hint(lesson) {
            Some(known) => known,
            None => {
                probed += 1;
                download_url(session, &media_id, &lesson_id, 2).is_ok()
            }
        };

        out.push(Lecture {
            id: media_id,
            lesson_id,
            title: inner["name"].as_str().unwrap_or("").to_string(),
            date: inner["timing"]["start"].as_str().unwrap_or("").to_string(),
            // Post-trim, to match the file on disk.
            duration_seconds: (raw_dur - TRIM_SECS as i64).max(0),
            has_second_source,
        });
    }
    let dual = out.iter().filter(|l| l.has_second_source).count();
    eprintln!(
        "[oculus] echo360 syllabus: {} lectures ({dual} with a second source)",
        out.len()
    );
    if probed > 0 {
        eprintln!(
            "[oculus] warn: syllabus carried no file lists for {probed} lecture(s) — \
             fell back to probing the download endpoint"
        );
    }
    Ok(out)
}

/// Whether a lesson has a camera stream, from `primaryFiles`/`secondaryFiles`
/// (searched for, since their nesting is not stable). `None` when there are no
/// file lists at all — the caller then probes the download endpoint.
pub(super) fn second_source_hint(lesson: &serde_json::Value) -> Option<bool> {
    let non_empty = |v: &serde_json::Value| v.as_array().is_some_and(|a| !a.is_empty());
    if find_key(lesson, "secondaryFiles").is_some_and(non_empty) {
        return Some(true);
    }
    find_key(lesson, "primaryFiles").map(|_| false)
}

/// First value under `key` anywhere in the tree.
fn find_key<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    match v {
        serde_json::Value::Object(map) => {
            if let Some(hit) = map.get(key) {
                return Some(hit);
            }
            map.values().find_map(|child| find_key(child, key))
        }
        serde_json::Value::Array(items) => items.iter().find_map(|child| find_key(child, key)),
        _ => None,
    }
}

pub(super) fn duration_between(start: &str, end: &str) -> i64 {
    fn secs(s: &str) -> Option<i64> {
        let t = s.split('T').nth(1)?;
        // Keep only HH:MM:SS, dropping any fraction and zone.
        let t: String = t
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == ':')
            .collect();
        let p: Vec<i64> = t.split(':').filter_map(|x| x.parse().ok()).collect();
        (p.len() >= 3).then(|| p[0] * 3600 + p[1] * 60 + p[2])
    }
    let s = secs(start).unwrap_or(0);
    let e = secs(end).unwrap_or(0);
    if e >= s {
        e - s
    } else {
        e + 86400 - s
    }
}
