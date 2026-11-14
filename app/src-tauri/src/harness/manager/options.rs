use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::studio::StudioBrief;

/// What a send asks for beyond the text. The model is persisted on the
/// thread; a later send with a different one changes it from then on.
/// Serialized whole onto a run that waits its turn (`store/queue.rs`), so a
/// field added here rides the queue and a restart with no further code; the
/// `skip` fields are resolved again from the thread's row at each start.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct SendOptions {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    /// Read only when the send creates a Studio thread; afterwards the row is
    /// the authority, since the brief is bound at session start. A plain Chat
    /// thread names its subject in `chat_scope` instead.
    pub subject_id: Option<i64>,
    /// The scope the student chose for a new plain Chat thread. Creating one
    /// without it is refused ([`SendOptions::scope_new_thread`]).
    pub chat_scope: Option<ChatScope>,
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
    /// The Studio video this send creates a thread for. Read only on
    /// creation, where it links the video to the thread (`studio_videos`).
    pub studio: Option<StudioSend>,
    /// That video, resolved from the thread's link.
    #[serde(skip)]
    pub studio_brief: Option<StudioBrief>,
    /// Antigravity only: the student's approved rules, for the spawn to write
    /// (`antigravity_rules::install`). `None` keeps the last ones written.
    #[serde(skip)]
    pub antigravity_rules: Option<Vec<String>>,
    /// Set by the app, never the webview: this send is the follow-up of
    /// that card (`app/continuation.rs`), not something the student typed.
    #[serde(skip)]
    pub continuation: Option<String>,
}

/// A new Chat thread's scope as the composer sends it: `"general"` or a
/// subject's id. Unchosen is the field's absence, so it is never General by
/// default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatScope {
    General,
    Subject(i64),
}

impl Serialize for ChatScope {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ChatScope::General => s.serialize_str("general"),
            ChatScope::Subject(id) => s.serialize_i64(*id),
        }
    }
}

impl<'de> Deserialize<'de> for ChatScope {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Id(i64),
            Word(String),
        }
        match Raw::deserialize(d)? {
            Raw::Id(id) if id > 0 => Ok(ChatScope::Subject(id)),
            Raw::Word(w) if w == "general" => Ok(ChatScope::General),
            _ => Err(serde::de::Error::custom(
                "a chat scope is \"general\" or a subject id",
            )),
        }
    }
}

impl SendOptions {
    /// Settle the subject a thread-creating send files under, before any row
    /// is written. A lecture's thread takes its lecture's subject and a Studio
    /// video's its own, so only a plain Chat thread must name a scope: without
    /// one a missed entry point would file into General silently. A send to
    /// an existing thread is never checked.
    pub(in crate::harness) fn scope_new_thread(
        &mut self,
        thread_id: Option<i64>,
    ) -> Result<(), String> {
        if thread_id.is_some() || self.lecture_id.is_some() || self.studio.is_some() {
            return Ok(());
        }
        self.subject_id = match self.chat_scope {
            Some(ChatScope::General) => None,
            Some(ChatScope::Subject(id)) => Some(id),
            None => return Err("choose a subject or General before starting a chat".into()),
        };
        Ok(())
    }
}

/// The video a thread is created for, as `StudioSend` in
/// `app/src/lib/studio/videos.ts` sends it. The brief reads the format from
/// video.json's size, which is what renders use, so the TS `format` is not
/// read here.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StudioSend {
    /// A folder under `<data>/studio/videos/`, made before the send.
    pub slug: String,
    /// The narration length asked for, in seconds.
    pub length_secs: Option<u32>,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(json: &str) -> SendOptions {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn a_plain_chat_without_a_scope_is_refused() {
        let mut o = opts(r#"{"model":"opus"}"#);
        assert!(o.scope_new_thread(None).is_err());
        // A stray subject id is not a choice either.
        let mut o = opts(r#"{"subjectId":4}"#);
        assert!(o.scope_new_thread(None).is_err());
    }

    #[test]
    fn a_plain_chat_files_under_its_chosen_scope() {
        let mut o = opts(r#"{"chatScope":"general","subjectId":4}"#);
        o.scope_new_thread(None).unwrap();
        assert_eq!(o.subject_id, None);
        let mut o = opts(r#"{"chatScope":12}"#);
        o.scope_new_thread(None).unwrap();
        assert_eq!(o.subject_id, Some(12));
    }

    #[test]
    fn lecture_and_studio_creations_carry_their_own_subject() {
        let mut lecture = opts(r#"{"lectureId":"abc"}"#);
        lecture.scope_new_thread(None).unwrap();
        assert_eq!(lecture.subject_id, None);
        let mut studio = opts(r#"{"subjectId":4,"studio":{"slug":"limits"}}"#);
        studio.scope_new_thread(None).unwrap();
        assert_eq!(studio.subject_id, Some(4));
    }

    #[test]
    fn an_existing_thread_sends_without_a_scope() {
        // A General thread's row has no subject and its sends name no scope.
        let mut o = opts(r#"{"model":"opus"}"#);
        o.scope_new_thread(Some(7)).unwrap();
        assert_eq!(o.subject_id, None);
    }

    /// What waits in the queue is this struct as JSON: everything the
    /// webview sent comes back, and nothing the app resolves is stored.
    #[test]
    fn the_sendable_fields_survive_a_round_trip_and_the_resolved_ones_are_left_out() {
        let sent = r#"{"model":"opus","reasoningEffort":"high","chatScope":"general","context":"the moment","at":42,"studio":{"slug":"limits","lengthSecs":90}}"#;
        let mut o = opts(sent);
        o.scope = Some("COMP1".into());
        o.continuation = Some("12.0".into());
        o.antigravity_rules = Some(vec!["command(ls)".into()]);
        let stored = serde_json::to_value(&o).unwrap();
        for resolved in [
            "scope",
            "lecture",
            "studioBrief",
            "antigravityRules",
            "continuation",
        ] {
            assert!(stored.get(resolved).is_none(), "{resolved} was stored");
        }
        let back: SendOptions = serde_json::from_value(stored.clone()).unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), stored);
        assert_eq!(back.model.as_deref(), Some("opus"));
        assert_eq!(back.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(back.chat_scope, Some(ChatScope::General));
        assert_eq!(back.context.as_deref(), Some("the moment"));
        assert_eq!(back.at, Some(42));
        assert_eq!(back.studio.unwrap().length_secs, Some(90));
        assert_eq!(serde_json::to_value(ChatScope::Subject(3)).unwrap(), 3);
    }

    #[test]
    fn a_scope_is_general_or_a_positive_id() {
        assert_eq!(
            opts(r#"{"chatScope":"general"}"#).chat_scope,
            Some(ChatScope::General)
        );
        assert_eq!(
            opts(r#"{"chatScope":3}"#).chat_scope,
            Some(ChatScope::Subject(3))
        );
        assert_eq!(opts("{}").chat_scope, None);
        for bad in [r#""General""#, r#""3""#, "0", "-1", "1.5", "null"] {
            let json = format!(r#"{{"chatScope":{bad}}}"#);
            let parsed = serde_json::from_str::<SendOptions>(&json);
            if bad == "null" {
                assert_eq!(parsed.unwrap().chat_scope, None);
            } else {
                assert!(parsed.is_err(), "{bad} parsed");
            }
        }
    }
}
