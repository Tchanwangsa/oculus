use std::sync::Mutex;

use crate::sources::canvas::CANCELLED;
use crate::test_support::{FakeServer, Reply};

use super::{answer, refused, rig};
use keyd_core::okta::LoginError;

fn with_length(len: usize) -> String {
    len.to_string()
}

#[test]
fn a_download_streams_through_keyd_to_disk_with_progress() {
    let body = vec![7u8; 300_000];
    let sent = body.clone();
    let r = rig(move |header, _| {
        assert_eq!(header["stream"], true);
        answer(
            200,
            &[
                ("content-type", "video/mp4"),
                ("content-length", &with_length(sent.len())),
            ],
            &sent,
        )
    });
    let seen = Mutex::new(Vec::new());
    let dest = r.dir.join("whole.mp4");
    let n = r
        .canvas
        .download_to(
            "/files/9/download?download_frd=1",
            &dest,
            &|p| seen.lock().unwrap().push(p),
            &|| false,
        )
        .unwrap();
    assert_eq!(n, 300_000);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    assert_eq!(seen.lock().unwrap().last(), Some(&100));
    assert_eq!(r.paths(), vec!["/files/9/download?download_frd=1"]);
}

#[test]
fn a_body_shorter_than_its_content_length_is_refused() {
    let r = rig(|_, _| answer(200, &[("content-length", "100")], b"only this"));
    let err = r
        .canvas
        .download_to("/f", &r.dir.join("short.mp4"), &|_| {}, &|| false)
        .unwrap_err();
    assert!(err.starts_with("download incomplete"), "{err}");
}

#[test]
fn a_cancelled_download_says_so() {
    let r = rig(|_, _| answer(200, &[("content-length", "4")], b"mp4!"));
    let err = r
        .canvas
        .download_to("/f", &r.dir.join("x.mp4"), &|_| {}, &|| true)
        .unwrap_err();
    assert_eq!(err, CANCELLED);
}

#[test]
fn html_where_a_file_should_be_is_refused() {
    let r = rig(|_, _| {
        answer(
            200,
            &[("content-type", "text/html; charset=utf-8")],
            b"<html>",
        )
    });
    let err = r
        .canvas
        .download_to("/f", &r.dir.join("x.pdf"), &|_| {}, &|| false)
        .unwrap_err();
    assert!(err.starts_with("got HTML instead of the file"), "{err}");
}

#[test]
fn an_error_status_is_named() {
    let r = rig(|_, _| answer(404, &[], b"{}"));
    let err = r
        .canvas
        .download_to("/f", &r.dir.join("x.pdf"), &|_| {}, &|| false)
        .unwrap_err();
    assert_eq!(err, "download HTTP 404");
}

#[test]
fn a_download_that_redirects_to_a_file_host_leaves_the_cookie_behind() {
    let body = vec![3u8; 1000];
    let served = body.clone();
    let cdn = FakeServer::start(move |_| Reply::bytes(served.clone()));
    let target = format!("{}/signed/video.mp4?sig=xyz", cdn.origin());
    let r = rig(move |_, _| answer(302, &[("location", &target)], b""));
    let dest = r.dir.join("video.mp4");
    let n = r
        .canvas
        .download_to("/files/9/download", &dest, &|_| {}, &|| false)
        .unwrap();
    assert_eq!(n, 1000);
    assert_eq!(std::fs::read(&dest).unwrap(), body);
    let hits = cdn.hits();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].header("cookie").is_none());
}

#[test]
fn a_download_that_meets_a_dead_session_is_expired_and_latches() {
    let r = rig(|_, _| refused(401, &[], b"{}", LoginError::SignedOut));
    let err = r
        .canvas
        .download_to("/f", &r.dir.join("x.pdf"), &|_| {}, &|| false)
        .unwrap_err();
    assert!(err.contains("Signed out"), "{err}");
    assert!(r.canvas.expired().is_some());
    assert!(!r.dir.join("x.pdf").exists());
}
