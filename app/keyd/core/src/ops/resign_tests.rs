//! `forward` on the `canvas` route reviving its session, against a scripted
//! Canvas and Okta on one loopback server (`test_support::okta_fake`) and a
//! clock the tests move in whole 30 s TOTP windows.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use serde_json::{json, Value};

use super::testing::{as_role, key};
use super::*;
use crate::forward::Routes;
use crate::platform::Role;
use crate::test_support::okta_fake::{answer, code_from_t0, script, COOKIE, SEED, T0, USERNAME};
use crate::test_support::{Answer, FakeOrigin, Hit, Scratch, TestClock, BUILD};
use crate::vault::{NoLegacy, StaticKey};

const STALE: &str = "canvas_session=stale-SECRET; _csrf_token=stale-csrf";
const PASSWORD: &str = "hunter2";

fn cli() -> Caller {
    as_role(Role::Cli)
}

/// What Canvas says to an API call: the signed-in cookie is let in, anything
/// else is a 401 for an unauthenticated user.
fn canvas_api(hit: &Hit) -> Option<Answer> {
    let path = hit.path.split('?').next().unwrap_or("");
    if !(path.starts_with("/api/v1/courses") || path.starts_with("/files/")) {
        return None;
    }
    Some(if hit.header("cookie") == Some(COOKIE) {
        answer(200, &[("Content-Type", "application/json")], "[]")
    } else {
        answer(
            401,
            &[("Set-Cookie", "canvas_session=anonymous-SECRET; Path=/")],
            r#"{"status":"unauthenticated","errors":[{"message":"user authorization required"}]}"#,
        )
    })
}

/// One server playing Canvas's API (`api`, falling back to `canvas_api`) and
/// the whole sign-in.
fn origin(api: impl Fn(&Hit) -> Option<Answer> + Send + 'static) -> FakeOrigin {
    let okta = script(code_from_t0);
    FakeOrigin::start(move |hit| {
        api(hit)
            .or_else(|| canvas_api(hit))
            .unwrap_or_else(|| okta(hit))
    })
}

struct Rig {
    dir: Scratch,
    state: Arc<State>,
    clock: TestClock,
    canvas: FakeOrigin,
}

fn rig_with(canvas: FakeOrigin, ed: Option<&FakeOrigin>, session: Option<&str>) -> Rig {
    let dir = Scratch::new("resign");
    let clock = TestClock::at(T0);
    let port = canvas.origin.rsplit(':').next().unwrap().to_string();
    let mut routes = Routes::compiled();
    if let Some(ed) = ed {
        routes = routes.with_origin("ed", &ed.origin).unwrap();
    }
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(StaticKey(key())),
        Box::new(NoLegacy),
    )
    .with_clock(clock.clock())
    .with_routes(routes)
    .with_origins(
        Some(&format!("http://127.0.0.1:{port}")),
        Some(&format!("http://localhost:{port}")),
    )
    .unwrap();
    let rig = Rig {
        dir,
        state: Arc::new(state),
        clock,
        canvas,
    };
    if let Some(session) = session {
        rig.op("session_put", json!({"kind": "canvas", "value": session}));
    }
    rig
}

/// A rig with working credentials and `STALE` as Canvas's session.
fn rig(canvas: FakeOrigin) -> Rig {
    let rig = rig_with(canvas, None, Some(STALE));
    rig.save_credentials(PASSWORD);
    rig
}

impl Rig {
    fn op(&self, name: &str, req: Value) -> Value {
        self.state
            .dispatch(&cli(), name, &req, b"")
            .unwrap_or_else(|e| panic!("{name}: {}", e.detail))
            .header
    }

    fn save_credentials(&self, password: &str) {
        self.op(
            "okta_save",
            json!({"username": USERNAME, "password": password, "totp_secret": SEED}),
        );
    }

    fn forward(&self, caller: &Caller, path: &str, stream: bool) -> Result<Reply, OpError> {
        let req = json!({"op": "forward", "secret": "canvas", "method": "GET", "path": path, "stream": stream});
        self.state.dispatch(caller, "forward", &req, b"")
    }

