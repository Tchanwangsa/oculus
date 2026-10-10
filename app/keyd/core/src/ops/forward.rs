//! The `forward` op: one request to a route's fixed origin, with the
//! credential from the vault attached and, on a session route, kept out of
//! every answer.

use serde_json::{json, Value};

use super::{BodyStream, OpError, Reply, State};
use crate::forward::{read_capped, Auth, Call};
use crate::session::{store, Kind};

/// Headers a session route's answer never carries to the client: a rotated
/// cookie is absorbed into the vault instead, and the rest would echo a
/// credential.
const WITHHELD: &[&str] = &["set-cookie", "set-cookie2", "authorization", "x-token"];

impl State {
    /// Sends the request, checked before the master key is read so a bad one
    /// never prompts. Any status the origin answers is a reply, not an error.
    /// With `"stream": true` the answer's body is not buffered: the reply
    /// header has no `body_len` and the body follows until keyd closes the
    /// connection. Neither the vault nor any lock is held while it does.
    pub(super) fn forward(&self, req: &Value, body: &[u8]) -> Result<Reply, OpError> {
        let name = req
            .get("secret")
            .and_then(Value::as_str)
            .ok_or_else(|| OpError::new("request", "missing \"secret\""))?;
        let route = self
            .routes
            .get(name)
            .ok_or_else(|| OpError::new("request", format!("{name} cannot be forwarded")))?;
        let call = Call::parse(req, route, body)?;
        let stream = match req.get("stream") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(on)) => *on,
            Some(_) => return Err(OpError::new("request", "stream must be true or false")),
        };

        let vault = self.vault()?;
        let secret = route.auth.secret();
        self.import_once(&vault, secret)?;
        let credential = vault
            .get(secret)?
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                let what = if route.is_session() { "session" } else { "key" };
                OpError::new("missing", format!("no {name} {what} is stored"))
            })?;
        let opened = self.upstream.open(route, &call, &credential, body)?;

        let mut headers = opened.headers;
        if route.is_session() {
            let set_cookies: Vec<String> = headers
                .iter()
                .filter(|(k, _)| k == "set-cookie")
                .map(|(_, v)| v.clone())
                .collect();
            headers.retain(|(k, _)| !WITHHELD.contains(&k.as_str()));
            if matches!(route.auth, Auth::Cookie(_)) && !set_cookies.is_empty() {
                let kind = Kind::ALL.into_iter().find(|k| k.secret() == secret);
                if let Some(kind) = kind {
                    if let Err(e) = store::absorb(&vault, &self.generation, kind, &set_cookies) {
                        crate::log(&format!("{name}: a rotated cookie was not saved: {e}"));
                    }
                }
            }
        }
        let headers: Vec<[&str; 2]> = headers
            .iter()
            .map(|(k, v)| [k.as_str(), v.as_str()])
            .collect();

        if stream {
            return Ok(Reply {
                header: json!({"status": opened.status, "headers": headers}),
                body: Vec::new(),
                note: Some(format!(
                    "secret={name} status={} bytes_out={} streamed",
                    opened.status,
                    body.len()
                )),
                stream: Some(BodyStream(opened.body)),
            });
        }
        let answer = read_capped(opened.body)?;
        Ok(Reply {
            header: json!({"status": opened.status, "headers": headers, "body_len": answer.len()}),
            note: Some(format!(
                "secret={name} status={} bytes_out={} bytes_in={}",
                opened.status,
                body.len(),
                answer.len()
            )),
            body: answer,
            stream: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::super::testing::{call, cli, state_in};
    use super::*;
    use crate::forward::Routes;
    use crate::test_support::{Answer, FakeOrigin, Scratch, BUILD};
    use crate::vault::{KeyError, KeySource, MasterKey, NoLegacy};

    const COOKIE: &str = "canvas_session=SECRET-COOKIE==; _csrf_token=csrf1";
    const TOKEN: &str = "SECRET.ED.TOKEN";

    fn answer(status: u16, headers: Vec<(&'static str, String)>, body: &[u8]) -> Answer {
        Answer {
            status,
            headers,
            body: body.to_vec(),
        }
    }

    fn ok(body: &[u8]) -> Answer {
        answer(200, vec![], body)
    }

    /// A keyd whose `canvas` and `ed` routes are the given fakes, holding both
    /// sessions.
    fn with_sessions(dir: &Scratch, canvas: &FakeOrigin, ed: &FakeOrigin) -> State {
        let state = state_in(dir).with_routes(
            Routes::compiled()
                .with_origin("canvas", &canvas.origin)
                .unwrap()
                .with_origin("ed", &ed.origin)
                .unwrap(),
        );
        call(
            &state,
            "session_put",
            json!({"kind": "canvas", "value": COOKIE}),
        )
        .unwrap();
        call(&state, "session_put", json!({"kind": "ed", "value": TOKEN})).unwrap();
        state
    }

    fn get(state: &State, route: &str, path: &str) -> Result<Reply, OpError> {
        let req = json!({"op": "forward", "secret": route, "method": "GET", "path": path});
        state.dispatch(&cli(), "forward", &req, b"")
    }

    fn header<'a>(reply: &'a Reply, name: &str) -> Option<&'a str> {
        reply.header["headers"]
            .as_array()?
            .iter()
            .find(|h| h[0] == name)
            .and_then(|h| h[1].as_str())
    }

    fn held(dir: &Scratch, name: &str) -> Option<String> {
        crate::vault::Vault::new(crate::paths::vault(&dir.0), super::super::testing::key())
            .get(name)
            .unwrap()
    }

    fn everything_the_client_sees(reply: &Reply) -> String {
        format!(
            "{} {} {:?}",
            reply.header,
            String::from_utf8_lossy(&reply.body),
            reply.note
        )
    }

    #[test]
    fn canvas_gets_the_cookie_and_ed_the_token_and_neither_comes_back() {
        let canvas = FakeOrigin::start(|_| {
            answer(
                200,
                vec![
                    ("Content-Type", "application/json".into()),
                    ("Set-Cookie", "tracker=1; Path=/".into()),
                    ("Authorization", "echo SECRET-COOKIE".into()),
                    ("X-Token", "echo SECRET.ED.TOKEN".into()),
                    ("Link", "<https://x.example/next>; rel=\"next\"".into()),
                ],
                b"{\"ok\":true}",
            )
        });
        let ed = FakeOrigin::start(|_| ok(b"{\"threads\":[]}"));
        let dir = Scratch::new("fwd-sessions");
        let state = with_sessions(&dir, &canvas, &ed);

        let path = "/api/v1/courses?include[]=term&include[]=account&per_page=100";
        let reply = get(&state, "canvas", path).unwrap();
        assert_eq!(reply.header["status"], 200);
        assert_eq!(reply.body, b"{\"ok\":true}");
        let hit = &canvas.hits()[0];
        assert_eq!(hit.path, path, "the path reaches the origin as written");
        assert_eq!(hit.header("cookie"), Some(COOKIE));
        assert_eq!(hit.header("authorization"), None);
        assert_eq!(hit.header("x-token"), None);
        assert_eq!(header(&reply, "content-type"), Some("application/json"));
        assert!(
            header(&reply, "link").is_some(),
            "other headers pass through"
        );
        for withheld in ["set-cookie", "authorization", "x-token"] {
            assert_eq!(header(&reply, withheld), None, "{withheld}");
        }
        let shown = everything_the_client_sees(&reply);
        for secret in ["SECRET-COOKIE", "SECRET.ED.TOKEN", "csrf1"] {
            assert!(!shown.contains(secret), "{secret} in {shown}");
        }

        let reply = get(&state, "ed", "/api/threads/77?view=1").unwrap();
        assert_eq!(reply.body, b"{\"threads\":[]}");
        let hit = &ed.hits()[0];
        assert_eq!(hit.path, "/api/threads/77?view=1");
        assert_eq!(hit.header("x-token"), Some(TOKEN));
        assert_eq!(hit.header("cookie"), None);
        assert_eq!(hit.header("authorization"), None);
        let shown = everything_the_client_sees(&reply);
        assert!(!shown.contains("SECRET"), "{shown}");
    }

    #[test]
    fn ed_takes_a_post_with_a_json_body() {
        let ed = FakeOrigin::start(|hit| ok(&hit.body));
        let canvas = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-ed-post");
        let state = with_sessions(&dir, &canvas, &ed);
        let body = b"{\"course_id\":9}";
        let req = json!({"op": "forward", "secret": "ed", "method": "POST", "path": "/api/login_token",
            "headers": [["Content-Type", "application/json"]], "body_len": body.len()});
        let reply = state.dispatch(&cli(), "forward", &req, body).unwrap();
        assert_eq!(reply.body, body);
        let hit = &ed.hits()[0];
        assert_eq!(
            (hit.method.as_str(), hit.body.as_slice()),
            ("POST", &body[..])
        );
        assert_eq!(hit.header("content-type"), Some("application/json"));
        assert_eq!(hit.header("x-token"), Some(TOKEN));
    }

    #[test]
    fn a_session_route_forwards_range_and_validators() {
        let canvas = FakeOrigin::start(|_| {
            answer(206, vec![("Content-Range", "bytes 0-1/9".into())], b"ab")
        });
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-range");
        let state = with_sessions(&dir, &canvas, &ed);
        let req = json!({"op": "forward", "secret": "canvas", "method": "GET", "path": "/files/1/download?download_frd=1&verifier=abc",
            "headers": [["Range", "bytes=0-1"], ["If-None-Match", "\"e\""], ["If-Modified-Since", "Sat, 01 Jan 2000 00:00:00 GMT"]]});
        let reply = state.dispatch(&cli(), "forward", &req, b"").unwrap();
        assert_eq!(reply.header["status"], 206);
        let hit = &canvas.hits()[0];
        assert_eq!(hit.header("range"), Some("bytes=0-1"));
        assert_eq!(hit.header("if-none-match"), Some("\"e\""));
        assert!(hit.header("if-modified-since").is_some());
    }

    #[test]
    fn a_client_cannot_supply_the_credential_headers_on_any_route() {
        let canvas = FakeOrigin::start(|_| ok(b""));
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-forged");
        let state = with_sessions(&dir, &canvas, &ed);
        for route in ["canvas", "ed"] {
            for name in ["Cookie", "cookie", "X-Token", "x-token", "Authorization"] {
                let req = json!({"op": "forward", "secret": route, "method": "GET", "path": "/api/x",
                    "headers": [[name, "forged=1"]]});
                let err = state.dispatch(&cli(), "forward", &req, b"").unwrap_err();
                assert_eq!(err.kind, "request", "{route} {name}");
            }
        }
        assert!(canvas.hits().is_empty() && ed.hits().is_empty());
    }

    #[test]
    fn a_rotated_cookie_is_absorbed_merged_and_never_returned() {
        let n = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = n.clone();
        let canvas = FakeOrigin::start(move |_| match count.fetch_add(1, Ordering::SeqCst) {
            0 => answer(
                200,
                vec![
                    (
                        "Set-Cookie",
                        "canvas_session=ROTATED; path=/; secure; httponly".into(),
                    ),
                    ("Set-Cookie", "added=1; Path=/".into()),
                ],
                b"",
            ),
            1 => answer(
                200,
                vec![
                    ("Set-Cookie", "_csrf_token=; Path=/".into()),
                    ("Set-Cookie", "added=2; Max-Age=0".into()),
                ],
                b"",
            ),
            _ => ok(b""),
        });
        let ed = FakeOrigin::start(|_| answer(200, vec![("Set-Cookie", "ed=1".into())], b""));
        let dir = Scratch::new("fwd-absorb");
        let state = with_sessions(&dir, &canvas, &ed);
        let before = state.session_generation();

        let reply = get(&state, "canvas", "/api/v1/courses").unwrap();
        assert_eq!(header(&reply, "set-cookie"), None);
        assert_eq!(
            held(&dir, "session.canvas").as_deref(),
            Some("canvas_session=ROTATED; _csrf_token=csrf1; added=1"),
            "a rotated cookie keeps its place, a new one goes last"
        );
        assert_eq!(state.session_generation(), before + 1);

        get(&state, "canvas", "/api/v1/courses").unwrap();
        assert_eq!(
            held(&dir, "session.canvas").as_deref(),
            Some("canvas_session=ROTATED"),
            "an empty value and a Max-Age of 0 both remove"
        );
        assert_eq!(state.session_generation(), before + 2);

        get(&state, "canvas", "/api/v1/courses").unwrap();
        let hits = canvas.hits();
        assert_eq!(hits[0].header("cookie"), Some(COOKIE));
        assert_eq!(
            hits[1].header("cookie"),
            Some("canvas_session=ROTATED; _csrf_token=csrf1; added=1")
        );
        assert_eq!(
            hits[2].header("cookie"),
            Some("canvas_session=ROTATED"),
            "the next request carries the merge"
        );
        assert_eq!(state.session_generation(), before + 2);

        // An answer with no cookie is not a change.
        get(&state, "canvas", "/api/v1/courses").unwrap();
        assert_eq!(state.session_generation(), before + 2);

        // Ed's Set-Cookie is neither absorbed nor returned.
        let reply = get(&state, "ed", "/api/user").unwrap();
        assert_eq!(header(&reply, "set-cookie"), None);
        assert_eq!(held(&dir, "session.ed").as_deref(), Some(TOKEN));
        assert_eq!(state.session_generation(), before + 2);
    }

    #[test]
    fn a_cleared_session_stays_cleared_when_a_late_answer_rotates_it() {
        let other = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-absorb-cleared");
        let state = with_sessions(&dir, &other, &other);
        // The origin answers after the session was dropped: the answer's
        // cookies have nothing to merge into.
        let slow = FakeOrigin::start_raw({
            let state_dir = dir.0.clone();
            move |_, stream| {
                crate::vault::Vault::new(
                    crate::paths::vault(&state_dir),
                    super::super::testing::key(),
                )
                .remove("session.canvas")
                .unwrap();
                stream
                    .write_all(b"HTTP/1.1 200 X\r\nSet-Cookie: late=1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .unwrap();
            }
        });
        let state = state.with_routes(
            Routes::compiled()
                .with_origin("canvas", &slow.origin)
                .unwrap(),
        );
        get(&state, "canvas", "/api/v1/courses").unwrap();
        assert_eq!(held(&dir, "session.canvas"), None);
    }

    #[test]
    fn a_redirect_is_returned_unfollowed_and_the_cookie_goes_nowhere_else() {
        let elsewhere = FakeOrigin::start(|_| ok(b"WRONG"));
        let target = format!("{}/landing?token=abc", elsewhere.origin);
        let canvas = FakeOrigin::start({
            let target = target.clone();
            move |_| {
                answer(
                    302,
                    vec![
                        ("Location", target.clone()),
                        ("Set-Cookie", "canvas_session=AFTER-REDIRECT; Path=/".into()),
                    ],
                    b"",
                )
            }
        });
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-redirect-session");
        let state = with_sessions(&dir, &canvas, &ed);

        let reply = get(
            &state,
            "canvas",
            "/courses/12/external_tools/34?display=borderless",
        )
        .unwrap();
        assert_eq!(reply.header["status"], 302);
        assert_eq!(header(&reply, "location"), Some(target.as_str()));
        assert_eq!(header(&reply, "set-cookie"), None);
        assert_eq!(canvas.hits().len(), 1);
        assert!(elsewhere.hits().is_empty(), "keyd never follows");
        assert_eq!(
            held(&dir, "session.canvas").as_deref(),
            Some("canvas_session=AFTER-REDIRECT; _csrf_token=csrf1"),
            "a Set-Cookie on a redirect is still the origin's"
        );

        // A redirect from Ed carries Ed's token to nobody else either.
        let ed = FakeOrigin::start(move |_| answer(307, vec![("Location", target.clone())], b""));
        let state = state.with_routes(
            Routes::compiled()
                .with_origin("canvas", &canvas.origin)
                .unwrap()
                .with_origin("ed", &ed.origin)
                .unwrap(),
        );
        assert_eq!(get(&state, "ed", "/api/x").unwrap().header["status"], 307);
        assert!(elsewhere.hits().is_empty());
    }

    #[test]
    fn no_session_is_missing_and_nothing_is_sent() {
        let canvas = FakeOrigin::start(|_| ok(b""));
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-no-session");
        let state = state_in(&dir).with_routes(
            Routes::compiled()
                .with_origin("canvas", &canvas.origin)
                .unwrap()
                .with_origin("ed", &ed.origin)
                .unwrap(),
        );
        for route in ["canvas", "ed"] {
            let err = get(&state, route, "/api/x").unwrap_err();
            assert_eq!(err.kind, "missing", "{route}");
        }
        call(
            &state,
            "session_put",
            json!({"kind": "canvas", "value": "a=1"}),
        )
        .unwrap();
        call(&state, "session_clear", json!({"kinds": ["canvas"]})).unwrap();
        assert_eq!(get(&state, "canvas", "/api/x").unwrap_err().kind, "missing");
        assert!(canvas.hits().is_empty() && ed.hits().is_empty());
    }

    #[test]
    fn a_bad_session_forward_is_refused_before_the_master_key_is_read() {
        struct Refusing;
        impl KeySource for Refusing {
            fn get_or_create(&self) -> Result<MasterKey, KeyError> {
                Err(KeyError::Refused("no".into()))
            }
        }
        let dir = Scratch::new("fwd-bad-session");
        let state = State::new(BUILD, dir.0.clone(), Box::new(Refusing), Box::new(NoLegacy));
        for req in [
            json!({"secret": "canvas", "method": "GET", "path": "/api/%2e%2e/x"}),
            json!({"secret": "canvas", "method": "PUT", "path": "/api/x"}),
            json!({"secret": "canvas", "method": "GET", "path": "/api/x", "headers": [["Cookie", "a=1"]]}),
            json!({"secret": "ed", "method": "GET", "path": "/user"}),
            json!({"secret": "ed", "method": "GET", "path": "/api/x", "stream": "yes"}),
            json!({"secret": "session.canvas", "method": "GET", "path": "/api/x"}),
            json!({"secret": "sso", "method": "GET", "path": "/"}),
            json!({"method": "GET", "path": "/"}),
        ] {
            let err = state.dispatch(&cli(), "forward", &req, b"").unwrap_err();
            assert_eq!(err.kind, "request", "{req}");
        }
    }

    fn big_body() -> Vec<u8> {
        (0..5 * 1024 * 1024u32).map(|i| (i % 251) as u8).collect()
    }

    fn streaming(state: &State, route: &str, path: &str) -> Result<Reply, OpError> {
        let req = json!({"op": "forward", "secret": route, "method": "GET", "path": path, "stream": true});
        state.dispatch(&cli(), "forward", &req, b"")
    }

    fn drain(reply: &mut Reply) -> (Vec<u8>, bool) {
        let mut out = Vec::new();
        let clean = reply
            .stream
            .as_mut()
            .unwrap()
            .0
            .read_to_end(&mut out)
            .is_ok();
        (out, clean)
    }

    #[test]
    fn a_streamed_reply_has_no_body_len_and_holds_no_lock_while_it_flows() {
        let body = big_body();
        let release = Arc::new(AtomicBool::new(false));
        let canvas = FakeOrigin::start_raw({
            let (body, release) = (body.clone(), release.clone());
            move |_, stream| {
                let head = format!(
                    "HTTP/1.1 200 X\r\nContent-Length: {}\r\nSet-Cookie: s=1\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(head.as_bytes()).unwrap();
                stream.write_all(&body[..1000]).unwrap();
                let started = Instant::now();
                while !release.load(Ordering::SeqCst) && started.elapsed() < Duration::from_secs(10)
                {
                    std::thread::sleep(Duration::from_millis(5));
                }
                stream.write_all(&body[1000..]).ok();
            }
        });
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-stream");
        let state = with_sessions(&dir, &canvas, &ed);

        let mut reply = streaming(&state, "canvas", "/files/9/download?download_frd=1").unwrap();
        assert_eq!(reply.header["status"], 200);
        assert!(reply.header.get("body_len").is_none());
        assert!(reply.body.is_empty());
        assert_eq!(
            header(&reply, "content-length"),
            Some(body.len().to_string().as_str())
        );
        assert_eq!(header(&reply, "set-cookie"), None);
        assert!(reply.note.as_deref().unwrap().contains("streamed"));

        // Mid-stream, the vault and the state are free to other ops.
        let started = Instant::now();
        call(
            &state,
            "session_put",
            json!({"kind": "ed", "value": "NEW.TOKEN"}),
        )
        .unwrap();
        call(&state, "session_status", json!({})).unwrap();
        assert!(started.elapsed() < Duration::from_secs(2));
        release.store(true, Ordering::SeqCst);

        let (got, clean) = drain(&mut reply);
        assert!(clean);
        assert_eq!(got.len(), body.len());
        assert!(got == body, "the streamed bytes differ");
    }

    #[test]
    fn an_origin_that_closes_mid_body_leaves_the_stream_short() {
        let canvas = FakeOrigin::start_raw(|_, stream| {
            stream
                .write_all(b"HTTP/1.1 200 X\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n")
                .unwrap();
            stream.write_all(&[7u8; 40_000]).unwrap();
        });
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-stream-short");
        let state = with_sessions(&dir, &canvas, &ed);
        let mut reply = streaming(&state, "canvas", "/files/9/download").unwrap();
        assert_eq!(header(&reply, "content-length"), Some("100000"));
        let (got, clean) = drain(&mut reply);
        assert!(got.len() < 100_000, "{}", got.len());
        assert!(!clean, "the cut is an error to whoever copies the stream");
    }

    #[test]
    fn a_buffered_forward_of_a_cut_body_is_an_upstream_error() {
        let canvas = FakeOrigin::start_raw(|_, stream| {
            stream
                .write_all(b"HTTP/1.1 200 X\r\nContent-Length: 100000\r\nConnection: close\r\n\r\n")
                .unwrap();
            stream.write_all(&[7u8; 40_000]).unwrap();
        });
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-buffered-short");
        let state = with_sessions(&dir, &canvas, &ed);
        assert_eq!(
            get(&state, "canvas", "/files/9/download").unwrap_err().kind,
            "upstream"
        );
    }

    #[test]
    fn a_stalled_origin_fails_that_forward_and_a_slow_steady_stream_completes() {
        let stall = FakeOrigin::start_raw(|hit, stream| {
            if hit.path.starts_with("/never") {
                std::thread::sleep(Duration::from_secs(3));
                return;
            }
            stream
                .write_all(b"HTTP/1.1 200 X\r\nContent-Length: 10\r\nConnection: close\r\n\r\nabc")
                .unwrap();
            std::thread::sleep(Duration::from_secs(3));
        });
        let steady = FakeOrigin::start_raw(|_, stream| {
            stream
                .write_all(b"HTTP/1.1 200 X\r\nContent-Length: 20\r\nConnection: close\r\n\r\n")
                .unwrap();
            for _ in 0..10 {
                std::thread::sleep(Duration::from_millis(120));
                stream.write_all(b"xx").unwrap();
            }
        });
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-timeout");
        let routes = |canvas: &FakeOrigin| {
            Routes::compiled()
                .with_origin("canvas", &canvas.origin)
                .unwrap()
                .with_origin("ed", &ed.origin)
                .unwrap()
                .with_read_timeout("canvas", Some(Duration::from_millis(400)))
                .unwrap()
        };
        let state = state_in(&dir).with_routes(routes(&stall));
        call(
            &state,
            "session_put",
            json!({"kind": "canvas", "value": COOKIE}),
        )
        .unwrap();

        // No head at all: the read stalls.
        let started = Instant::now();
        let err = get(&state, "canvas", "/never").unwrap_err();
        assert_eq!(err.kind, "upstream");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
        assert!(!err.detail.contains("SECRET"), "{}", err.detail);

        // A head, then silence: buffered fails, streamed ends short.
        let started = Instant::now();
        assert_eq!(get(&state, "canvas", "/half").unwrap_err().kind, "upstream");
        assert!(started.elapsed() < Duration::from_secs(2));
        let mut reply = streaming(&state, "canvas", "/half").unwrap();
        let (got, clean) = drain(&mut reply);
        assert!(got.len() < 10 && !clean);

        // The same detector lets a stream that is slow but never silent finish,
        // though it takes three times as long as one read may.
        let state = state.with_routes(routes(&steady));
        let started = Instant::now();
        let reply = get(&state, "canvas", "/slow").unwrap();
        assert_eq!(reply.body, [b'x'; 20]);
        assert!(started.elapsed() > Duration::from_millis(1000));
        let mut reply = streaming(&state, "canvas", "/slow").unwrap();
        let (got, clean) = drain(&mut reply);
        assert!(clean && got == [b'x'; 20]);
    }

    #[test]
    fn a_cloud_route_still_has_no_read_timeout() {
        let slow = FakeOrigin::start_raw(|_, stream| {
            std::thread::sleep(Duration::from_millis(700));
            stream
                .write_all(b"HTTP/1.1 200 X\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
        });
        let dir = Scratch::new("fwd-cloud-slow");
        let state = state_in(&dir).with_routes(
            Routes::compiled()
                .with_origin("voyage", &slow.origin)
                .unwrap()
                .with_read_timeout("canvas", Some(Duration::from_millis(100)))
                .unwrap(),
        );
        call(
            &state,
            "store",
            json!({"secret": "voyage", "value": "pa-k"}),
        )
        .unwrap();
        let req =
            json!({"op": "forward", "secret": "voyage", "method": "GET", "path": "/v1/models"});
        let reply = state.dispatch(&cli(), "forward", &req, b"").unwrap();
        assert_eq!(reply.body, b"ok");
    }

    #[test]
    fn the_log_note_names_the_route_and_never_a_path_or_a_value() {
        let canvas = FakeOrigin::start(|_| ok(b"body"));
        let ed = FakeOrigin::start(|_| ok(b""));
        let dir = Scratch::new("fwd-note");
        let state = with_sessions(&dir, &canvas, &ed);
        let reply = get(
            &state,
            "canvas",
            "/files/1/download?download_frd=1&verifier=SECRET-VERIFIER",
        )
        .unwrap();
        let note = reply.note.unwrap();
        assert!(
            note.contains("secret=canvas") && note.contains("status=200"),
            "{note}"
        );
        assert!(
            !note.contains("SECRET") && !note.contains("verifier"),
            "{note}"
        );
    }
}
