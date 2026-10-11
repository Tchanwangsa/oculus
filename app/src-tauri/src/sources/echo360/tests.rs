use super::auth::{extract_section_id, launch_form, parse_lti_form, tool_candidates};
use super::syllabus::{duration_between, second_source_hint};
use super::{trim_video, NO_TOOL};

#[test]
fn reads_the_section_id_out_of_a_launch_url() {
    assert_eq!(
        extract_section_id("https://echo360.net.au/section/abc-123/home").unwrap(),
        "abc-123"
    );
    assert!(extract_section_id("https://echo360.net.au/home").is_err());
    assert!(extract_section_id("https://echo360.net.au/section/").is_err());
}

#[test]
fn pulls_hidden_fields_out_of_the_lti_form() {
    let html = r#"<div><form action="https://echo360.net.au/lti" method="POST">
            <input type="hidden" name="oauth_nonce" value="abc&amp;1"/>
            <input type="hidden" name="lti_version" value="LTI-1p0"/>
            <input type="submit" value="Go"/></form></div>"#;
    let (action, fields) = parse_lti_form(html).unwrap();
    assert_eq!(action, "https://echo360.net.au/lti");
    assert_eq!(
        fields,
        vec![
            ("oauth_nonce".to_string(), "abc&1".to_string()),
            ("lti_version".to_string(), "LTI-1p0".to_string()),
        ]
    );
}

fn keyd_serving(
    pages: &'static [(&'static str, u16, &'static str, &'static str)],
) -> (crate::test_support::Scratch, crate::test_support::FakeKeyd) {
    let dir = crate::test_support::Scratch::new("echo360-lti");
    let unimelb = crate::sources::institution::unimelb();
    crate::sources::institution::save(&dir, &unimelb).unwrap();
    let keyd = crate::test_support::FakeKeyd::start(&dir, move |header, _| {
        let path = header["path"].as_str().unwrap_or("");
        let (_, status, location, body) = pages
            .iter()
            .find(|(p, ..)| *p == path)
            .unwrap_or_else(|| panic!("unexpected {path}"));
        let headers = if location.is_empty() {
            serde_json::json!([])
        } else {
            serde_json::json!([["location", location]])
        };
        (
            serde_json::json!({"status": status, "headers": headers}),
            body.as_bytes().to_vec(),
        )
    });
    (dir, keyd)
}

#[test]
fn the_launch_page_comes_through_keyd_and_its_redirects_are_followed_there() {
    const FORM: &str = r#"<form action="https://echo360.net.au/lti"><input type="hidden" name="a" value="1"/></form>"#;
    let (dir, keyd) = keyd_serving(&[
        ("/api/v1/courses/5/tabs", 200, "", "[]"),
        (
            "/courses/5/external_tools/701",
            302,
            "/courses/5/launch",
            "",
        ),
        ("/courses/5/launch", 200, "", FORM),
    ]);
    let canvas = crate::sources::canvas::Canvas::open(&dir);
    let (lti_url, action, fields) = launch_form(&canvas, 5).unwrap();
    assert_eq!(
        lti_url,
        "https://canvas.lms.unimelb.edu.au/courses/5/external_tools/701"
    );
    assert_eq!(action, "https://echo360.net.au/lti");
    assert_eq!(fields, vec![("a".to_string(), "1".to_string())]);
    let requests = keyd.requests();
    assert_eq!(requests.len(), 3);
    for (header, _) in &requests {
        assert_eq!(header["secret"], "canvas");
        assert_eq!(header["headers"], serde_json::json!([]));
    }
}

#[test]
fn a_tab_labelled_for_lecture_capture_comes_before_the_profiles_tool() {
    const FORM: &str = r#"<form action="https://echo360.org/lti"><input type="hidden" name="a" value="1"/></form>"#;
    let (dir, _keyd) = keyd_serving(&[
        (
            "/api/v1/courses/5/tabs",
            200,
            "",
            r#"[{"label": "Echo360", "html_url": "/courses/5/external_tools/55"}]"#,
        ),
        ("/courses/5/external_tools/55", 200, "", FORM),
    ]);
    let canvas = crate::sources::canvas::Canvas::open(&dir);
    let (lti_url, action, _) = launch_form(&canvas, 5).unwrap();
    assert!(lti_url.ends_with("/courses/5/external_tools/55"));
    assert_eq!(action, "https://echo360.org/lti");
}