    fn get(&self, path: &str) -> Reply {
        self.forward(&cli(), path, false)
            .unwrap_or_else(|e| panic!("{}: {}", e.kind, e.detail))
    }

    fn sign_ins(&self) -> usize {
        self.canvas
            .hits()
            .iter()
            .filter(|h| h.path.starts_with("/idp/idx/introspect"))
            .count()
    }

    fn hits_for(&self, prefix: &str) -> usize {
        self.canvas
            .hits()
            .iter()
            .filter(|h| h.path.starts_with(prefix))
            .count()
    }

    fn held(&self) -> Option<String> {
        crate::vault::Vault::new(crate::paths::vault(&self.dir.0), key())
            .get("session.canvas")
            .unwrap()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(crate::paths::sign_in_log(&self.dir.0)).unwrap_or_default()
    }

    fn write_record(&self, json: &str) {
        std::fs::create_dir_all(crate::paths::session_dir(&self.dir.0)).unwrap();
        std::fs::write(crate::paths::sign_in_record(&self.dir.0), json).unwrap();
    }
}

fn status(reply: &Reply) -> u64 {
    reply.header["status"].as_u64().unwrap()
}

fn signin_code(reply: &Reply) -> Option<&str> {
    reply.header["signin"]["code"].as_str()
}

/// Everything a client or the log could be shown of a reply.
fn shown(reply: &Reply) -> String {
    format!(
        "{} {} {:?}",
        reply.header,
        String::from_utf8_lossy(&reply.body),
        reply.note
    )
}

fn assert_no_cookie(text: &str) {
    for secret in [
        "stale-SECRET",
        "stale-csrf",
        "anonymous-SECRET",
        "canvas_session=real",
        "csrf1",
        PASSWORD,
        SEED,
    ] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
}

#[test]
fn twenty_requests_rejected_together_are_one_sign_in_and_all_end_200() {
    let rig = rig(origin(|_| None));
    let replies: Vec<Reply> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..20)
            .map(|i| {
                let rig = &rig;
                scope.spawn(move || {
                    let caller = if i % 2 == 0 {
                        cli()
                    } else {
                        as_role(Role::App)
                    };
                    rig.forward(&caller, &format!("/api/v1/courses?page={i}"), i % 3 == 0)
                        .unwrap()
                })
            })
            .collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    });

    assert_eq!(rig.sign_ins(), 1, "one login for the whole burst");
    for (i, reply) in replies.iter().enumerate() {
        assert_eq!(status(reply), 200, "{i}: {}", shown(reply));
        assert!(reply.header.get("signin").is_none(), "{i}");
        assert_no_cookie(&shown(reply));
    }
    assert_eq!(rig.held().as_deref(), Some(COOKIE));
    let log = rig.log();
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(log.contains("forward: signed in"), "{log}");
    assert!(crate::paths::authenticated(&rig.dir.0).exists());
    // Every request reached Canvas with the stale cookie first and the fresh
    // one after, never a third time.
    let courses: Vec<Hit> = rig
        .canvas
        .hits()
        .into_iter()
        .filter(|h| h.path.starts_with("/api/v1/courses"))
        .collect();
    assert!(
        (20..=40).contains(&courses.len()),
        "{} requests reached Canvas",
        courses.len()
    );
}

#[test]
fn a_request_that_finds_the_session_already_replaced_retries_without_signing_in() {
    let state_cell: Arc<OnceLock<Arc<State>>> = Arc::new(OnceLock::new());
    let first = Arc::new(AtomicUsize::new(0));
    let rig = {
        let (cell, first) = (state_cell.clone(), first.clone());
        // Canvas rejects the first call, but only after some other request has
        // replaced the session (as a sign-in elsewhere would).
        let rig = rig_with(
            origin(move |hit| {
                if hit.path.starts_with("/api/v1/courses")
                    && first.fetch_add(1, Ordering::SeqCst) == 0
                {
                    cell.get()
                        .unwrap()
                        .dispatch(
                            &cli(),
                            "session_put",
                            &json!({"kind": "canvas", "value": COOKIE}),
                            b"",
                        )
                        .unwrap();
                    return Some(answer(401, &[], "{}"));
                }
                None
            }),
            None,
            Some(STALE),
        );
        rig.save_credentials(PASSWORD);
        rig
    };
    state_cell.set(rig.state.clone()).ok();

    let reply = rig.get("/api/v1/courses");
    assert_eq!(status(&reply), 200);
    assert_eq!(rig.sign_ins(), 0, "someone else already had the new cookie");
    assert_eq!(rig.hits_for("/api/v1/courses"), 2);
    assert!(reply.note.as_deref().unwrap().contains("retried"));
    assert!(rig.log().is_empty(), "no attempt was made");
}

