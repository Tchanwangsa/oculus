//! Mapping a Canvas course code to its Ed course.

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
                            created_at: c["created_at"].as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        *guard = Some(list.clone());
        Ok(list)
    }

    /// The Ed course matching a Canvas course code. Ed codes are staff-typed
    /// free text, so match on the leading subject token; year and semester
    /// from the Canvas code break ties, then recency.
    pub fn course_for(&self, canvas_code: &str) -> Result<Option<i64>, String> {
        let want = code_token(canvas_code);
        if want.is_empty() {
            return Ok(None);
        }
        let canvas_upper = canvas_code.to_uppercase();
        let courses = self.courses()?;

        let mut best: Option<(i32, &EdCourse)> = None;
        for c in courses.iter().filter(|c| code_token(&c.code) == want) {
            let mut score = 0;
            if !c.year.is_empty() && canvas_upper.contains(&c.year) {
                score += 2;
            }
            if let Some(d) = c.session.chars().find(char::is_ascii_digit) {
                if canvas_upper.contains(&format!("SM{d}")) {
                    score += 1;
                }
            }
            let better = match &best {
                Some((s, b)) => score > *s || (score == *s && c.created_at > b.created_at),
                None => true,
            };
            if better {
                best = Some((score, c));
            }
        }
        Ok(best.map(|(_, c)| c.id))
    }
}

/// The leading subject code: `"comp10002 2024s2"` → `"COMP10002"`.
pub(super) fn code_token(code: &str) -> String {
    code.trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}
