use std::sync::{Arc, Mutex, Weak};

use serde_json::{json, Value};

use crate::harness::child::{ChildProc, ThreadSpawn};
use crate::harness::requests::{QuestionAnswer, Reply};
use crate::harness::HarnessEvent;

use super::{questions::Form, AcpSession};

fn params() -> Value {
    json!({"sessionId":"s","toolCallId":"ask-1","mode":"form","message":"The agent has a question.",
        "requestedSchema":{"type":"object","properties":{
            "q1":{"type":"string","title":"Colour","description":"Choose a colour", "oneOf":[{"const":"1","title":"Red","description":"Warm"},{"const":"2","title":"Blue"}]},
            "q1_other":{"type":"string","title":"Other","description":"Optional text"}
        }}
    })
}

fn answer(id: &str, values: &[&str], dismissed: bool) -> QuestionAnswer {
    QuestionAnswer {
        question_id: id.into(),
        answers: values.iter().map(|s| s.to_string()).collect(),
        dismissed,
    }
}

#[test]
fn a_form_keeps_native_keys_titles_descriptions_constants_and_other() {
    let f = Form::parse(&params(), "s", "\"1\"".into()).unwrap();
    let q = &f.request.payload["questions"][0];
    assert_eq!(q["id"], "q1");
    assert_eq!(q["header"], "Colour");
    assert_eq!(q["text"], "Choose a colour");
    assert_eq!(q["options"][0]["description"], "Warm");
    assert_eq!(q["multi"], false);
    assert_eq!(q["allowsTypedAnswer"], true);
    assert_eq!(
        f.encode(&[answer("q1", &["Blue"], false)]).unwrap(),
        json!({"action":"accept","content":{"q1":"2","q1_other":"Blue"}})
    );
    assert_eq!(
        f.encode(&[answer("q1", &["Green"], false)]).unwrap(),
        json!({"action":"accept","content":{"q1_other":"Green"}})
    );
    assert_eq!(
        f.encode(&[answer("q1", &[], true)]).unwrap(),
        json!({"action":"accept","content":{}})
    );
}

#[test]
fn arrays_two_questions_choice_only_and_partial_dismissal_encode_without_guessing_ids() {
    let mut p = params();
    let props = &mut p["requestedSchema"]["properties"];
    props["q1"]["type"] = json!("array");
    props["q1"]["items"] = json!({"type":"string","oneOf":props["q1"].as_object_mut().unwrap().remove("oneOf").unwrap()});
    props["second"] = json!({"type":"string","title":"Size","oneOf":[{"const":"tiny","title":"Small"},{"const":"big","title":"Large"}]});
    let f = Form::parse(&p, "s", "1".into()).unwrap();
    assert_eq!(f.request.payload["questions"][0]["multi"], true);
    assert_eq!(
        f.request.payload["questions"][1]["allowsTypedAnswer"],
        false
    );
    let a = [
        answer("second", &[], true),
        answer("q1", &["Red", "Blue", "Green"], false),
    ];
    assert_eq!(
        f.encode(&a).unwrap(),
        json!({"action":"accept","content":{"q1":["1","2"],"q1_other":"Red, Blue, Green"}})
    );
    assert!(f
        .encode(&[answer("second", &["typed"], false), a[1].clone()])
        .is_err());
    assert!(f
        .encode(&[answer("second", &["Small", "Large"], false), a[1].clone()])
        .is_err());
}

#[test]
fn unknown_or_ambiguous_schema_shapes_are_refused() {
    for mutate in [
        |p: &mut Value| p["mode"] = json!("url"),
        |p: &mut Value| p["sessionId"] = json!("another-session"),
        |p: &mut Value| p["requestedSchema"]["properties"]["q1"]["format"] = json!("password"),
        |p: &mut Value| p["requestedSchema"]["properties"]["q1"]["type"] = json!("number"),
        |p: &mut Value| p["requestedSchema"]["properties"]["q1"]["oneOf"][1]["const"] = json!("1"),
        |p: &mut Value| {
            p["requestedSchema"]["properties"]["q1"]["oneOf"][1]["title"] = json!("Red")
        },
        |p: &mut Value| p["requestedSchema"]["properties"]["q1"]["oneOf"] = json!([]),
        |p: &mut Value| {
            p["requestedSchema"]["properties"]
                .as_object_mut()
                .unwrap()
                .remove("q1");
        },
        |p: &mut Value| p["requestedSchema"]["properties"]["q1_other"]["type"] = json!("array"),
        |p: &mut Value| p["requestedSchema"]["required"] = json!(["missing"]),
    ] {
        let mut p = params();
        mutate(&mut p);
        assert!(Form::parse(&p, "s", "1".into()).is_err(), "{p}");
    }
}

