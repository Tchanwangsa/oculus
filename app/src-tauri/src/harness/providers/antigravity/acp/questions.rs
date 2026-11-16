//! agy 1.5.9's elicitation forms. Native constants stay on the route;
//! the shared question payload contains the labels the student answers.
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::harness::requests::{
    Decision, LiveRoute, NewRequest, Question, QuestionAnswer, QuestionOption, QuestionPayload,
    Reply, RequestOption, Route,
};
use crate::harness::HarnessEvent;

use super::route::{AcpRoute, Options};
use super::session::AcpSession;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Params {
    session_id: String,
    tool_call_id: String,
    mode: String,
    message: String,
    requested_schema: Schema,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Schema {
    #[serde(rename = "type")]
    kind: String,
    properties: Map<String, Value>,
    #[serde(default)]
    required: Vec<String>,
    additional_properties: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Field {
    #[serde(rename = "type")]
    kind: String,
    title: String,
    description: Option<String>,
    one_of: Option<Vec<Choice>>,
    items: Option<Items>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Items {
    #[serde(rename = "type")]
    kind: Option<String>,
    one_of: Vec<Choice>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Choice {
    #[serde(rename = "const")]
    value: String,
    title: String,
    description: Option<String>,
}

struct NativeQuestion {
    id: String,
    multi: bool,
    other: Option<String>,
    choices: Vec<Choice>,
}

pub(super) struct Form {
    pub(super) request: NewRequest,
    native: Vec<NativeQuestion>,
    tool: String,
}

impl Form {
    pub(super) fn parse(params: &Value, session: &str, wire: String) -> Result<Self, String> {
        let p: Params = serde_json::from_value(params.clone()).map_err(|e| e.to_string())?;
        if p.session_id != session
            || p.tool_call_id.trim().is_empty()
            || p.mode != "form"
            || p.message.trim().is_empty()
            || p.requested_schema.kind != "object"
        {
            return Err("expected a form for this session and tool".into());
        }
        let schema = p.requested_schema;
        if schema
            .required
            .iter()
            .any(|id| !schema.properties.contains_key(id))
            || schema.additional_properties == Some(true)
        {
            return Err("unknown required property or open form".into());
        }
        let mut questions = Vec::new();
        let mut native = Vec::new();
        for (id, value) in &schema.properties {
            if id.ends_with("_other") {
                let base = id.strip_suffix("_other").unwrap();
                if !schema.properties.contains_key(base) || base.ends_with("_other") {
                    return Err(format!("orphan free-text property {id}"));
                }
                continue;
            }
            let field: Field =
                serde_json::from_value(value.clone()).map_err(|e| format!("{id}: {e}"))?;
            if field.title.trim().is_empty() {
                return Err(format!("{id}: missing title"));
            }
            let multi = field.kind == "array";
            let choices = match (field.kind.as_str(), field.one_of, field.items) {
                ("string", Some(choices), None) => choices,
                ("array", None, Some(items))
                    if items.kind.as_deref().is_none_or(|k| k == "string") =>
                {
                    items.one_of
                }
                _ => return Err(format!("{id}: expected string oneOf or array items.oneOf")),
            };
            let mut constants = HashSet::new();
            if choices.is_empty()
                || choices
                    .iter()
                    .any(|o| o.value.is_empty() || !constants.insert(&o.value))
            {
                return Err(format!("{id}: empty or duplicate constants"));
            }
            let other = format!("{id}_other");
            let other = if let Some(value) = schema.properties.get(&other) {
                let f: Field =
                    serde_json::from_value(value.clone()).map_err(|e| format!("{other}: {e}"))?;
                if f.kind != "string" || f.one_of.is_some() || f.items.is_some() {
                    return Err(format!("{other}: expected free text"));
                }
                Some(other)
            } else {
                None
            };
            questions.push(Question {
                id: id.clone(),
                header: field.title.clone(),
                text: field
                    .description
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(field.title),
                options: choices
                    .iter()
                    .map(|o| QuestionOption {
                        label: o.title.clone(),
                        description: o.description.clone().unwrap_or_default(),
                    })
                    .collect(),
                multi,
                allows_typed_answer: other.is_some(),
                secret: false,
            });
            native.push(NativeQuestion {
                id: id.clone(),
                multi,
                other,
                choices,
            });
        }
        let mut request = QuestionPayload {
            questions,
            is_blocking: None,
        }
        .request(wire)?;
        request.payload["tool"] = json!(p.tool_call_id);
        Ok(Self {
            request,
            native,
            tool: p.tool_call_id,
        })
    }

    pub(super) fn encode(&self, answers: &[QuestionAnswer]) -> Result<Value, String> {
        QuestionPayload::read(&self.request.payload)?.validate("answer", answers)?;
        let mut content = Map::new();
        for q in &self.native {
            let a = answers
                .iter()
                .find(|a| a.question_id == q.id)
                .expect("validated answer");
            if a.dismissed {
                continue;
            }
            let selected: Vec<_> = a
                .answers
                .iter()
                .filter_map(|label| {
                    q.choices
                        .iter()
                        .find(|c| c.title == *label)
                        .map(|c| c.value.clone())
                })
                .collect();
            if q.multi {
                content.insert(q.id.clone(), json!(selected));
            } else if let Some(value) = selected.first() {
                content.insert(q.id.clone(), json!(value));
            }
            if let Some(other) = &q.other {
                // The recorded reply carries both the constant and the text.
                content.insert(other.clone(), json!(a.answers.join(", ")));
            }
        }
        Ok(json!({"action":"accept", "content":content}))
    }
}

struct QuestionRoute {
    route: AcpRoute,
    form: Form,
}

impl Route for QuestionRoute {
    fn reply(&self, _: &RequestOption) -> Reply {
        self.route.write(decline())
    }
    fn reply_answers(&self, option: &RequestOption, answers: &[QuestionAnswer]) -> Reply {
        if option.decision == Decision::Decline {
            return self.reply(option);
        }
        match self.form.encode(answers) {
            Ok(result) => self.route.write(result),
            Err(e) => Reply::Failed(e),
        }
    }
    fn withdraw(&self) {
        let _ = self.route.write(decline());
    }
}

pub(super) fn decline() -> Value {
    json!({"action":"decline"})
}

impl AcpSession {
    pub(super) fn question(&self, id: &Value, params: &Value) {
        if self.one_off
            || !self.prompting.load(Ordering::SeqCst)
            || self.interrupting.load(Ordering::SeqCst)
            || self.closing.load(Ordering::SeqCst)
        {
            if let Err(e) = self.rpc.answer(id, decline()) {
                elog!("[oculus] agy: declining a question: {e}");
            }
            return;
        }
        let wire = id.to_string();
        let form = match Form::parse(params, &self.session_id.lock().unwrap(), wire.clone()) {
            Ok(form) => form,
            Err(e) => {
                let message =
                    format!("Unsupported Antigravity question shape (expected agy 1.5.9): {e}");
                if let Err(e) = self.rpc.refuse(id, -32602, &message) {
                    elog!("[oculus] agy: refusing a question: {e}");
                }
                (self.sink)(HarnessEvent::error(message));
                return;
            }
        };
        let generation = self.register_callback(id, decline(), Some(form.tool.clone()));
        let request = form.request.clone();
        let route = QuestionRoute {
            form,
            route: AcpRoute {
                session: self.me.clone(),
                id: id.clone(),
                wire,
                generation,
                options: Options::default(),
            },
        };
        (self.sink)(HarnessEvent::RequestAsked {
            request,
            route: Some(LiveRoute(Arc::new(route))),
        });
    }

    /// Finishing its tool without an answer withdraws that tool's question.
    pub(super) fn retire_question(&self, tool: &str) {
        let mut callbacks = self.callbacks.lock().unwrap();
        let retired: Vec<_> = callbacks
            .1
            .iter()
            .filter(|(_, c)| c.tool.as_deref() == Some(tool))
            .map(|(wire, _)| wire.clone())
            .collect();
        for wire in &retired {
            callbacks.1.remove(wire);
        }
        drop(callbacks);
        for native_ref in retired {
            (self.sink)(HarnessEvent::RequestRetired { native_ref });
        }
    }
}
