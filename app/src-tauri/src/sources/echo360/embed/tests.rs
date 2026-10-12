use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use url::Url;

use super::super::auth::parse_lti_form;
use super::bearer::{jwt_exp, Bearer};
use super::download::{captions_path, video_share};
use super::launch::parse_player_url;
use super::link::{region_of, retrieve_url};
use super::playlist::{best_variant, single_file, UNSUPPORTED};
use super::properties::{iso_duration, parse, QueryString};
use super::region::{in_region, is_region};
use super::sign::{glob, sign};

const LINK: &str = "943d56e6-0000-4000-8000-000000000001";
const CANVAS: &str = "https://canvas.lms.unimelb.edu.au";
const TOOL: &str = "https://echo360.net.au/lti/9381284d-5253-4e5b-b173-fb47a63576cb";

fn src_for(tool_query: &str) -> String {
    let mut url =
        Url::parse("https://canvas.lms.unimelb.edu.au/courses/253699/external_tools/retrieve")
            .unwrap();
    url.query_pairs_mut()
        .append_pair("display", "borderless")
        .append_pair("url", &format!("{TOOL}?{tool_query}"));
    url.to_string()
}

#[test]
fn a_canvas_launch_of_the_same_echo360_link_is_rebuilt() {
    let src = src_for(&format!("linkId={LINK}&autoplay=false&automute=false"));
    let out = retrieve_url(CANVAS, &src, LINK).unwrap();
    let url = Url::parse(&out).unwrap();
    assert_eq!(url.path(), "/courses/253699/external_tools/retrieve");
    let tool = url
        .query_pairs()
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.into_owned())
        .unwrap();
    assert_eq!(
        tool,
        format!("{TOOL}?linkId={LINK}&autoplay=false&automute=false")
    );
    // An attribute that kept its HTML escaping names the same launch.
    assert_eq!(
        retrieve_url(CANVAS, &src.replace('&', "&amp;"), LINK).unwrap(),
        out
    );
}

#[test]
fn anything_but_that_launch_is_refused() {
    let ok = src_for(&format!("linkId={LINK}"));
    let other_link = src_for("linkId=another-link");
    let evil_tool = ok.replace("echo360.net.au", "evil.example");
    let off_canvas = ok.replace("canvas.lms.unimelb.edu.au", "canvas.example");
    let other_path = ok.replace("external_tools/retrieve", "files/1/download");
    let bad_course = ok.replace("253699", "25x699");
    for (src, link) in [
        (other_link.as_str(), LINK),
        (evil_tool.as_str(), LINK),
        (off_canvas.as_str(), LINK),
        (other_path.as_str(), LINK),
        (bad_course.as_str(), LINK),
        ("not a url", LINK),
        (ok.as_str(), "../etc"),
    ] {
        let err = retrieve_url(CANVAS, src, link).unwrap_err();
        assert!(err.starts_with("Not an Echo360 embed link"), "{src}: {err}");
    }
}

#[test]
fn the_retrieve_page_form_parses_like_the_course_launch() {
    let html = format!(
        r#"<form action="{TOOL}?linkId={LINK}&amp;autoplay=false" method="POST" id="tool_form">
           <input type="hidden" name="oauth_signature" value="a&#39;b"/>
           <input type="hidden" name="resource_link_id" value="r1"/></form>"#
    );
    let (action, fields) = parse_lti_form(&html).unwrap();
    assert_eq!(action, format!("{TOOL}?linkId={LINK}&autoplay=false"));
    assert_eq!(fields[0], ("oauth_signature".into(), "a'b".into()));
}

