//! Claude 2.1.325 keys answers by question text. Multiple selections are
//! one comma-separated string; its question tool always accepts typed text.
use serde::Deserialize;
use serde_json::Value;

use crate::harness::requests::{NewRequest, Question, QuestionOption, QuestionPayload};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeQuestion {
    question: String,
    header: String,
    options: Vec<QuestionOption>,
    multi_select: bool,
}

pub(super) fn request(input: &Value, native_ref: String) -> Result<NewRequest, String> {
    let questions: Vec<NativeQuestion> =
        serde_json::from_value(input["questions"].clone()).map_err(|e| e.to_string())?;
    QuestionPayload {
        questions: questions
            .into_iter()
            .map(|q| Question {
                id: q.question.clone(),
                text: q.question,
                header: q.header,
                options: q.options,
                multi: q.multi_select,
                allows_typed_answer: true,
                secret: false,
            })
            .collect(),
        is_blocking: None,
    }
    .request(native_ref)
}
