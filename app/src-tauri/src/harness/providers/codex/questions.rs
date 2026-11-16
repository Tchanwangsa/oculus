//! `requestUserInput` on Codex 0.169.0. Unknown experimental shapes are
//! refused with a protocol error and a visible chat error, never guessed.
use std::sync::{Arc, Weak};

use serde::Deserialize;
use serde_json::{json, Value};

use crate::harness::requests::{
    LiveRoute, Question, QuestionAnswer, QuestionOption, QuestionPayload, Reply, RequestOption,
    Route,
};
use crate::harness::HarnessEvent;

use super::{server::CodexServer, ThreadRoute};

pub(super) const METHOD: &str = "item/tool/requestUserInput";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Params {
    thread_id: String,
    turn_id: String,
    item_id: String,
    questions: Vec<NativeQuestion>,
    is_blocking: bool,
    auto_resolution_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NativeQuestion {
    id: String,
    header: String,
    question: String,
    is_other: bool,
    is_secret: bool,
    options: Vec<NativeOption>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeOption {
    label: String,
    description: String,
}

struct QuestionRoute {
    server: Weak<CodexServer>,
    thread: Arc<ThreadRoute>,
    wire_id: Value,
}

impl QuestionRoute {
    fn write(&self, answers: &[QuestionAnswer]) -> Reply {
        let Some(server) = self.server.upgrade().filter(|s| s.is_alive()) else {
            return Reply::Failed("the codex app-server is not running".into());
        };
        self.thread
            .state
            .lock()
            .unwrap()
            .callbacks
            .remove(&self.wire_id.to_string());
        let answers: serde_json::Map<_, _> = answers
            .iter()
            .map(|a| (a.question_id.clone(), json!({"answers": a.answers})))
            .collect();
        match server
            .proc
            .write_line(&json!({"jsonrpc":"2.0", "id":self.wire_id, "result":{"answers":answers}}))
        {
            Ok(()) => Reply::Written,
            Err(e) => Reply::Uncertain(e),
        }
    }
}

impl Route for QuestionRoute {
    fn reply(&self, _: &RequestOption) -> Reply {
        self.write(&[])
    }
    fn reply_answers(&self, _: &RequestOption, answers: &[QuestionAnswer]) -> Reply {
        self.write(answers)
    }
    fn withdraw(&self) {
        let _ = self.write(&[]);
    }
}

impl CodexServer {
    pub(super) fn question(&self, id: &Value, params: &Value) {
        let parsed = serde_json::from_value::<Params>(params.clone());
        let thread_id = params["threadId"].as_str().unwrap_or_default();
        let route = self.asker(thread_id);
        let invalid = |why: String| {
            let message = format!("Unsupported Codex question shape (expected CLI 0.169.0): {why}");
            self.respond_error(id, -32602, &message);
            if let Some((route, _)) = &route {
                (route.sink)(HarnessEvent::error(message));
            }
        };
        let params = match parsed {
            Ok(p) => p,
            Err(e) => return invalid(e.to_string()),
        };
        if params.thread_id.is_empty()
            || params.turn_id.is_empty()
            || params.item_id.is_empty()
            || params.auto_resolution_ms.is_some()
        {
            return invalid("missing identity or unsupported automatic resolution".into());
        }
        let Some((thread, agent)) = &route else {
            return self.respond_error(
                id,
                -32602,
                "Oculus does not answer questions on this thread",
            );
        };
        if thread.state.lock().unwrap().one_off {
            return self.respond_error(
                id,
                -32602,
                "Oculus does not answer questions on one-off jobs",
            );
        }
        if params.questions.iter().any(|q| q.options.is_empty()) {
            return invalid("questions require non-empty options".into());
        }
        let payload = QuestionPayload {
            questions: params
                .questions
                .into_iter()
                .map(|q| Question {
                    id: q.id,
                    header: q.header,
                    text: q.question,
                    options: q
                        .options
                        .into_iter()
                        .map(|o| QuestionOption {
                            label: o.label,
                            description: o.description,
                        })
                        .collect(),
                    multi: false,
                    allows_typed_answer: q.is_other,
                    secret: q.is_secret,
                })
                .collect(),
            is_blocking: Some(params.is_blocking),
        };
        let mut request = match payload.request(id.to_string()) {
            Ok(r) => r,
            Err(e) => return invalid(e),
        };
        request.payload["tool"] = json!(params.item_id);
        if let Some(agent) = agent {
            request.payload["agent"] = json!({"path":agent,"thread":params.thread_id});
        }
        thread
            .state
            .lock()
            .unwrap()
            .callbacks
            .insert(request.native_ref.clone());
        (thread.sink)(HarnessEvent::RequestAsked {
            request,
            route: Some(LiveRoute(Arc::new(QuestionRoute {
                server: self.me.clone(),
                thread: thread.clone(),
                wire_id: id.clone(),
            }))),
        });
    }
}