#[test]
fn the_player_redirect_names_media_secure_link_and_session() {
    let url = Url::parse(
        "https://echo360.net.au/ui/player/11111111-2222?sessionId=abc-123&secureLinkAccessDataId=sl-9&autoplay=false",
    )
    .unwrap();
    let launch = parse_player_url(&url).unwrap();
    assert_eq!(
        (
            launch.media_id.as_str(),
            launch.secure_link_id.as_str(),
            launch.session_id.as_str()
        ),
        ("11111111-2222", "sl-9", "abc-123")
    );
    assert_eq!(launch.base, "https://echo360.net.au");
    let missing = Url::parse("https://echo360.net.au/ui/player/m1?sessionId=s").unwrap();
    assert!(parse_player_url(&missing).is_err());
    let unsafe_id =
        Url::parse("https://echo360.net.au/ui/player/m1?sessionId=s&secureLinkAccessDataId=a%2F..")
            .unwrap();
    assert!(parse_player_url(&unsafe_id).is_err());
}

#[test]
fn a_jwt_exp_is_read_from_its_payload() {
    let payload = URL_SAFE_NO_PAD.encode(br#"{"iat":1000,"exp":47800}"#);
    assert_eq!(jwt_exp(&format!("h.{payload}.sig")).unwrap(), 47800);
    assert_eq!(jwt_exp(&format!("h.{payload}==.sig")).unwrap(), 47800);
    assert!(jwt_exp("not-a-jwt").is_err());
    let no_exp = URL_SAFE_NO_PAD.encode(br#"{"iat":1}"#);
    assert!(jwt_exp(&format!("h.{no_exp}.s")).is_err());
}

#[test]
fn a_bearer_is_reused_until_five_minutes_before_expiry() {
    let b = Bearer {
        base: "https://echo360.net.au".into(),
        jwt: "j".into(),
        exp: 10_000,
    };
    assert!(b.usable_at(10_000 - 301));
    assert!(!b.usable_at(10_000 - 300));
    assert!(!b.usable_at(20_000));
}

#[test]
fn iso_durations_become_seconds() {
    assert_eq!(iso_duration("PT1358.393S"), Some(1358.393));
    assert_eq!(iso_duration("PT1H2M3.5S"), Some(3723.5));
    assert_eq!(iso_duration("P1DT1S"), Some(86_401.0));
    assert_eq!(iso_duration("PT0S"), Some(0.0));
    assert_eq!(iso_duration("P1M"), None, "months have no fixed length");
    assert_eq!(iso_duration("PT"), None);
    assert_eq!(iso_duration("1358"), None);
    assert_eq!(iso_duration("PT12"), None);
}

#[test]
fn player_properties_yield_what_a_download_needs() {
    let json: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/echo360/player-properties.json"
    ))
    .unwrap();
    let info = parse(&json).unwrap();
    let base = "https://content.echo360.net.au/0000.inst/11111111-2222-3333-4444-555555555555";
    assert_eq!(info.name, "WWW_WOTWHYWHO_2024.mp4");
    assert_eq!(info.duration_s, Some(1358.393));
    assert!(!info.restrict_download);
    assert_eq!(
        info.captions_url.as_deref(),
        Some(format!("{base}/1/captions.vtt").as_str())
    );
    assert_eq!(
        info.poster_url.as_deref(),
        Some(format!("{base}/1/poster1.jpg").as_str())
    );
    assert_eq!(
        info.video_master.as_deref(),
        Some(format!("{base}/1/s1_v.m3u8").as_str()),
        "the video-only master over the muxed one"
    );
    assert_eq!(
        info.audio_master.as_deref(),
        Some(format!("{base}/1/s0_a.m3u8").as_str())
    );
    assert_eq!(info.extra_sources, 0);
    assert_eq!(info.query_strings.len(), 1);
}

