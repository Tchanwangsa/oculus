//! The session ops and the session routes, from the client, through the real
//! server loop and ops in memory.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::forward::Routes;
use crate::ops::State;
use crate::platform::{memory, Role};
use crate::server::Server;
use crate::test_support::{Answer, FakeOrigin, Peers, Scratch, BUILD};
use crate::vault::{MasterKey, NoLegacy, StaticKey};

const COOKIE: &str = "canvas_session=SECRET-CANVAS==; _csrf_token=csrf1";
const TOKEN: &str = "SECRET.ED.TOKEN";
const WAIT: Option<Duration> = Some(Duration::from_secs(10));

fn serve(role: Role, routes: Routes, dir: &Scratch) -> Client {
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(StaticKey(MasterKey::from_bytes([6; 32]))),
        Box::new(NoLegacy),
    )
    .with_routes(routes);
    let (listener, connector) = memory::listener();
    let server = Arc::new(Server::new(
        state,
        Box::new(Peers::admit_as(role)),
        Duration::from_secs(60),
    ));
    std::thread::spawn(move || server.run(&[listener]));
    Client {
        endpoint: PathBuf::from("memory"),
        connect: Arc::new(move || Ok(connector.connect())),
    }
}

fn routes(canvas: &FakeOrigin, ed: &FakeOrigin) -> Routes {
    Routes::compiled()
        .with_origin("canvas", &canvas.origin)
        .unwrap()
        .with_origin("ed", &ed.origin)
        .unwrap()
}

fn ok(body: &[u8]) -> Answer {
    Answer {
        status: 200,
        headers: vec![],
        body: body.to_vec(),
    }
}

#[test]
fn the_session_ops_round_trip_and_only_the_app_reads_a_cookie_back() {
    let (canvas, ed) = (
        FakeOrigin::start(|_| ok(b"")),
        FakeOrigin::start(|_| ok(b"")),
    );
    let dir = Scratch::new("client-session-ops");
    let app = serve(Role::App, routes(&canvas, &ed), &dir);
    let cli = serve(Role::Cli, routes(&canvas, &ed), &dir);

    assert_eq!(
        app.session_status().unwrap(),
        SessionStatus {
            canvas: false,
            sso: false,
            ed: false
        }
    );
    cli.session_put(SessionKind::Canvas, COOKIE).unwrap();
    app.session_put(SessionKind::Sso, "sid=SECRET-SSO").unwrap();
    cli.session_put(SessionKind::Ed, TOKEN).unwrap();
    let status = cli.session_status().unwrap();
    assert!(SessionKind::ALL.into_iter().all(|k| status.has(k)));

    let cookies = app.session_get().unwrap();
    assert_eq!(cookies.canvas.as_deref(), Some(COOKIE));
    assert_eq!(cookies.sso.as_deref(), Some("sid=SECRET-SSO"));
    assert!(!format!("{cookies:?}").contains("SECRET"));
    assert!(matches!(cli.session_get(), Err(KeydError::Caller(_))));

    assert_eq!(
        cli.session_clear(&[SessionKind::Sso, SessionKind::Ed])
            .unwrap(),
        [SessionKind::Sso, SessionKind::Ed]
    );
    assert_eq!(
        app.session_clear(&SessionKind::ALL).unwrap(),
        [SessionKind::Canvas]
    );
    let cookies = app.session_get().unwrap();
    assert_eq!((cookies.canvas, cookies.sso), (None, None));
}

