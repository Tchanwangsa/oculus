use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Action, Decision, NewRequest, RequestOption};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// Codex's native id, Claude's question text, or agy's schema property.
    pub id: String,
    pub header: String,
    pub text: String,
    pub options: Vec<QuestionOption>,
    pub multi: bool,
    pub allows_typed_answer: bool,
    pub secret: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QuestionPayload {
    pub questions: Vec<Question>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_blocking: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionAnswer {
    pub question_id: String,
    pub answers: Vec<String>,
    #[serde(default)]
    pub dismissed: bool,
}

impl QuestionPayload {
    pub fn read(value: &Value) -> Result<Self, String> {
        let payload: Self = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        let mut ids = HashSet::new();
        if payload.questions.is_empty()
            || payload.questions.iter().any(|q| {
                q.secret
                    || q.id.trim().is_empty()
                    || q.text.trim().is_empty()
                    || !ids.insert(&q.id)
                    || q.options.iter().any(|o| o.label.trim().is_empty())
                    || q.options
                        .iter()
                        .map(|o| &o.label)
                        .collect::<HashSet<_>>()
                        .len()
                        != q.options.len()
                    || (q.options.is_empty() && !q.allows_typed_answer)
            })
        {
            return Err(
                "Oculus cannot show this question payload (empty, duplicate or secret question)."
                    .into(),
            );
        }
        Ok(payload)
    }

    pub fn request(self, native_ref: String) -> Result<NewRequest, String> {
        let payload = serde_json::to_value(self).map_err(|e| e.to_string())?;
        Self::read(&payload)?;
        Ok(NewRequest {
            action: Action::Question,
            payload,
            options: vec![
                RequestOption {
                    id: "answer".into(),
                    label: "Answer".into(),
                    decision: Decision::Accept,
                    scope: None,
                },
                RequestOption {
                    id: "dismiss".into(),
                    label: "Dismiss question without answering".into(),
                    decision: Decision::Decline,
                    scope: None,
                },
            ],
            native_ref,
            paths: Vec::new(),
        })
    }

    pub fn validate(&self, option: &str, answers: &[QuestionAnswer]) -> Result<(), String> {
        let invalid = || {
            "Answer every question with a listed option or an allowed typed answer, or dismiss it."
                .to_string()
        };
        if option == "dismiss" {
            return if answers.is_empty() {
                Ok(())
            } else {
                Err(invalid())
            };
        }
        if option != "answer" || answers.len() != self.questions.len() {
            return Err(invalid());
        }
        let mut ids = HashSet::new();
        for answer in answers {
            let q = self
                .questions
                .iter()
                .find(|q| q.id == answer.question_id)
                .ok_or_else(invalid)?;
            if !ids.insert(&answer.question_id) {
                return Err(invalid());
            }
            if answer.dismissed {
                if !answer.answers.is_empty() {
                    return Err(invalid());
                }
                continue;
            }
            if answer.answers.is_empty()
                || (!q.multi && answer.answers.len() != 1)
                || answer.answers.iter().any(|a| {
                    a.trim().is_empty()
                        || (!q.allows_typed_answer && !q.options.iter().any(|o| o.label == *a))
                })
                || answer.answers.iter().collect::<HashSet<_>>().len() != answer.answers.len()
            {
                return Err(invalid());
            }
        }
        Ok(())
    }

    /// A fallback send is ordinary student text, with each question next to
    /// its answer. The command saves this run and claims the request together.
    pub fn message(&self, answers: &[QuestionAnswer]) -> String {
        self.questions
            .iter()
            .map(|q| {
                let a = answers
                    .iter()
                    .find(|a| a.question_id == q.id)
                    .expect("validated answer");
                let text = if a.dismissed {
                    "(dismissed without answering)".into()
                } else {
                    a.answers.join(", ")
                };
                format!("{}\n{}", q.text, text)
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> QuestionPayload {
        QuestionPayload {
            questions: vec![Question {
                id: "native".into(),
                header: "Color".into(),
                text: "Choose color".into(),
                options: vec![QuestionOption {
                    label: "Red".into(),
                    description: "Warm".into(),
                }],
                multi: false,
                allows_typed_answer: false,
                secret: false,
            }],
            is_blocking: Some(false),
        }
    }

    #[test]
    fn choice_only_typed_multiple_empty_duplicate_and_unknown_answers_are_checked() {
        let p = payload();
        let answer = |id: &str, values: &[&str], dismissed| QuestionAnswer {
            question_id: id.into(),
            answers: values.iter().map(|v| v.to_string()).collect(),
            dismissed,
        };
        assert!(p
            .validate("answer", &[answer("native", &["Red"], false)])
            .is_ok());
        for a in [
            answer("foreign", &["Red"], false),
            answer("native", &["typed"], false),
            answer("native", &[], false),
            answer("native", &[" "], false),
            answer("native", &["Red", "Red"], false),
            answer("native", &["Red"], true),
        ] {
            assert!(p.validate("answer", &[a]).is_err());
        }
        assert!(p.validate("answer", &[answer("native", &[], true)]).is_ok());
        assert!(p.validate("dismiss", &[]).is_ok());
        assert!(p
            .validate("dismiss", &[answer("native", &["Red"], false)])
            .is_err());
        let mut p = p;
        p.questions[0].allows_typed_answer = true;
        assert!(p
            .validate("answer", &[answer("native", &["typed"], false)])
            .is_ok());
        p.questions[0].multi = true;
        assert!(p
            .validate("answer", &[answer("native", &["Red", "typed"], false)])
            .is_ok());
    }

    #[test]
    fn secret_and_ambiguous_questions_never_become_requests() {
        let mut p = payload();
        p.questions[0].secret = true;
        assert!(p.clone().request("0".into()).is_err());
        p.questions[0].secret = false;
        p.questions.push(p.questions[0].clone());
        assert!(p.request("0".into()).is_err());
    }
}