type Events = Arc<Mutex<Vec<HarnessEvent>>>;
type Frames = Arc<Mutex<Vec<Value>>>;
fn session(one_off: bool) -> (Arc<AcpSession>, Events, Frames) {
    let base = ThreadSpawn {
        bin: "agy".into(),
        cwd: "/scratch/agents".into(),
        library: "/scratch".into(),
        resume: Some("s".into()),
        model: None,
        effort: None,
        env: vec![],
        raw_log: None,
    };
    let me: Arc<Mutex<Weak<AcpSession>>> = Arc::default();
    let frames: Frames = Arc::default();
    let (reply, heard) = (me.clone(), frames.clone());
    let proc = ChildProc::replay(Arc::new(move |frame| {
        heard.lock().unwrap().push(frame.clone());
        if frame.get("method").is_some()
            && frame.get("id").is_some()
            && frame["method"] != "session/prompt"
        {
            let result = if frame["method"] == "initialize" {
                json!({"agentInfo":{"version":"1.5.9"}})
            } else {
                json!({})
            };
            reply
                .lock()
                .unwrap()
                .upgrade()
                .unwrap()
                .rpc_reply(json!({"id":frame["id"],"result":result}));
        }
        Ok(())
    }));
    let events: Events = Arc::default();
    let got = events.clone();
    let session = AcpSession::new(
        proc,
        1,
        Arc::new(move |e| got.lock().unwrap().push(e)),
        &base,
        one_off,
    );
    *me.lock().unwrap() = Arc::downgrade(&session);
    session.open(&base, "").unwrap();
    (session, events, frames)
}
fn ask(session: &AcpSession, p: Value) {
    session.inbound(&json!({"jsonrpc":"2.0","method":"elicitation/create","id":"1","params":p}));
}
fn route(events: &Events) -> crate::harness::requests::LiveRoute {
    events
        .lock()
        .unwrap()
        .iter()
        .find_map(|e| match e {
            HarnessEvent::RequestAsked { route, .. } => route.clone(),
            _ => None,
        })
        .unwrap()
}
fn option(id: &str) -> crate::harness::requests::RequestOption {
    Form::parse(&params(), "s", "1".into())
        .unwrap()
        .request
        .options
        .into_iter()
        .find(|o| o.id == id)
        .unwrap()
}

#[test]
fn chat_advertises_forms_but_one_off_jobs_decline_even_a_stray_form() {
    for one_off in [false, true] {
        let (s, events, frames) = session(one_off);
        assert_eq!(
            frames.lock().unwrap()[0]["params"]["clientCapabilities"]
                .get("elicitation")
                .is_none(),
            one_off
        );
        s.send("hello", &crate::harness::event::unbound()).unwrap();
        ask(&s, params());
        if one_off {
            assert!(!events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, HarnessEvent::RequestAsked { .. })));
            assert_eq!(
                frames.lock().unwrap().last().unwrap()["result"],
                json!({"action":"decline"})
            );
        } else {
            assert_eq!(
                route(&events)
                    .0
                    .reply_answers(&option("answer"), &[answer("q1", &["Blue"], false)]),
                Reply::Acknowledged
            );
            assert_eq!(
                frames.lock().unwrap().last().unwrap()["result"],
                json!({"action":"accept","content":{"q1":"2","q1_other":"Blue"}})
            );
        }
        s.kill();
    }
}

#[test]
fn dismiss_withdraw_stop_teardown_and_a_retired_tool_guard_late_answers() {
    for how in ["dismiss", "withdraw", "stop", "kill", "retire", "end"] {
        let (s, events, frames) = session(false);
        s.send("hello", &crate::harness::event::unbound()).unwrap();
        ask(&s, params());
        let r = route(&events);
        match how {
            "dismiss" => assert_eq!(r.0.reply(&option("dismiss")), Reply::Acknowledged),
            "withdraw" => r.0.withdraw(),
            "stop" => s.interrupt().unwrap(),
            "kill" => s.kill(),
            "retire" => s.inbound(&json!({"method":"session/update","params":{"update":{"sessionUpdate":"tool_call_update","toolCallId":"ask-1","status":"failed"}}})),
            "end" => s.rpc_reply(json!({"id":4,"result":{"stopReason":"end_turn"}})),
            _ => unreachable!(),
        }
        if matches!(how, "dismiss" | "withdraw" | "stop") {
            assert_eq!(
                frames.lock().unwrap().last().unwrap()["result"],
                json!({"action":"decline"})
            );
        }
        if how == "retire" {
            assert!(events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, HarnessEvent::RequestRetired { .. })));
        }
        let count = frames.lock().unwrap().len();
        assert!(
            matches!(
                r.0.reply_answers(&option("answer"), &[answer("q1", &["Blue"], false)]),
                Reply::Failed(_)
            ),
            "{how}"
        );
        assert_eq!(frames.lock().unwrap().len(), count);
        s.kill();
    }
}

#[test]
fn an_unknown_question_fails_loudly_and_a_question_without_a_turn_is_declined() {
    let (s, events, frames) = session(false);
    ask(&s, params());
    assert_eq!(
        frames.lock().unwrap().last().unwrap()["result"],
        json!({"action":"decline"})
    );
    s.send("hello", &crate::harness::event::unbound()).unwrap();
    let mut p = params();
    p["requestedSchema"]["properties"]["q1"]["format"] = json!("secret");
    ask(&s, p);
    let heard = frames.lock().unwrap();
    assert_eq!(heard.last().unwrap()["error"]["code"], -32602);
    assert!(heard.last().unwrap()["error"]["message"]
        .as_str()
        .unwrap()
        .contains("agy 1.5.9"));
    assert!(events
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, HarnessEvent::Error { message,..} if message.contains("agy 1.5.9"))));
    s.kill();
}
