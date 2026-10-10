use super::client::store_token;
use super::courses::code_token;
use super::document::document_md;
use super::lti::{ed_course_in_url, exchange_login_token, walk_lti_chain};
use super::render::fmt_ts;
use super::Ed;
use crate::providers::credentials::Credentialed;
use crate::test_support::{FakeKeyd, FakeServer, Reply, Scratch};
use serde_json::json;

#[test]
fn subject_tokens_ignore_staff_typed_suffixes() {
    assert_eq!(code_token("comp10002 2024s2"), "COMP10002");
    assert_eq!(code_token("SWEN20003_2025_S2"), "SWEN20003");
    assert_eq!(code_token("INFO30006"), "INFO30006");
    assert_eq!(code_token("  "), "");
}

#[test]
fn timestamps_lose_the_offset_but_keep_the_minute() {
    assert_eq!(
        fmt_ts("2026-08-07T15:42:01.522942+10:00"),
        "2026-08-07 15:42"
    );
    assert_eq!(fmt_ts("junk"), "junk");
}

#[test]
fn document_xml_becomes_markdown() {
    let md = document_md(
        r#"<document version="2.0"><paragraph><bold>CTF</bold> on <link href="https://x.test">this page</link></paragraph><paragraph>See below.</paragraph></document>"#,
    );
    assert_eq!(md, "**CTF** on [this page](https://x.test)\n\nSee below.");
}

#[test]
fn void_tags_do_not_swallow_content() {
    let md = document_md(
        r#"<document><paragraph>one<break/>two <bold>three</bold></paragraph></document>"#,
    );
    assert!(md.contains("one"), "{md}");
    assert!(md.contains("two"), "{md}");
    assert!(md.contains("**three**"), "{md}");
}

#[test]
fn math_blocks_become_display_latex() {
    let md = document_md(
        r#"<document><paragraph>So starting with</paragraph><math>\left(\begin{matrix}1&amp;0\\0&amp;1\end{matrix}\right)</math><paragraph>using dagger.</paragraph><math/></document>"#,
    );
    assert_eq!(
            md,
            "So starting with\n\n$$\n\\left(\\begin{matrix}1&0\\\\0&1\\end{matrix}\\right)\n$$\n\nusing dagger."
        );
}

#[test]
fn images_lists_and_callouts_render() {
    let md = document_md(
        r#"<document><figure><image src="https://img.test/a.png" width="10"/></figure><list style="number"><list-item><paragraph>first</paragraph></list-item><list-item><paragraph>second</paragraph></list-item></list><callout type="info"><bold>note</bold></callout></document>"#,
    );
    assert!(md.contains("![](https://img.test/a.png)"), "{md}");
    assert!(md.contains("1. first"), "{md}");
    assert!(md.contains("2. second"), "{md}");
    assert!(md.contains("> **note**"), "{md}");
}

/// An oculus-keyd that answers Ed's `/api/user` and records the rest.
fn keyd_for_ed(dir: &Scratch) -> FakeKeyd {
    FakeKeyd::start(dir, |header, _| match header["op"].as_str().unwrap() {
        "forward" => match header["path"].as_str().unwrap() {
            "/api/user" => (
                json!({"status": 200, "headers": []}),
                br#"{"user":{"name":"Ada"},"courses":[]}"#.to_vec(),
            ),
            "/api/renew_token" => (
                json!({"status": 200, "headers": []}),
                br#"{"token":"RENEWED"}"#.to_vec(),
            ),
            _ => (json!({"status": 401, "headers": []}), b"{}".to_vec()),
        },
        "session_status" => (
            json!({"canvas": false, "sso": false, "ed": true,
                   "authenticated": false, "signed_out": false}),
            vec![],
        ),
        _ => (json!({}), vec![]),
    })
}

