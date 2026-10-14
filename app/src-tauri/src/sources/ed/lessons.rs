//! Ed Lessons: a course's lessons, one lesson's slides, a quiz's questions,
//! a challenge's settings, and a challenge's workspace as a zip.
//!
//! Every call is a read except `workspace_zip`, whose POSTs and live session
//! attach the student's own workspace the way opening the slide does. No request sends
//! `?view=1`: the web app does, and it records a view.

use super::{region, Ed};

use std::collections::HashMap;

/// The host that serves Ed's challenge workspaces in `region`. Its tickets
/// are the credential, so it is reached directly, never through keyd.
pub fn workspace_host(region: &str) -> String {
    format!("sahara.{region}.edstem.org")
}

/// `GET /courses/<c>/lessons`: lessons carry no slides here.
pub struct LessonList {
    pub lessons: Vec<serde_json::Value>,
    /// Ed module id → name, in the order Ed lists the modules.
    pub modules: Vec<(i64, String)>,
}

impl LessonList {
    pub fn module_name(&self, id: Option<i64>) -> Option<&str> {
        let id = id?;
        self.modules
            .iter()
            .find(|(m, _)| *m == id)
            .map(|(_, name)| name.as_str())
    }
}

impl Ed {
    /// A course's lessons, in the order the Ed sidebar shows them
    /// ([`order_lessons`]).
    pub fn lessons(&self, course_id: i64) -> Result<LessonList, String> {
        let reply = self.get(&format!("/courses/{course_id}/lessons"))?;
        let modules: Vec<(i64, String)> = reply["modules"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                Some((
                    m["id"].as_i64()?,
                    m["name"].as_str().unwrap_or("").to_string(),
                ))
            })
            .collect();
        let mut lessons = reply["lessons"].as_array().cloned().unwrap_or_default();
        order_lessons(&mut lessons, &modules);
        Ok(LessonList { lessons, modules })
    }

    /// One lesson with its slides.
    pub fn lesson(&self, lesson_id: i64) -> Result<serde_json::Value, String> {
        let reply = self.get(&format!("/lessons/{lesson_id}"))?;
        match &reply["lesson"] {
            serde_json::Value::Object(_) => Ok(reply["lesson"].clone()),
            _ => Err(format!("no lesson in the answer for lesson {lesson_id}")),
        }
    }

    /// A quiz slide's questions in Ed's order. Ed leaves out each `solution`
    /// until the lesson's solutions are released.
    pub fn quiz_questions(&self, slide_id: i64) -> Result<Vec<serde_json::Value>, String> {
        let reply = self.get(&format!("/lessons/slides/{slide_id}/questions"))?;
        let mut questions = reply["questions"].as_array().cloned().unwrap_or_default();
        questions.sort_by_key(|q| q["index"].as_i64().unwrap_or(i64::MAX));
        Ok(questions)
    }

    /// A challenge's settings (`type`, `settings.run_command`, `features`,
    /// `due_at`) and the student's standing on it. The answer wraps it in
    /// `challenge`; a bare object is taken as it is.
    pub fn challenge(&self, challenge_id: i64) -> Result<serde_json::Value, String> {
        let reply = self.get(&format!("/challenges/{challenge_id}"))?;
        match &reply["challenge"] {
            serde_json::Value::Object(_) => Ok(reply["challenge"].clone()),
            _ if reply.is_object() => Ok(reply),
            _ => Err(format!(
                "no challenge in the answer for challenge {challenge_id}"
            )),
        }
    }

    /// Ed's web region, once a launch has named it.
    pub fn region(&self) -> Option<String> {
        region(&self.data_dir)
    }

    /// The workspace server for this install's Ed region.
    pub fn workspace_host(&self) -> Result<String, String> {
        self.region().map(|r| workspace_host(&r)).ok_or_else(|| {
            "Ed's region is not known yet; it is learned when Oculus next signs in to Ed".into()
        })
    }

    /// The student's current workspace for a challenge, as a zip. Ed only
    /// hands out a download for a live workspace, so: connect, hand the
    /// connect ticket to `open_live` (the session stays open while its guard
    /// lives), ask for a download ticket for the whole tree, then hand the
    /// ticket URL to `download`, which fetches it once with no credentials.
    /// Both tickets are credentials and must never reach a log or an error.
    pub fn workspace_zip<G>(
        &self,
        challenge_id: i64,
        open_live: impl FnOnce(&str, &str) -> Result<G, String>,
        download: impl FnOnce(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Vec<u8>, String> {
        let host = self.workspace_host()?;
        let connected = self.connect(challenge_id)?;
        let _live = open_live(&host, connected.ticket.expose())?;
        let ticket = self.download_ticket(&connected.workspace_id, ".")?;
        download(&format!("https://{host}/download/{}", ticket.expose()))
    }
}

/// Module order (as Ed lists the modules; a lesson in none comes last), then
/// the lesson's `index` with unset ones last, then id.
pub fn order_lessons(lessons: &mut [serde_json::Value], modules: &[(i64, String)]) {
    let rank: HashMap<i64, usize> = modules
        .iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i))
        .collect();
    lessons.sort_by_key(|l| {
        (
            l["module_id"]
                .as_i64()
                .and_then(|m| rank.get(&m).copied())
                .unwrap_or(usize::MAX),
            l["index"].as_i64().unwrap_or(i64::MAX),
            l["id"].as_i64().unwrap_or(i64::MAX),
        )
    });
}