#[test]
fn a_value_keyd_cannot_carry_is_refused_by_the_client_and_by_keyd() {
    let (canvas, ed) = (
        FakeOrigin::start(|_| ok(b"")),
        FakeOrigin::start(|_| ok(b"")),
    );
    let dir = Scratch::new("client-session-big");
    let app = serve(Role::App, routes(&canvas, &ed), &dir);
    let big = "a".repeat(70 * 1024);
    assert!(matches!(
        app.session_put(SessionKind::Canvas, &big),
        Err(KeydError::Request(_))
    ));
    for bad in ["", "a=1\r\nHost: x"] {
        assert!(matches!(
            app.session_put(SessionKind::Canvas, bad),
            Err(KeydError::Request(_))
        ));
    }
    // Around the client's check, the header line is over keyd's limit.
    let raw = app.exchange(
        &json!({"op": "session_put", "kind": "canvas", "value": big}),
        &[],
        WAIT,
    );
    assert!(matches!(raw, Err(KeydError::Request(_))), "{raw:?}");
    let just_over = "a".repeat(crate::session::MAX_VALUE + 1);
    let raw = app.exchange(
        &json!({"op": "session_put", "kind": "canvas", "value": just_over}),
        &[],
        WAIT,
    );
    assert!(matches!(raw, Err(KeydError::Request(_))), "{raw:?}");
    assert!(!app.session_status().unwrap().canvas);
}

#[test]
fn the_generic_ops_cannot_write_a_session_but_has_reports_one() {
    let (canvas, ed) = (
        FakeOrigin::start(|_| ok(b"")),
        FakeOrigin::start(|_| ok(b"")),
    );
    let dir = Scratch::new("client-session-generic");
    let cli = serve(Role::Cli, routes(&canvas, &ed), &dir);
    for name in ["session.canvas", "session.sso", "session.ed"] {
        assert!(matches!(cli.store(name, "a=1"), Err(KeydError::Request(_))));
        assert!(matches!(cli.delete(name), Err(KeydError::Request(_))));
        assert!(!cli.has(name).unwrap());
    }
    cli.session_put(SessionKind::Ed, TOKEN).unwrap();
    assert!(cli.has("session.ed").unwrap());
}

#[test]
fn forward_attaches_each_session_and_the_client_never_sees_one() {
    let canvas = FakeOrigin::start(|_| Answer {
        status: 200,
        headers: vec![
            ("Set-Cookie", "canvas_session=ROTATED; Path=/".into()),
            ("Link", "<https://x.example/n>; rel=\"next\"".into()),
        ],
        body: b"[]".to_vec(),
    });
    let ed = FakeOrigin::start(|hit| ok(&hit.body));
    let dir = Scratch::new("client-session-forward");
    let cli = serve(Role::Cli, routes(&canvas, &ed), &dir);

    assert!(matches!(
        cli.send("canvas", "GET", "/api/v1/courses", &[], b"", WAIT),
        Err(KeydError::Missing(_))
    ));
    assert!(matches!(
        cli.send("ed", "GET", "/api/threads/1", &[], b"", WAIT),
        Err(KeydError::Missing(_))
    ));
    cli.session_put(SessionKind::Canvas, COOKIE).unwrap();
    cli.session_put(SessionKind::Ed, TOKEN).unwrap();

    let got = cli
        .send(
            "canvas",
            "GET",
            "/api/v1/courses?include[]=term&per_page=100",
            &[("Accept", "application/json")],
            b"",
            WAIT,
        )
        .unwrap();
    assert_eq!((got.status, got.body.as_slice()), (200, &b"[]"[..]));
    assert!(got.header("link").is_some());
    assert!(got.header("set-cookie").is_none());
    assert!(!format!("{got:?}").contains("SECRET"));
    let hit = &canvas.hits()[0];
    assert_eq!(hit.path, "/api/v1/courses?include[]=term&per_page=100");
    assert_eq!(hit.header("cookie"), Some(COOKIE));

    let got = cli
        .send(
            "ed",
            "POST",
            "/api/renew_token",
            &[("Content-Type", "application/json")],
            b"{\"a\":1}",
            WAIT,
        )
        .unwrap();
    assert_eq!(got.body, b"{\"a\":1}");
    assert_eq!(ed.hits()[0].header("x-token"), Some(TOKEN));

    // The rotated cookie is in keyd now, and rides the next request.
    cli.send("canvas", "GET", "/api/v1/users/self", &[], b"", WAIT)
        .unwrap();
    assert_eq!(
        canvas.hits()[1].header("cookie"),
        Some("canvas_session=ROTATED; _csrf_token=csrf1")
    );
    assert_eq!(
        cli.session_clear(&[SessionKind::Canvas]).unwrap(),
        [SessionKind::Canvas]
    );

    for (name, value) in [("Cookie", "a=1"), ("X-Token", "t"), ("Authorization", "x")] {
        let refused = cli.send("ed", "GET", "/api/x", &[(name, value)], b"", WAIT);
        assert!(matches!(refused, Err(KeydError::Request(_))), "{name}");
    }
    let tricks = [
        "/api/%2e%2e/x",
        "/api/x%2fy",
        "//evil.example/x",
        "/api/x#y",
    ];
    for path in tricks {
        let refused = cli.send("ed", "GET", path, &[], b"", WAIT);
        assert!(matches!(refused, Err(KeydError::Request(_))), "{path}");
    }
}

