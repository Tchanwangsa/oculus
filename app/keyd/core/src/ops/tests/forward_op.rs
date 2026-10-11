use super::*;

const KEY: &str = "pa-VAULT-KEY";

fn forwarding(dir: &Scratch, origin: &FakeOrigin) -> State {
    let state = state_in(dir).with_routes(
        Routes::compiled()
            .with_origin("voyage", &origin.origin)
            .unwrap(),
    );
    call(&state, "store", json!({"secret": "voyage", "value": KEY})).unwrap();
    state
}

pub(super) fn forward(state: &State, body: &[u8]) -> Result<Reply, OpError> {
    let req = json!({
        "op": "forward", "secret": "voyage", "method": "POST", "path": "/v1/multimodalembeddings",
        "headers": [["Content-Type", "application/json"], ["Accept", "application/json"]],
        "body_len": body.len(),
    });
    state.dispatch(&cli(), "forward", &req, body)
}

fn header<'a>(reply: &'a Reply, name: &str) -> Option<&'a str> {
    reply.header["headers"]
        .as_array()?
        .iter()
        .find(|h| h[0] == name)
        .and_then(|h| h[1].as_str())
}

pub(super) fn answer(status: u16, headers: Vec<(&'static str, String)>, body: &[u8]) -> Answer {
    Answer {
        status,
        headers,
        body: body.to_vec(),
    }
}

#[test]
fn forward_adds_the_vault_key_and_passes_the_answer_through() {
    let origin = FakeOrigin::start(|_| {
        answer(
            200,
            vec![("Content-Type", "application/json".into())],
            b"{\"usage\":{\"total_tokens\":7}}",
        )
    });
    let dir = Scratch::new("fwd-ok");
    let state = forwarding(&dir, &origin);

    let reply = forward(&state, b"{\"inputs\":[]}").unwrap();
    assert_eq!(reply.header["status"], 200);
    assert_eq!(reply.header["body_len"], reply.body.len());
    assert_eq!(reply.body, b"{\"usage\":{\"total_tokens\":7}}");
    assert_eq!(header(&reply, "content-type"), Some("application/json"));

    let hit = &origin.hits()[0];
    assert_eq!(
        (hit.method.as_str(), hit.path.as_str()),
        ("POST", "/v1/multimodalembeddings")
    );
    assert_eq!(
        hit.header("authorization"),
        Some(format!("Bearer {KEY}").as_str())
    );
    assert_eq!(hit.header("content-type"), Some("application/json"));
    assert_eq!(
        hit.header("accept-encoding"),
        None,
        "no gzip, so the body is the origin's bytes"
    );
    assert_eq!(hit.body, b"{\"inputs\":[]}");

    let shown = format!(
        "{} {} {:?}",
        reply.header,
        String::from_utf8_lossy(&reply.body),
        reply.note
    );
    assert!(!shown.contains(KEY), "{shown}");
    assert!(reply.note.unwrap().contains("status=200"));
}

#[test]
fn error_statuses_come_back_as_replies_byte_for_byte() {
    let bodies: [(u16, &[u8]); 5] = [
        (401, b"{\"detail\":\"Provided API key is invalid.\"}"),
        (402, b"{\"detail\":\"credit\"}"),
        (429, b"{\"detail\":\"3 RPM and 10K TPM\"}"),
        (503, b"<html>upstream \xff</html>"),
        (400, b""),
    ];
    for (status, body) in bodies {
        let origin =
            FakeOrigin::start(move |_| answer(status, vec![("Retry-After", "17".into())], body));
        let dir = Scratch::new("fwd-status");
        let reply = forward(&forwarding(&dir, &origin), b"{}").unwrap();
        assert_eq!(reply.header["status"], status);
        assert_eq!(reply.body, body);
        assert_eq!(header(&reply, "retry-after"), Some("17"));
    }
}

#[test]
fn a_redirect_is_returned_not_followed() {
    let origin = FakeOrigin::start(|_| {
        answer(
            302,
            vec![("Location", "http://127.0.0.1:1/elsewhere".into())],
            b"",
        )
    });
    let dir = Scratch::new("fwd-redirect");
    let reply = forward(&forwarding(&dir, &origin), b"{}").unwrap();
    assert_eq!(reply.header["status"], 302);
    assert_eq!(
        header(&reply, "location"),
        Some("http://127.0.0.1:1/elsewhere")
    );
    assert_eq!(origin.hits().len(), 1);
}

#[test]
fn no_stored_key_is_missing_and_nothing_is_sent() {
    let origin = FakeOrigin::start(|_| answer(200, vec![], b""));
    let dir = Scratch::new("fwd-missing");
    let state = state_in(&dir).with_routes(
        Routes::compiled()
            .with_origin("voyage", &origin.origin)
            .unwrap(),
    );
    assert_eq!(forward(&state, b"{}").unwrap_err().kind, "missing");
    assert!(origin.hits().is_empty());
}

#[test]
fn an_unreachable_origin_is_upstream_and_names_nothing_sent() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let dir = Scratch::new("fwd-upstream");
    let state = state_in(&dir).with_routes(
        Routes::compiled()
            .with_origin("voyage", &format!("http://127.0.0.1:{port}"))
            .unwrap(),
    );
    call(&state, "store", json!({"secret": "voyage", "value": KEY})).unwrap();
    let err = forward(&state, b"{\"inputs\":\"BODY-TEXT\"}").unwrap_err();
    assert_eq!(err.kind, "upstream");
    for leak in [KEY, "BODY-TEXT", "multimodal", &port.to_string()] {
        assert!(!err.detail.contains(leak), "{leak} in {}", err.detail);
    }
}

#[test]
fn a_bad_forward_is_refused_before_the_master_key_is_read() {
    let dir = Scratch::new("fwd-bad");
    let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
    for req in [
        json!({"secret": "voyage", "method": "POST", "path": "/v2/x"}),
        json!({"secret": "voyage", "method": "DELETE", "path": "/v1/x"}),
        json!({"secret": "voyage", "method": "POST", "path": "/v1/x", "headers": [["Authorization", "Bearer x"]]}),
        json!({"secret": "mineru", "method": "POST", "path": "/v1/x"}),
        json!({"secret": "groq", "method": "POST", "path": "/api/v4/x"}),
        json!({"secret": "okta.password", "method": "POST", "path": "/v1/x"}),
    ] {
        let err = state.dispatch(&cli(), "forward", &req, b"").unwrap_err();
        assert_eq!(err.kind, "request", "{req}");
    }
}
