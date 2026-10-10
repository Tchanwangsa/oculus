use serde_json::json;

use super::{answer, rig, CANVAS};

#[test]
fn a_path_and_query_go_to_keyd_as_given_and_never_carry_a_credential() {
    let r = rig(|_, _| answer(200, &[], b"{}"));
    r.canvas
        .get("/api/v1/courses?per_page=100&include[]=term&include[]=account")
        .unwrap();
    let (path, header) = &r.forwards()[0];
    assert_eq!(
        path,
        "/api/v1/courses?per_page=100&include[]=term&include[]=account"
    );
    assert_eq!(header["secret"], "canvas");
    assert_eq!(header["method"], "GET");
    assert_eq!(header["headers"], json!([]));
    assert!(!header.to_string().to_lowercase().contains("cookie"));
}

#[test]
fn an_absolute_canvas_url_becomes_its_path_and_query_with_escapes_kept() {
    let r = rig(|_, _| answer(200, &[], b"[]"));
    r.canvas
        .get(&format!(
            "{CANVAS}/api/v1/courses/7/pages?include%5B%5D=body&per_page=50&page=2#frag"
        ))
        .unwrap();
    assert_eq!(
        r.paths(),
        vec!["/api/v1/courses/7/pages?include%5B%5D=body&per_page=50&page=2"]
    );
}

#[test]
fn pagination_follows_link_next_through_keyd() {
    let r = rig(|header, _| match header["path"].as_str().unwrap() {
        "/api/v1/things?per_page=2" => answer(
            200,
            &[(
                "link",
                &format!(
                    "<{CANVAS}/api/v1/things?page=2&per_page=2&include%5B%5D=x>; rel=\"next\", \
                     <{CANVAS}/api/v1/things?page=9>; rel=\"last\""
                ),
            )],
            b"[1,2]",
        ),
        "/api/v1/things?page=2&per_page=2&include%5B%5D=x" => answer(200, &[], b"[3]"),
        other => panic!("unexpected {other}"),
    });
    let all = r.canvas.get_all("/api/v1/things?per_page=2").unwrap();
    assert_eq!(all, vec![json!(1), json!(2), json!(3)]);
    assert_eq!(r.paths().len(), 2);
}

#[test]
fn a_forbidden_page_stops_the_walk_with_what_it_has() {
    let r = rig(|header, _| match header["path"].as_str().unwrap() {
        "/a" => answer(
            200,
            &[("link", &format!("<{CANVAS}/b>; rel=\"next\""))],
            b"[1]",
        ),
        _ => answer(403, &[], b"{}"),
    });
    assert_eq!(r.canvas.get_all("/a").unwrap(), vec![json!(1)]);
}

#[test]
fn a_server_error_is_retried_and_then_an_answer_not_an_expired_session() {
    let r = rig(|_, _| answer(503, &[], b"down"));
    let res = r.canvas.get("/api/v1/users/self").unwrap();
    assert_eq!(res.status, 503);
    assert_eq!(r.paths().len(), 3, "one try and two retries");
    assert!(r.canvas.expired().is_none());
}

#[test]
fn a_canvas_outage_reads_as_unreachable_not_as_a_rejected_session() {
    use crate::canvas::SessionProbe;
    let r = rig(|_, _| answer(503, &[], b"down"));
    match r.canvas.probe() {
        SessionProbe::Unreachable(why) => assert_eq!(why, "Canvas returned HTTP 503"),
        _ => panic!("a 503 must not be Rejected"),
    }
}

#[test]
fn keyd_failing_to_reach_canvas_is_retried_then_unreachable() {
    use crate::canvas::{CanvasError, SessionProbe};
    let r = rig(|_, _| {
        (
            json!({"error": "upstream", "detail": "connect timed out"}),
            vec![],
        )
    });
    let err = r.canvas.get("/x").unwrap_err();
    assert_eq!(err, CanvasError::Unreachable("connect timed out".into()));
    assert_eq!(r.paths().len(), 3);
    assert!(matches!(r.canvas.probe(), SessionProbe::Unreachable(_)));
    assert!(r.canvas.expired().is_none());
}

#[test]
fn a_valid_session_probes_as_the_users_name() {
    use crate::canvas::SessionProbe;
    let r = rig(|_, _| answer(200, &[], br#"{"name":"Ada Lovelace"}"#));
    match r.canvas.probe() {
        SessionProbe::Valid(name) => assert_eq!(name, "Ada Lovelace"),
        _ => panic!("expected a valid session"),
    }
}

#[test]
fn a_missing_keyd_is_said_plainly_and_is_never_an_expired_session() {
    use crate::canvas::{CanvasError, SessionProbe};
    let dir = crate::test_support::Scratch::new("canvas-absent");
    let canvas = crate::canvas::Canvas::open(&dir);
    assert_eq!(canvas.get("/x").unwrap_err(), CanvasError::KeydAbsent);
    assert_eq!(canvas.check_keyd().unwrap_err(), CanvasError::KeydAbsent);
    assert!(!canvas.has_session());
    match canvas.probe() {
        SessionProbe::Unreachable(why) => {
            assert!(why.contains("not running or not installed"), "{why}")
        }
        _ => panic!("an absent keyd must not read as a rejected session"),
    }
    assert!(canvas.expired().is_none());
}

#[test]
fn json_that_is_not_json_is_a_failure_with_the_reason() {
    let r = rig(|_, _| answer(200, &[], b"<html>"));
    assert!(r
        .canvas
        .get_json("/x")
        .unwrap_err()
        .to_string()
        .starts_with("bad JSON"));
}

#[test]
fn a_status_the_caller_wanted_as_an_error_names_it() {
    let r = rig(|_, _| answer(404, &[], b"{}"));
    assert_eq!(
        r.canvas.get_json("/api/v1/x").unwrap_err().to_string(),
        "HTTP 404 for /api/v1/x"
    );
}
