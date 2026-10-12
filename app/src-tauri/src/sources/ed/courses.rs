//! Mapping a Canvas course code to its Ed course.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{Ed, EdCourse};

impl Ed {
    fn courses(&self) -> Result<Vec<EdCourse>, String> {
        let mut guard = self.courses.lock().unwrap();
        if let Some(cached) = guard.as_ref() {
            return Ok(cached.clone());
        }
        self.renew();
        let user = self.get("/user")?;
        let list: Vec<EdCourse> = user["courses"]
            .as_array()
            .map(|cs| {
                cs.iter()
                    .filter_map(|c| {
                        let c = &c["course"];
                        Some(EdCourse {
                            id: c["id"].as_i64()?,
                            code: c["code"].as_str().unwrap_or("").to_string(),
                            year: c["year"].as_str().unwrap_or("").to_string(),
                            session: c["session"].as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        *guard = Some(list.clone());
        Ok(list)
    }

    /// The Ed course matching a Canvas course code. Ed codes are staff-typed
    /// free text, so match on the subject token; year and term from the Canvas
    /// code break ties. A tie they cannot break is `None`, and the caller's
    /// LTI launch names the course exactly.
    pub fn course_for(&self, canvas_code: &str) -> Result<Option<i64>, String> {
        let want = code_token(canvas_code);
        if want.is_empty() {
            return Ok(None);
        }
        let canvas_upper = canvas_code.to_uppercase();
        let courses = self.courses()?;

        let scored: Vec<(i32, &EdCourse)> = courses
            .iter()
            .filter(|c| code_token(&c.code) == want)
            .map(|c| (term_score(&canvas_upper, c), c))
            .collect();
        Ok(pick_course(&scored))
    }

    /// The Ed course an earlier launch of this Canvas course landed on, while
    /// the session is still enrolled in it. A launch names the course
    /// exactly, so it settles a tie `course_for` cannot without a new launch
    /// (and a new Ed session) every sync.
    pub(super) fn launched_course(&self, canvas_course_id: i64) -> Option<i64> {
        let id = *read_launched(&self.data_dir).get(&canvas_course_id.to_string())?;
        self.courses()
            .is_ok_and(|cs| cs.iter().any(|c| c.id == id))
            .then_some(id)
    }

    /// Keep what a launch named for [`Ed::launched_course`]. Best-effort:
    /// losing it costs one more launch.
    pub(super) fn remember_launch(&self, canvas_course_id: i64, ed_course_id: i64) {
        let mut launched = read_launched(&self.data_dir);
        if launched.insert(canvas_course_id.to_string(), ed_course_id) == Some(ed_course_id) {
            return;
        }
        let target = launched_path(&self.data_dir);
        let written = serde_json::to_vec_pretty(&launched)
            .map_err(|e| e.to_string())
            .and_then(|body| {
                crate::runtime::atomic_write::write(
                    &target,
                    &target.with_extension("json.tmp"),
                    &body,
                )
            });
        if let Err(e) = written {
            elog!("[oculus] could not save {}: {e}", target.display());
        }
    }
}

/// Canvas course id → the Ed course its launch landed on.
fn launched_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ed-courses.json")
}

fn read_launched(data_dir: &Path) -> BTreeMap<String, i64> {
    std::fs::read_to_string(launched_path(data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// The subject code: the words up to and including the first with a digit.
/// `"comp10002 2024s2"` → `"COMP10002"`, `"CS 101-001"` → `"CS101"`.
pub(super) fn code_token(code: &str) -> String {
    let mut out = String::new();
    for word in code
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        out.push_str(word);
        if word.chars().any(|c| c.is_ascii_digit()) {
            break;
        }
    }
    out.to_uppercase()
}

const SEASONS: &[&str] = &["SUMMER", "WINTER", "SPRING", "AUTUMN", "FALL"];

/// How well an Ed course's year and session fit an upper-cased Canvas code:
/// the year counts double, then a UniMelb `SM<n>` or a shared season name.
pub(super) fn term_score(canvas_upper: &str, c: &EdCourse) -> i32 {
    let mut score = 0;
    if !c.year.is_empty() && canvas_upper.contains(&c.year) {
        score += 2;
    }
    let session = c.session.to_uppercase();
    let semester = session
        .chars()
        .find(char::is_ascii_digit)
        .is_some_and(|d| canvas_upper.contains(&format!("SM{d}")));
    let season = SEASONS
        .iter()
        .any(|s| session.contains(s) && canvas_upper.contains(s));
    if semester || season {
        score += 1;
    }
    score
}

/// The best-scoring course, or `None` when two share the best score.
pub(super) fn pick_course(scored: &[(i32, &EdCourse)]) -> Option<i64> {
    let best = scored.iter().map(|(s, _)| *s).max()?;
    let mut top = scored.iter().filter(|(s, _)| *s == best);
    let (_, first) = top.next()?;
    top.next().is_none().then_some(first.id)
}
