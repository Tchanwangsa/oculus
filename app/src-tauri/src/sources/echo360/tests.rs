use super::auth::{extract_section_id, launch_form, parse_lti_form};
use super::syllabus::{duration_between, second_source_hint};

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
        (
            "/courses/5/external_tools/701",
            302,
            "/courses/5/launch",
            "",
        ),
        ("/courses/5/launch", 200, "", FORM),
    ]);
    let canvas = crate::sources::canvas::Canvas::open(&dir);
    let (action, fields) = launch_form(&canvas, 5).unwrap();
    assert_eq!(action, "https://echo360.net.au/lti");
    assert_eq!(fields, vec![("a".to_string(), "1".to_string())]);
    let requests = keyd.requests();
    assert_eq!(requests.len(), 2);
    for (header, _) in &requests {
        assert_eq!(header["secret"], "canvas");
        assert_eq!(header["headers"], serde_json::json!([]));
    }
}

#[test]
fn a_launch_page_without_the_form_or_with_an_error_says_what_failed() {
    let (dir, _keyd) = keyd_serving(&[
        (
            "/courses/5/external_tools/701",
            200,
            "",
            "<html>nothing</html>",
        ),
        ("/courses/6/external_tools/701", 404, "", "{}"),
    ]);
    let canvas = crate::sources::canvas::Canvas::open(&dir);
    assert!(launch_form(&canvas, 5)
        .unwrap_err()
        .starts_with("Could not parse the Echo360 LTI form"));
    assert_eq!(
        launch_form(&canvas, 6).unwrap_err(),
        "Canvas LTI page fetch failed: HTTP 404"
    );
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