#[test]
fn a_refused_sign_in_returns_the_rejection_with_the_reason_and_asks_okta_nothing() {
    // Paused by an earlier lockout.
    let rig = rig(origin(|_| None));
    rig.write_record(&format!(
        r#"{{"last":{},"failures":1,"paused":"The account is locked or blocked: x","credentials_paused":true}}"#,
        T0 - 7200
    ));
    let reply = rig.get("/api/v1/courses");
    assert_eq!(status(&reply), 401);
    assert_eq!(signin_code(&reply), Some("paused"));
    assert!(reply.header["signin"]["detail"]
        .as_str()
        .unwrap()
        .contains("locked"));
    assert_eq!(rig.sign_ins(), 0);
    assert_eq!(rig.canvas.hits().len(), 1, "Canvas once, Okta never");
    assert_eq!(
        rig.held().as_deref(),
        Some(STALE),
        "the session is untouched"
    );
    assert_no_cookie(&shown(&reply));
    assert_eq!(
        String::from_utf8_lossy(&reply.body),
        r#"{"status":"unauthenticated","errors":[{"message":"user authorization required"}]}"#,
        "the rejection itself comes back"
    );
    assert!(reply.header["headers"]
        .as_array()
        .unwrap()
        .iter()
        .all(|h| h[0] != "set-cookie"));
}

#[test]
fn the_guards_spacing_and_back_off_hold_a_second_sign_in_off() {
    let rig = rig(origin(|_| None));
    assert_eq!(status(&rig.get("/api/v1/courses")), 200);
    assert_eq!(rig.sign_ins(), 1);

    // The session dies again at once.
    rig.op("session_put", json!({"kind": "canvas", "value": STALE}));
    for windows in 0..19 {
        let reply = rig.get("/api/v1/courses");
        assert_eq!(status(&reply), 401, "{windows}");
        assert_eq!(signin_code(&reply), Some("waiting"), "{windows}");
        let wait = reply.header["signin"]["wait_secs"].as_u64().unwrap();
        assert_eq!(wait, 600 - 30 * windows, "{windows}");
        assert_eq!(rig.sign_ins(), 1, "{windows}: Okta was not asked again");
        rig.clock.advance(30);
    }
    // Ten minutes after the first attempt it may run again.
    rig.clock.advance(30);
    assert_eq!(status(&rig.get("/api/v1/courses")), 200);
    assert_eq!(rig.sign_ins(), 2);
}

#[test]
fn a_second_rejection_after_a_sign_in_is_returned_and_never_signs_in_again() {
    let rig = rig(origin(|hit| {
        hit.path
            .starts_with("/api/v1/never")
            .then(|| answer(401, &[], "{}"))
    }));
    let reply = rig.get("/api/v1/never");
    assert_eq!(status(&reply), 401);
    assert!(reply.header.get("signin").is_none(), "a sign-in did run");
    assert_eq!(rig.sign_ins(), 1);
    assert_eq!(
        rig.hits_for("/api/v1/never"),
        2,
        "sent, signed in, sent once more"
    );
    assert!(reply.note.as_deref().unwrap().contains("signin=signed_in"));
    assert_eq!(
        rig.held().as_deref(),
        Some(COOKIE),
        "the new session stands"
    );

    // The next request is judged by the guard, so Okta hears nothing more.
    let reply = rig.get("/api/v1/never");
    assert_eq!(status(&reply), 401);
    assert_eq!(signin_code(&reply), Some("waiting"));
    assert_eq!(rig.sign_ins(), 1);
    assert_eq!(rig.hits_for("/api/v1/never"), 3);
}