#[test]
fn a_course_whose_pages_carry_no_form_has_no_tool() {
    let (dir, _keyd) = keyd_serving(&[
        ("/api/v1/courses/5/tabs", 200, "", "[]"),
        (
            "/courses/5/external_tools/701",
            200,
            "",
            "<html>nothing</html>",
        ),
        ("/api/v1/courses/6/tabs", 200, "", "[]"),
        ("/courses/6/external_tools/701", 404, "", "{}"),
    ]);
    let canvas = crate::sources::canvas::Canvas::open(&dir);
    assert_eq!(launch_form(&canvas, 5).unwrap_err(), NO_TOOL);
    assert_eq!(launch_form(&canvas, 6).unwrap_err(), NO_TOOL);
}

#[test]
fn candidates_are_the_tabs_labelled_for_lecture_capture() {
    let tabs = serde_json::json!([
        {"label": "Home", "html_url": "/courses/9"},
        {"label": "Lecture Recordings", "html_url": "/courses/9/external_tools/701"},
        {"label": "Echo360", "html_url": "https://canvas.foo.edu/courses/9/external_tools/55"},
        {"label": "Ed Discussion", "html_url": "/courses/9/external_tools/12"},
        {"label": "Echo360 help", "html_url": "/courses/9/pages/echo"},
    ]);
    assert_eq!(
        tool_candidates(&tabs),
        [
            "/courses/9/external_tools/701",
            "/courses/9/external_tools/55"
        ]
    );
}

#[test]
fn without_a_lead_in_the_download_is_the_file() {
    let dir = crate::test_support::Scratch::new("echo-trim");
    let (raw, out) = (dir.join("raw.mp4"), dir.join("source1.mp4"));
    std::fs::write(&raw, b"video").unwrap();
    // No ffmpeg is run, so a missing one cannot fail it.
    assert!(trim_video(
        std::path::Path::new("/nonexistent/ffmpeg"),
        &raw,
        &out,
        0
    ));
    assert_eq!(std::fs::read(&out).unwrap(), b"video");
    assert!(!raw.exists());
}

#[test]
fn no_echo360_form_means_no_launch() {
    assert!(parse_lti_form("<form action=\"https://elsewhere\"></form>").is_none());
}

#[test]
fn durations_handle_midnight_rollover() {
    assert_eq!(
        duration_between("2026-01-01T10:00:00Z", "2026-01-01T11:30:00Z"),
        5400
    );
    assert_eq!(
        duration_between("2026-01-01T23:30:00Z", "2026-01-02T00:30:00Z"),
        3600
    );
}

#[test]
fn a_camera_stream_is_found_wherever_echo360_nests_it() {
    let with_camera = serde_json::json!({
        "medias": [{ "media": { "current": {
            "primaryFiles": [{ "s3Url": "a" }],
            "secondaryFiles": [{ "s3Url": "b" }],
        }}}]
    });
    assert_eq!(second_source_hint(&with_camera), Some(true));

    let screen_only = serde_json::json!({
        "medias": [{ "media": { "current": {
            "primaryFiles": [{ "s3Url": "a" }],
            "secondaryFiles": [],
        }}}]
    });
    assert_eq!(second_source_hint(&screen_only), Some(false));

    let no_files = serde_json::json!({ "medias": [{ "id": "abc", "isAvailable": true }] });
    assert_eq!(second_source_hint(&no_files), None);
}

#[test]
fn durations_read_every_timestamp_shape_echo360_sends() {
    assert_eq!(
        duration_between("2026-01-01T10:00:00.000Z", "2026-01-01T10:50:00.000Z"),
        3000
    );
    assert_eq!(
        duration_between("2026-01-01T10:00:00+11:00", "2026-01-01T10:50:00+11:00"),
        3000
    );
    assert_eq!(
        duration_between("2026-01-01T10:00:00", "2026-01-01T10:50:00"),
        3000
    );
}
