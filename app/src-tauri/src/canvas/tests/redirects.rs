use crate::canvas::CanvasError;
use crate::test_support::{FakeServer, Reply};

use super::{answer, rig, CANVAS};

#[test]
fn a_redirect_within_canvas_goes_back_through_keyd() {
    let r = rig(|header, _| match header["path"].as_str().unwrap() {
        "/files/9/download" => answer(
            302,
            &[("location", "/files/9/download?download_frd=1&verifier=v")],
            b"",
        ),
        "/files/9/download?download_frd=1&verifier=v" => answer(200, &[], b"the file"),
        other => panic!("unexpected {other}"),
    });
    let res = r.canvas.get("/files/9/download").unwrap();
    assert_eq!(res.body, b"the file");
    assert_eq!(r.forwards().len(), 2);
}

#[test]
fn a_redirect_off_canvas_is_fetched_directly_and_carries_no_cookie() {
    let cdn = FakeServer::start(|_| Reply::bytes(b"from the cdn".to_vec()));
    let target = format!("{}/signed/file?token=abc", cdn.origin());
    let r = rig(move |_, _| answer(302, &[("location", &target)], b""));
    let res = r.canvas.get("/files/9/download").unwrap();
    assert_eq!(res.body, b"from the cdn");
    assert_eq!(r.forwards().len(), 1, "only the Canvas hop used keyd");
    let hits = cdn.hits();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].url, "/signed/file?token=abc");
    for forbidden in ["cookie", "authorization", "x-token"] {
        assert!(hits[0].header(forbidden).is_none(), "{forbidden} leaked");
    }
}

#[test]
fn a_direct_hop_may_redirect_back_to_canvas_and_goes_through_keyd_again() {
    let back = format!("{CANVAS}/files/9/final");
    let cdn =
        FakeServer::start(move |_| Reply::from((302, Vec::new())).with_header("Location", &back));
    let target = format!("{}/hop", cdn.origin());
    let r = rig(move |header, _| match header["path"].as_str().unwrap() {
        "/start" => answer(302, &[("location", &target)], b""),
        "/files/9/final" => answer(200, &[], b"done"),
        other => panic!("unexpected {other}"),
    });
    assert_eq!(r.canvas.get("/start").unwrap().body, b"done");
    assert_eq!(r.paths(), vec!["/start", "/files/9/final"]);
}

#[test]
fn more_than_five_redirects_is_an_error() {
    let r = rig(|_, _| answer(302, &[("location", "/again")], b""));
    let err = r.canvas.get("/start").unwrap_err();
    assert!(matches!(err, CanvasError::Failed(_)), "{err:?}");
    assert_eq!(r.forwards().len(), 6, "the request and five redirects");
    assert!(r.canvas.expired().is_none());
}

#[test]
fn a_redirect_without_a_location_is_just_the_answer() {
    let r = rig(|_, _| answer(302, &[], b"moved"));
    assert_eq!(r.canvas.get("/x").unwrap().status, 302);
}