#[test]
fn only_a_401_or_a_redirect_to_sign_in_is_a_dead_session() {
    for (code, body) in [
        (403, "{}"),
        (404, "{}"),
        (422, "{}"),
        (429, "{}"),
        (500, "boom"),
        (502, "bad gateway"),
        (503, "{}"),
        // Signed in, but not allowed to see this one.
        (
            401,
            r#"{"status":"unauthorized","errors":[{"message":"user not authorized"}]}"#,
        ),
    ] {
        let rig = rig(origin(move |hit| {
            hit.path
                .starts_with("/api/v1/other")
                .then(|| answer(code, &[], body))
        }));
        let reply = rig.get("/api/v1/other");
        assert_eq!(status(&reply), code as u64);
        assert_eq!(String::from_utf8_lossy(&reply.body), body);
        assert_eq!(rig.sign_ins(), 0, "{code}");
        assert_eq!(rig.canvas.hits().len(), 1, "{code}: sent once");
        assert!(reply.header.get("signin").is_none());
        assert_eq!(rig.held().as_deref(), Some(STALE));
    }
}

#[test]
fn a_redirect_to_the_sso_host_or_canvas_login_is_rejected_and_one_to_a_file_host_is_not() {
    for location in [
        "SSO:/app/canvas/sso/saml",
        "/login/saml",
        "/login",
        "SELF:/login/canvas",
    ] {
        let rig = rig(origin({
            let location = location.to_string();
            move |hit| {
                if !hit.path.starts_with("/api/v1/courses") || hit.header("cookie") == Some(COOKIE)
                {
                    return None;
                }
                let host = hit.header("host").unwrap_or("");
                let port = host.rsplit(':').next().unwrap_or("");
                let target = location
                    .replace("SSO:", &format!("http://localhost:{port}"))
                    .replace("SELF:", &format!("http://127.0.0.1:{port}"));
                Some(answer(302, &[("Location", &target)], ""))
            }
        }));
        let reply = rig.get("/api/v1/courses");
        assert_eq!(status(&reply), 200, "{location}: {}", shown(&reply));
        assert_eq!(rig.sign_ins(), 1, "{location}");
    }

    for location in [
        "https://files.example-cdn.com/signed?token=abc",
        "/files/9/download?verifier=x",
        "SELF:/courses/1",
    ] {
        let rig = rig(origin({
            let location = location.to_string();
            move |hit| {
                hit.path.starts_with("/files/").then(|| {
                    let host = hit.header("host").unwrap_or("");
                    let port = host.rsplit(':').next().unwrap_or("");
                    let target = location.replace("SELF:", &format!("http://127.0.0.1:{port}"));
                    answer(302, &[("Location", &target)], "")
                })
            }
        }));
        let reply = rig.get("/files/9/download");
        assert_eq!(status(&reply), 302, "{location}");
        assert_eq!(rig.sign_ins(), 0, "{location}");
        assert_eq!(rig.canvas.hits().len(), 1, "{location}");
    }
}