#[test]
fn properties_tolerate_nulls_string_track_types_and_a_second_camera() {
    let json = serde_json::json!({
        "data": {
            "mediaName": null,
            "copyrightData": null,
            "captions": [{"uri": "https://cdn/c.vtt"}],
            "posterMedia": null,
            "sourceQueryStrings": null,
            "playableAudioVideo": {
                "duration": 61.5,
                "playableMedias": [
                    {"uri": "https://cdn/s2_v.m3u8", "sourceIndex": 2, "trackType": "Video"},
                    {"uri": "https://cdn/s1_av.m3u8", "sourceIndex": 1, "trackType": "Audio+Video"},
                    {"uri": "https://cdn/s0_a.m3u8", "sourceIndex": 0, "trackType": "Audio"},
                    {"uri": "https://cdn/s1.mp4", "sourceIndex": 1, "trackType": "Video"}
                ]
            }
        }
    });
    let info = parse(&json).unwrap();
    assert_eq!(info.name, "");
    assert_eq!(info.duration_s, Some(61.5));
    assert!(!info.restrict_download);
    assert_eq!(info.captions_url.as_deref(), Some("https://cdn/c.vtt"));
    assert_eq!(info.poster_url, None);
    assert_eq!(info.video_master.as_deref(), Some("https://cdn/s1_av.m3u8"));
    assert_eq!(info.audio_master.as_deref(), Some("https://cdn/s0_a.m3u8"));
    assert_eq!(info.extra_sources, 1);
    assert!(info.query_strings.is_empty());

    let restricted = serde_json::json!({"copyrightData": {"restrictDownload": true}});
    assert!(parse(&restricted).unwrap().restrict_download);
}

fn patterns() -> Vec<QueryString> {
    vec![
        QueryString {
            uri_pattern: "https://cdn.example/inst/*".into(),
            query_string: "Policy=wide".into(),
        },
        QueryString {
            uri_pattern: "https://cdn.example/inst/media-1/*".into(),
            query_string: "?Policy=P&Signature=S&Key-Pair-Id=K".into(),
        },
    ]
}

#[test]
fn signing_replaces_the_query_and_never_appends() {
    let signed = sign(
        "https://cdn.example/inst/media-1/1/s1q1.mp4?old=1&x-src=a",
        &patterns(),
    )
    .unwrap();
    assert_eq!(
        signed, "https://cdn.example/inst/media-1/1/s1q1.mp4?Policy=P&Signature=S&Key-Pair-Id=K",
        "the longest matching pattern, its query in place of the old one"
    );
    assert_eq!(
        sign("https://cdn.example/inst/media-2/a.m3u8", &patterns()).unwrap(),
        "https://cdn.example/inst/media-2/a.m3u8?Policy=wide"
    );
    let err = sign("https://other.example/a.m3u8", &patterns()).unwrap_err();
    assert!(
        err.contains("None of Echo360's 2 signing patterns"),
        "{err}"
    );
}

#[test]
fn glob_stars_match_any_run_including_slashes() {
    assert!(glob("https://a/*", "https://a/b/c.mp4"));
    assert!(glob("https://a/*/c.mp4", "https://a/b/x/c.mp4"));
    assert!(glob("*", ""));
    assert!(glob("https://a/b", "https://a/b"));
    assert!(!glob("https://a/b", "https://a/bc"));
    assert!(!glob("https://a/*.vtt", "https://a/b.mp4"));
    assert!(glob("https://a/*q*.mp4", "https://a/s1q1.mp4"));
}

const MASTER: &str = "#EXTM3U
#EXT-X-VERSION:7
#EXT-X-INDEPENDENT-SEGMENTS
#EXT-X-STREAM-INF:BANDWIDTH=2200000,RESOLUTION=1920x1080,CODECS=\"avc1.640028,mp4a.40.2\",AUDIO=\"s0q1\"
s1q1.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=640000,RESOLUTION=640x360,CODECS=\"avc1.4d401e\"
s1q0.m3u8
";

const VARIANT: &str = "#EXTM3U
#EXT-X-VERSION:7
#EXT-X-TARGETDURATION:6
#EXT-X-MAP:URI=\"s1q1.mp4\",BYTERANGE=\"812@0\"
#EXTINF:6.0,
#EXT-X-BYTERANGE:120000@812
s1q1.mp4
#EXTINF:6.0,
#EXT-X-BYTERANGE:118000@120812
s1q1.mp4
#EXT-X-ENDLIST
";

