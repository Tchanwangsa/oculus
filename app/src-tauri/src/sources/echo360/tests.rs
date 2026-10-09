use super::auth::{extract_section_id, parse_lti_form};
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