#[test]
fn ed_never_signs_in_and_a_401_there_is_the_clients() {
    let ed = FakeOrigin::start(|_| answer(401, &[], r#"{"error":"bad token"}"#));
    let rig = rig_with(origin(|_| None), Some(&ed), Some(STALE));
    rig.save_credentials(PASSWORD);
    rig.op(
        "session_put",
        json!({"kind": "ed", "value": "OLD.ED.TOKEN"}),
    );

    let req = json!({"op": "forward", "secret": "ed", "method": "GET", "path": "/api/user"});
    let reply = rig.state.dispatch(&cli(), "forward", &req, b"").unwrap();
    assert_eq!(status(&reply), 401);
    assert!(reply.header.get("signin").is_none());
    assert_eq!(rig.sign_ins(), 0);
    assert_eq!(rig.canvas.hits().len(), 0);
    assert_eq!(ed.hits().len(), 1);

    rig.op("session_clear", json!({"kinds": ["ed"]}));
    let err = rig
        .state
        .dispatch(&cli(), "forward", &req, b"")
        .unwrap_err();
    assert_eq!(err.kind, "missing");
    assert!(err.signin.is_none(), "no sign-in is tried for Ed");
    assert_eq!(rig.sign_ins(), 0);
}

#[test]
fn no_canvas_session_starts_a_sign_in_and_then_sends() {
    let rig = rig_with(origin(|_| None), None, None);
    rig.save_credentials(PASSWORD);
    let reply = rig.get("/api/v1/courses");
    assert_eq!(status(&reply), 200);
    assert_eq!(rig.sign_ins(), 1);
    assert_eq!(
        rig.hits_for("/api/v1/courses"),
        1,
        "nothing was sent before the session"
    );
    assert_eq!(rig.held().as_deref(), Some(COOKIE));
}

#[test]
fn no_canvas_session_and_no_way_to_sign_in_is_missing_with_the_reason() {
    // No credentials on file.
    let rig = rig_with(origin(|_| None), None, None);
    let err = rig.forward(&cli(), "/api/v1/courses", false).unwrap_err();
    assert_eq!(err.kind, "missing");
    let wire = err.to_json();
    assert_eq!(wire["error"], "missing");
    assert_eq!(
        wire["signin"],
        json!({"result": "error", "code": "not_configured"})
    );
    assert_eq!(rig.canvas.hits().len(), 0);

    // Signed out.
    rig.save_credentials(PASSWORD);
    rig.op("sign_out", json!({}));
    let err = rig.forward(&cli(), "/api/v1/courses", true).unwrap_err();
    assert_eq!(err.to_json()["signin"]["code"], "signed_out");
    assert_eq!(rig.canvas.hits().len(), 0);

    // Paused.
    let rig = rig_with(origin(|_| None), None, None);
    rig.save_credentials(PASSWORD);
    rig.write_record(&format!(
        r#"{{"last":{},"failures":1,"paused":"The account is locked or blocked: x","credentials_paused":true}}"#,
        T0 - 7200
    ));
    let err = rig.forward(&cli(), "/api/v1/courses", false).unwrap_err();
    assert_eq!(err.to_json()["signin"]["code"], "paused");
    assert_eq!(rig.sign_ins(), 0);
}

#[test]
fn a_signed_out_keyd_stands_forward_down() {
    let rig = rig(origin(|_| None));
    crate::session::markers::mark_signed_out(&rig.dir.0).unwrap();
    let reply = rig.get("/api/v1/courses");
    assert_eq!(status(&reply), 401);
    assert_eq!(signin_code(&reply), Some("signed_out"));
    assert_eq!(rig.sign_ins(), 0);
    assert_eq!(rig.canvas.hits().len(), 1);
    assert!(rig.log().is_empty());
}

#[test]
fn a_streamed_request_rejected_at_its_head_is_retried_and_streams_the_new_answer() {
    let rig = rig(origin(|hit| {
        (hit.path.starts_with("/files/") && hit.header("cookie") == Some(COOKIE))
            .then(|| answer(200, &[("Content-Type", "application/pdf")], "PDF-BYTES"))
    }));
    let mut reply = rig
        .forward(&cli(), "/files/9/download?download_frd=1", true)
        .unwrap();
    assert_eq!(status(&reply), 200);
    assert!(reply.header.get("body_len").is_none());
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut reply.stream.as_mut().unwrap().0, &mut body).unwrap();
    assert_eq!(body, b"PDF-BYTES");
    assert_eq!(rig.sign_ins(), 1);
    assert_eq!(rig.hits_for("/files/9"), 2);

    // And when the sign-in is refused, the stream is the rejection.
    let rig = self::rig(origin(|_| None));
    crate::session::markers::mark_signed_out(&rig.dir.0).unwrap();
    let mut reply = rig.forward(&cli(), "/files/9/download", true).unwrap();
    assert_eq!(status(&reply), 401);
    assert_eq!(signin_code(&reply), Some("signed_out"));
    let mut body = Vec::new();
    std::io::Read::read_to_end(&mut reply.stream.as_mut().unwrap().0, &mut body).unwrap();
    assert!(String::from_utf8_lossy(&body).contains("unauthenticated"));
}

#[test]
fn a_post_body_is_sent_again_on_the_retry() {
    let rig = rig(origin(|hit| {
        (hit.method == "POST" && hit.path.starts_with("/api/v1/courses")).then(|| {
            if hit.header("cookie") == Some(COOKIE) {
                answer(200, &[], &String::from_utf8_lossy(&hit.body))
            } else {
                answer(401, &[], "{}")
            }
        })
    }));
    let body = b"a=1&b=2";
    let req = json!({"op": "forward", "secret": "canvas", "method": "POST", "path": "/api/v1/courses/1/x",
        "headers": [["Content-Type", "application/x-www-form-urlencoded"]], "body_len": body.len()});
    let reply = rig.state.dispatch(&cli(), "forward", &req, body).unwrap();
    assert_eq!(status(&reply), 200);
    assert_eq!(reply.body, body);
    assert_eq!(rig.sign_ins(), 1);
}

#[test]
fn a_failed_sign_in_leaks_no_cookie_and_a_burst_still_makes_one_attempt() {
    let rig = rig(origin(|_| None));
    rig.save_credentials("not-the-password");
    let replies: Vec<Reply> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..20)
            .map(|i| {
                let rig = &rig;
                scope.spawn(move || rig.get(&format!("/api/v1/courses?page={i}")))
            })
            .collect();
        threads.into_iter().map(|t| t.join().unwrap()).collect()
    });
    assert_eq!(rig.sign_ins(), 1, "twenty rejections, one attempt at Okta");
    for reply in &replies {
        assert_eq!(status(reply), 401);
        assert!(
            matches!(
                signin_code(reply),
                Some("bad_password" | "not_configured" | "paused" | "waiting")
            ),
            "{}",
            shown(reply)
        );
        assert_no_cookie(&shown(reply));
        assert!(reply.header["headers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h[0] != "set-cookie"));
    }
    assert!(replies
        .iter()
        .any(|r| signin_code(r) == Some("bad_password")));
    assert_eq!(
        rig.held().as_deref(),
        Some(STALE),
        "a failed attempt keeps the old session"
    );
    assert_no_cookie(&rig.log());
    assert_eq!(rig.log().lines().count(), 1);

    // The rejected password is gone, so the next request has nothing to try.
    let reply = rig.get("/api/v1/courses");
    assert_eq!(signin_code(&reply), Some("not_configured"));
    assert_eq!(rig.sign_ins(), 1);
}

#[test]
fn a_rejected_answers_cookies_are_neither_kept_nor_returned_and_a_good_answers_are_kept() {
    let rig = rig(origin(|hit| {
        (hit.path.starts_with("/api/v1/good") && hit.header("cookie") == Some(COOKIE)).then(|| {
            answer(
                200,
                &[("Set-Cookie", "canvas_session=real; Path=/; Max-Age=3600")],
                "[]",
            )
        })
    }));
    // The unauthenticated answer would replace the session with an anonymous
    // cookie; only the sign-in's session may.
    let reply = rig.get("/api/v1/courses");
    assert_eq!(status(&reply), 200);
    assert_eq!(rig.held().as_deref(), Some(COOKIE));
    let before = rig.state.session_generation();
    rig.get("/api/v1/good");
    assert_eq!(
        rig.state.session_generation(),
        before,
        "an unchanged cookie is no change"
    );
}

#[test]
fn the_cloud_routes_never_sign_in() {
    let voyage = FakeOrigin::start(|_| answer(401, &[], "{}"));
    let dir = Scratch::new("resign-cloud");
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(StaticKey(key())),
        Box::new(NoLegacy),
    )
    .with_routes(
        Routes::compiled()
            .with_origin("voyage", &voyage.origin)
            .unwrap(),
    );
    state
        .dispatch(
            &cli(),
            "store",
            &json!({"secret": "voyage", "value": "pa-k"}),
            b"",
        )
        .unwrap();
    let req = json!({"op": "forward", "secret": "voyage", "method": "GET", "path": "/v1/models"});
    let reply = state.dispatch(&cli(), "forward", &req, b"").unwrap();
    assert_eq!(reply.header["status"], 401);
    assert!(reply.header.get("signin").is_none());
    assert_eq!(voyage.hits().len(), 1);
    assert!(!crate::paths::sign_in_log(&dir.0).exists());
}