#[test]
fn the_highest_bandwidth_variant_wins_whatever_its_place() {
    assert_eq!(best_variant(MASTER).unwrap().as_deref(), Some("s1q1.m3u8"));
    let reversed = MASTER
        .replace("2200000", "100")
        .replace("640000", "9999999");
    assert_eq!(
        best_variant(&reversed).unwrap().as_deref(),
        Some("s1q0.m3u8")
    );
    assert_eq!(
        best_variant(VARIANT).unwrap(),
        None,
        "already a media playlist"
    );
    assert!(best_variant("#EXTM3U\n")
        .unwrap_err()
        .starts_with(UNSUPPORTED));
}

#[test]
fn a_variant_over_one_file_resolves_to_that_file() {
    let base = Url::parse("https://cdn.example/inst/m/1/s1q1.m3u8?Policy=P").unwrap();
    assert_eq!(
        single_file(VARIANT, &base).unwrap().as_str(),
        "https://cdn.example/inst/m/1/s1q1.mp4"
    );
}

#[test]
fn other_packagings_are_refused() {
    let base = Url::parse("https://cdn.example/inst/m/1/s1q1.m3u8").unwrap();
    let segmented = VARIANT.replacen("s1q1.mp4\n#EXTINF", "seg1.m4s\n#EXTINF", 1);
    let no_map = VARIANT.replace("#EXT-X-MAP:URI=\"s1q1.mp4\",BYTERANGE=\"812@0\"\n", "");
    let ts = "#EXTM3U\n#EXTINF:6.0,\na.ts\n#EXTINF:6.0,\nb.ts\n";
    let no_segments = "#EXTM3U\n#EXT-X-MAP:URI=\"s1q1.mp4\"\n#EXT-X-ENDLIST\n";
    for text in [segmented.as_str(), no_map.as_str(), ts, no_segments] {
        let err = single_file(text, &base).unwrap_err();
        assert!(err.starts_with(UNSUPPORTED), "{err}");
    }
}

#[test]
fn progress_splits_by_bytes_when_both_sizes_are_known() {
    assert_eq!(video_share(Some(90), Some(10)), 0.9);
    assert_eq!(video_share(Some(90), None), 0.8);
    assert_eq!(video_share(None, None), 0.8);
    assert_eq!(video_share(Some(0), Some(0)), 0.8);
}

#[test]
fn captions_sit_beside_the_video_as_name_dot_vtt() {
    assert_eq!(
        captions_path(std::path::Path::new("/d/Week 1.mp4")),
        std::path::Path::new("/d/Week 1.mp4.vtt")
    );
}

#[test]
fn the_tag_md_writes_is_accepted() {
    let src = "https://canvas.lms.unimelb.edu.au/courses/253699/external_tools/retrieve?display=borderless&amp;url=https%3A%2F%2Fecho360.net.au%2Flti%2F9381284d-5253-4e5b-b173-fb47a63576cb%3FlinkId%3Dabc-123%26autoplay%3Dfalse";
    let out = retrieve_url(CANVAS, src, "abc-123").unwrap();
    assert_eq!(
        out,
        retrieve_url(CANVAS, &src.replace("&amp;", "&"), "abc-123").unwrap()
    );
    assert!(out.starts_with("https://canvas.lms.unimelb.edu.au/courses/253699/external_tools/retrieve?display=borderless&url=https%3A%2F%2Fecho360.net.au%2Flti%2F"));
}

#[test]
fn every_echo360_region_is_accepted_and_nothing_posing_as_one() {
    for host in [
        "echo360.org",
        "echo360.org.uk",
        "echo360.net.au",
        "echo360.ca",
    ] {
        assert!(is_region(host), "{host}");
        assert!(in_region(&format!("au.{host}")), "{host}");
    }
    for host in [
        "echo360.evil.com",
        "evil-echo360.org",
        "xecho360.org",
        "echo360.org.evil",
    ] {
        assert!(!is_region(host), "{host}");
        assert!(!in_region(host), "{host}");
    }
    let us = src_for(&format!("linkId={LINK}")).replace("echo360.net.au", "echo360.org");
    let out = retrieve_url(CANVAS, &us, LINK).unwrap();
    assert_eq!(region_of(&out).as_deref(), Some("https://echo360.org"));
}
