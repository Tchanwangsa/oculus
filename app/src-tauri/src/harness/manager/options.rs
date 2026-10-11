use serde::Deserialize;

/// What a send asks for beyond the text. The model is persisted on the
/// thread; a later send with a different one changes it from then on.
#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendOptions {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Read only when the send creates the thread; afterwards the row is the
    /// authority, since the brief is bound at session start.
    pub subject_id: Option<i64>,
    /// That subject's folder name, resolved from the thread row.
    #[serde(skip)]
    pub scope: Option<String>,
    /// The recording a dock conversation is about. Read only on creation,
    /// and it also decides the subject (`store::create_thread`).
    pub lecture_id: Option<String>,
    /// That lecture, resolved from the thread row.
    #[serde(skip)]
    pub lecture: Option<LectureBrief>,
    /// The player's moment (timestamp, transcript tail, frames). Appended to
    /// the prompt after the student's text; never part of the row.
    pub context: Option<String>,
    /// The playhead's second at send; see [`crate::harness::HarnessEvent::UserMessage`].
    pub at: Option<i64>,
    /// Antigravity only: the student's approved rules, for the spawn to write
    /// (`antigravity_rules::install`). `None` keeps the last ones written.
    #[serde(skip)]
    pub antigravity_rules: Option<Vec<String>>,
}

/// What a lecture thread's brief says about the recording, assembled from
/// the thread's row and `store::chapters`.
#[derive(Clone)]
pub struct LectureBrief {
    pub id: String,
    pub title: String,
    /// `YYYY-MM-DD`: the title is the timetable's, so the date is what says
    /// which week (and slide deck) this is.
    pub date: String,
    pub has_transcript: bool,
    pub chapters: Vec<crate::lectures::chapters::Chapter>,
}

/// Every reasoning level any CLI accepts, mirrored by `REASONING_LABELS` in
/// `app/src/lib/harness/models.ts`. Checked up front so an unknown string never
/// reaches an argv or a Codex config.
const REASONING_EFFORTS: [&str; 6] = ["minimal", "low", "medium", "high", "xhigh", "max"];

pub(in crate::harness) fn validate_effort(value: Option<String>) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(v) if REASONING_EFFORTS.contains(&v.as_str()) => Ok(Some(v)),
        Some(v) => Err(format!("unknown reasoning effort: {v}")),
    }
}