fn session_puts(keyd: &FakeKeyd) -> Vec<(String, String)> {
    keyd.requests()
        .into_iter()
        .filter(|(h, _)| h["op"] == "session_put")
        .map(|(h, _)| {
            (
                h["kind"].as_str().unwrap().to_string(),
                h["value"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn requests_go_to_the_ed_route_and_carry_no_token() {
    let dir = Scratch::new("ed-route");
    let keyd = keyd_for_ed(&dir);
    let ed = Ed::open(&dir);
    assert_eq!(ed.whoami().unwrap(), "Ada");
    let (header, _) = &keyd.requests()[0];
    assert_eq!(header["secret"], "ed");
    assert_eq!(header["method"], "GET");
    assert_eq!(header["path"], "/api/user");
    assert_eq!(header["headers"], json!([]));
    assert!(!header.to_string().contains("x-token"));
}

#[test]
fn has_session_is_what_keyd_reports() {
    let dir = Scratch::new("ed-status");
    let _keyd = keyd_for_ed(&dir);
    assert!(Ed::open(&dir).has_session());
    assert!(!Ed::open(&Scratch::new("ed-no-keyd")).has_session());
}

#[test]
fn renew_hands_the_fresh_token_to_keyd() {
    let dir = Scratch::new("ed-renew");
    let keyd = keyd_for_ed(&dir);
    Ed::open(&dir).renew();
    let (header, _) = &keyd.requests()[0];
    assert_eq!(header["method"], "POST");
    assert_eq!(header["path"], "/api/renew_token");
    assert_eq!(session_puts(&keyd), [("ed".into(), "RENEWED".into())]);
}

#[test]
fn a_rejected_token_points_at_the_manual_override() {
    let dir = Scratch::new("ed-401");
    let _keyd = FakeKeyd::start(&dir, |_, _| {
        (json!({"status": 401, "headers": []}), b"{}".to_vec())
    });
    let err = Ed::open(&dir).whoami().unwrap_err();
    assert!(err.contains("oculus auth ed <TOKEN>"), "{err}");
}

#[test]
fn no_token_and_no_keyd_are_told_apart() {
    let dir = Scratch::new("ed-missing");
    let _keyd = FakeKeyd::start(&dir, |_, _| {
        (
            json!({"error": "missing", "detail": "no ed session is stored"}),
            vec![],
        )
    });
    let err = Ed::open(&dir).whoami().unwrap_err();
    assert!(err.starts_with("No saved Ed token"), "{err}");

    let err = Ed::open(&Scratch::new("ed-absent")).whoami().unwrap_err();
    assert!(err.contains("not running or not installed"), "{err}");
}

#[test]
fn a_pasted_token_is_checked_directly_and_then_given_to_keyd() {
    let ed = FakeServer::start(|hit| {
        if hit.header("x-token") == Some("good") {
            Reply::json(json!({"user": {"name": "Ada"}}))
        } else {
            Reply::status(401, json!({}))
        }
    });
    let dir = Scratch::new("ed-set");
    let keyd = keyd_for_ed(&dir);
    let broker = Credentialed::at(&dir);

    let err = store_token(&broker, &ed.origin(), "bad").unwrap_err();
    assert!(err.contains("oculus auth ed <TOKEN>"), "{err}");
    assert!(
        session_puts(&keyd).is_empty(),
        "a bad paste is never stored"
    );

    assert_eq!(
        store_token(&broker, &ed.origin(), " good\n").unwrap(),
        "Ada"
    );
    assert_eq!(session_puts(&keyd), [("ed".into(), "good".into())]);
    assert!(keyd.requests().iter().all(|(h, _)| h["op"] != "forward"));
}

#[test]
fn the_login_token_exchange_is_direct_and_never_touches_keyd() {
    let ed = FakeServer::start(|_| Reply::json(json!({"token": "MINTED"})));
    let dir = Scratch::new("ed-login");
    let keyd = keyd_for_ed(&dir);
    assert_eq!(
        exchange_login_token(&ed.origin(), "once").unwrap(),
        "MINTED"
    );
    let hits = ed.hits();
    assert_eq!(hits[0].method, "POST");
    assert_eq!(hits[0].url, "/login_token");
    assert_eq!(hits[0].json(), json!({"login_token": "once"}));
    assert!(keyd.requests().is_empty());
}

#[test]
fn the_launch_reaches_canvas_through_keyd_and_other_hosts_without_its_cookie() {
    let lti = FakeServer::start(|hit| {
        Reply::from((302, Vec::new())).with_header(
            "Location",
            &format!("{}/au/courses/38809?_logintoken=LT", hit.origin),
        )
    });
    let form = format!(
        r#"<form action="{}/lti/launch"><input name="id_token" value="jwt"/></form>"#,
        lti.origin()
    );
    let dir = Scratch::new("ed-lti");
    let keyd = FakeKeyd::start(&dir, move |_, _| {
        (
            json!({"status": 200, "headers": []}),
            form.clone().into_bytes(),
        )
    });
    let canvas = crate::sources::canvas::Canvas::open(&dir);

    let (token, destination) = walk_lti_chain(&canvas, "/courses/5/external_tools/9").unwrap();

    assert_eq!(token, "LT");
    assert_eq!(ed_course_in_url(&destination), Some(38809));
    let requests = keyd.requests();
    assert_eq!(requests.len(), 1, "only the Canvas leg used keyd");
    assert_eq!(requests[0].0["secret"], "canvas");
    assert_eq!(requests[0].0["path"], "/courses/5/external_tools/9");
    let hits = lti.hits();
    assert_eq!(hits[0].method, "POST");
    assert_eq!(hits[0].body, b"id_token=jwt");
    assert!(hits[0].header("cookie").is_none());
}

#[test]
fn a_dead_canvas_session_ends_the_launch_with_the_reason() {
    let dir = Scratch::new("ed-lti-dead");
    let _keyd = FakeKeyd::start(&dir, |_, _| {
        (
            json!({"error": "missing", "detail": "no canvas session is stored",
                   "signin": {"result": "error", "code": "signed_out"}}),
            vec![],
        )
    });
    let canvas = crate::sources::canvas::Canvas::open(&dir);
    let err = walk_lti_chain(&canvas, "/courses/5/external_tools/9").unwrap_err();
    assert!(err.contains("Signed out"), "{err}");
    assert!(canvas.expired().is_some());
}