fn body_of(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

#[test]
fn a_five_mebibyte_body_streams_through_intact() {
    let payload = body_of(5 * 1024 * 1024);
    let canvas = FakeOrigin::start({
        let payload = payload.clone();
        move |_| Answer {
            status: 200,
            headers: vec![
                ("Content-Type", "application/octet-stream".into()),
                ("Set-Cookie", "canvas_session=ROTATED".into()),
            ],
            body: payload.clone(),
        }
    });
    let ed = FakeOrigin::start(|_| ok(b""));
    let dir = Scratch::new("client-stream");
    let cli = serve(Role::Cli, routes(&canvas, &ed), &dir);
    cli.session_put(SessionKind::Canvas, COOKIE).unwrap();

    let mut got = cli
        .send_stream(
            "canvas",
            "GET",
            "/files/1/download?download_frd=1",
            &[],
            b"",
            WAIT,
        )
        .unwrap();
    assert_eq!(got.status, 200);
    assert_eq!(got.content_length(), Some(payload.len() as u64));
    assert_eq!(got.header("content-type"), Some("application/octet-stream"));
    assert!(got.header("set-cookie").is_none());
    let mut body = Vec::new();
    got.body.read_to_end(&mut body).unwrap();
    assert!(body == payload, "{} bytes differ", body.len());
    assert_eq!(canvas.hits()[0].header("cookie"), Some(COOKIE));

    // The cookie that rode the stream's headers was absorbed.
    cli.send("canvas", "GET", "/api/v1/x", &[], b"", WAIT)
        .unwrap();
    assert_eq!(
        canvas.hits()[1].header("cookie"),
        Some("canvas_session=ROTATED; _csrf_token=csrf1")
    );
}

#[test]
fn a_stream_with_a_refusal_or_no_session_is_an_error_not_a_body() {
    let (canvas, ed) = (
        FakeOrigin::start(|_| ok(b"")),
        FakeOrigin::start(|_| ok(b"")),
    );
    let dir = Scratch::new("client-stream-errors");
    let cli = serve(Role::Cli, routes(&canvas, &ed), &dir);
    assert!(matches!(
        cli.send_stream("canvas", "GET", "/files/1", &[], b"", WAIT),
        Err(KeydError::Missing(_))
    ));
    assert!(matches!(
        cli.send_stream("canvas", "GET", "/api/%2e%2e/x", &[], b"", WAIT),
        Err(KeydError::Request(_))
    ));
    let absent = Client {
        endpoint: PathBuf::from("/d/keyd.sock"),
        connect: Arc::new(|| Err(ConnectError::Absent("ENOENT".into()))),
    };
    assert!(matches!(
        absent.send_stream("canvas", "GET", "/", &[], b"", WAIT),
        Err(KeydError::Absent)
    ));
}

#[test]
fn an_origin_cut_mid_body_shows_as_a_body_shorter_than_its_content_length() {
    let canvas = FakeOrigin::start_raw(|_, stream| {
        stream
            .write_all(b"HTTP/1.1 200 X\r\nContent-Length: 200000\r\nConnection: close\r\n\r\n")
            .unwrap();
        stream.write_all(&body_of(50_000)).unwrap();
    });
    let ed = FakeOrigin::start(|_| ok(b""));
    let dir = Scratch::new("client-stream-cut");
    let cli = serve(Role::Cli, routes(&canvas, &ed), &dir);
    cli.session_put(SessionKind::Canvas, COOKIE).unwrap();
    let mut got = cli
        .send_stream("canvas", "GET", "/files/1/download", &[], b"", WAIT)
        .unwrap();
    assert_eq!(got.content_length(), Some(200_000));
    let mut body = Vec::new();
    got.body.read_to_end(&mut body).unwrap();
    assert!(body.len() < 200_000, "{}", body.len());
}

#[test]
fn a_stalled_origin_frees_its_thread_and_a_slow_steady_download_completes() {
    let canvas = FakeOrigin::start_raw(|hit, stream| {
        stream
            .write_all(b"HTTP/1.1 200 X\r\nContent-Length: 20\r\nConnection: close\r\n\r\n")
            .unwrap();
        if hit.path.starts_with("/steady") {
            for _ in 0..10 {
                std::thread::sleep(Duration::from_millis(120));
                stream.write_all(b"xx").unwrap();
            }
        } else {
            stream.write_all(b"xx").unwrap();
            std::thread::sleep(Duration::from_secs(4));
        }
    });
    let ed = FakeOrigin::start(|_| ok(b""));
    let dir = Scratch::new("client-stream-stall");
    let short = routes(&canvas, &ed)
        .with_read_timeout("canvas", Some(Duration::from_millis(400)))
        .unwrap();
    let cli = serve(Role::Cli, short, &dir);
    cli.session_put(SessionKind::Canvas, COOKIE).unwrap();

    let started = Instant::now();
    let steady = cli
        .send("canvas", "GET", "/steady", &[], b"", WAIT)
        .unwrap();
    assert_eq!(steady.body, [b'x'; 20]);
    assert!(started.elapsed() > Duration::from_millis(1000));

    let started = Instant::now();
    let stalled = cli.send("canvas", "GET", "/stall", &[], b"", WAIT);
    assert!(
        matches!(stalled, Err(KeydError::Upstream(_))),
        "{stalled:?}"
    );
    assert!(started.elapsed() < Duration::from_secs(3));

    let mut got = cli
        .send_stream("canvas", "GET", "/stall", &[], b"", WAIT)
        .unwrap();
    let mut body = Vec::new();
    got.body.read_to_end(&mut body).unwrap();
    assert!(body.len() < 20);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_redirect_off_origin_is_returned_and_nothing_follows_it() {
    let elsewhere = FakeOrigin::start(|_| ok(b"WRONG"));
    let target = format!("{}/signed", elsewhere.origin);
    let canvas = FakeOrigin::start({
        let target = target.clone();
        move |_| Answer {
            status: 302,
            headers: vec![("Location", target.clone())],
            body: vec![],
        }
    });
    let ed = FakeOrigin::start(|_| ok(b""));
    let dir = Scratch::new("client-redirect");
    let cli = serve(Role::Cli, routes(&canvas, &ed), &dir);
    cli.session_put(SessionKind::Canvas, COOKIE).unwrap();
    let got = cli
        .send("canvas", "GET", "/files/9/download", &[], b"", WAIT)
        .unwrap();
    assert_eq!(got.status, 302);
    assert_eq!(got.header("location"), Some(target.as_str()));
    let got = cli
        .send_stream("canvas", "GET", "/files/9/download", &[], b"", WAIT)
        .unwrap();
    assert_eq!(got.status, 302);
    assert!(elsewhere.hits().is_empty());
}
