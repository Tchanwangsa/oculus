use url::Url;

use super::super::canvas_links;
use super::super::labels::embed_label;
use super::{md, ECHO_SRC};

#[test]
fn echo_iframe_carries_its_title() {
    let tag = |title: &str| {
        let out = md(&format!(
            "<iframe title=\"{title}\" src=\"{ECHO_SRC}\"></iframe>"
        ));
        out.lines().next().unwrap().to_string()
    };
    assert!(
        tag("SWEN30006_Week2  Vid1\n(20:52)")
            .ends_with("autoplay%3Dfalse\" title=\"SWEN30006_Week2 Vid1 (20:52)\">"),
        "{}",
        tag("x")
    );
    assert!(tag("Kaltura Player").ends_with("autoplay%3Dfalse\">"));
    assert!(tag("kaltura player").ends_with("autoplay%3Dfalse\">"));
    assert!(tag(" ").ends_with("autoplay%3Dfalse\">"));
    assert!(tag("a &quot;b&quot; &amp; c").ends_with("title=\"a &quot;b&quot; &amp; c\">"));
}

const MEDIA: &str = r#"<iframe style="width: 480px;" title="Video player for Workshop5.mp4" data-media-type="video" src="https://canvas.lms.unimelb.edu.au/media_attachments_iframe/26446254?embedded=true&amp;type=video" data-media-id="m-3Vb"></iframe>"#;

#[test]
fn canvas_media_iframe_becomes_a_canvas_video_tag() {
    assert_eq!(
        md(&format!("<p>Watch {MEDIA} now</p>")),
        "Watch \n\n<canvas-video file-id=\"26446254\" title=\"Workshop5.mp4\">\n</canvas-video>\n\n now"
    );
    assert_eq!(
        md(
            r#"<iframe title="Video player for " src="/media_attachments_iframe/77?type=video"></iframe>"#
        ),
        "<canvas-video file-id=\"77\">\n</canvas-video>"
    );
    assert_eq!(
        md(&format!("<table><tr><td>a {MEDIA}</td></tr></table>")),
        "| a <canvas-video file-id=\"26446254\" title=\"Workshop5.mp4\"></canvas-video> |\n| --- |"
    );
}

#[test]
fn canvas_media_file_ids_are_not_crawled() {
    let (pages, files) = canvas_links(
        &format!("{MEDIA}<iframe src=\"/courses/42/files/9/preview\"></iframe>"),
        42,
    );
    assert!(pages.is_empty() && files.is_empty(), "{pages:?} {files:?}");
}

#[test]
fn legacy_media_objects_stay_links() {
    assert_eq!(
        md(r#"<iframe title="Video player for m-1" src="/media_objects_iframe/m-1abc?type=video"></iframe>"#),
        "[Canvas media: m-1](https://canvas.lms.unimelb.edu.au/media_objects_iframe/m-1abc?type=video)"
    );
}

#[test]
fn youtube_and_vimeo_become_web_video_tags() {
    assert_eq!(
        md(
            r#"<iframe title="YouTube video player" src="https://www.youtube.com/embed/dQw4w9WgXcQ?si=x&amp;start=3"></iframe>"#
        ),
        "<web-video provider=\"youtube\" video-id=\"dQw4w9WgXcQ\">\n</web-video>"
    );
    assert_eq!(
        md(r#"<iframe title="Bayes in 5 minutes" src="https://www.youtube-nocookie.com/embed/abc_DEF-12"></iframe>"#),
        "<web-video provider=\"youtube\" video-id=\"abc_DEF-12\" title=\"Bayes in 5 minutes\">\n</web-video>"
    );
    assert_eq!(
        md(
            r#"<iframe title="embedded content" src="https://player.vimeo.com/video/123456?h=9f8e7d&amp;badge=0"></iframe>"#
        ),
        "<web-video provider=\"vimeo\" video-id=\"123456\" hash=\"9f8e7d\">\n</web-video>"
    );
    assert_eq!(
        md("<table><tr><td><iframe src=\"https://player.vimeo.com/video/1\"></iframe></td></tr></table>"),
        "| <web-video provider=\"vimeo\" video-id=\"1\"></web-video> |\n| --- |"
    );
}

#[test]
fn malformed_video_ids_fall_back_to_links() {
    assert_eq!(
        md(r#"<iframe src="https://www.youtube.com/embed/abc"></iframe>"#),
        "[YouTube](https://www.youtube.com/embed/abc)"
    );
    assert_eq!(
        md(r#"<iframe title="Course showcase" src="https://vimeo.com/showcase/77"></iframe>"#),
        "[Vimeo: Course showcase](https://vimeo.com/showcase/77)"
    );
    assert_eq!(
        md(r#"<iframe src="https://player.vimeo.com/video/abc"></iframe>"#),
        "[Vimeo](https://player.vimeo.com/video/abc)"
    );
}

#[test]
fn other_embeds_are_named_links() {
    assert_eq!(
        md(
            r#"<iframe class="lti-embed" title="Conceptual model in *Chen* notation" src="/courses/1/external_tools/retrieve?display=borderless&amp;url=https%3A%2F%2Funimelb.h5p.com%2Fcontent%2F129"></iframe>"#
        ),
        r"[H5P: Conceptual model in \*Chen\* notation](https://canvas.lms.unimelb.edu.au/courses/1/external_tools/retrieve?display=borderless&url=https%3A%2F%2Funimelb.h5p.com%2Fcontent%2F129)"
    );
    assert_eq!(
        md(r#"<iframe title="padlet" src="https://padlet.com/embed/x"></iframe>"#),
        "[Padlet](https://padlet.com/embed/x)"
    );
    assert_eq!(
        md(
            r#"<iframe title="Kaltura Player" src="https://www.ourworldindata.org/grapher/x"></iframe>"#
        ),
        "[ourworldindata.org](https://www.ourworldindata.org/grapher/x)"
    );
}

#[test]
fn embed_labels_by_provider() {
    let label = |src: &str, title: &str| embed_label(&Url::parse(src).unwrap(), title);
    let retrieve = "https://canvas.lms.unimelb.edu.au/courses/1/external_tools/retrieve";
    let cases = [
        (
            format!("{retrieve}?url=https%3A%2F%2Funimelb.h5p.com%2Fcontent%2F1"),
            "",
            "H5P",
        ),
        (
            format!("{retrieve}?url=https%3A%2F%2F112.kaf.ap.kaltura.com%2Fbrowseandembed"),
            "",
            "Kaltura",
        ),
        (
            format!("{retrieve}?url=https%3A%2F%2Fexample.org%2F"),
            "",
            "Canvas tool",
        ),
        (
            format!("{retrieve}?display=in_rce&resource_link_lookup_uuid=ab-12"),
            "ERES_RL_60207",
            "Readings Online",
        ),
        (
            format!("{retrieve}?resource_link_lookup_uuid=ab-12"),
            "Quiz",
            "Canvas tool",
        ),
        ("https://curio.h5p.com/c/1".into(), "", "H5P"),
        ("https://padlet.com/embed/x".into(), "", "Padlet"),
        ("https://unimelb.padlet.org/x".into(), "", "Padlet"),
        (
            "https://melbourneuni.au1.qualtrics.com/jfe/form/1".into(),
            "",
            "Survey",
        ),
        (
            "https://q.surveys.unimelb.edu.au/jfe/form/1".into(),
            "",
            "Survey",
        ),
        (
            "https://docs.google.com/forms/d/e/1/viewform".into(),
            "",
            "Google Form",
        ),
        (
            "https://docs.google.com/document/d/1".into(),
            "",
            "Google Docs",
        ),
        ("https://www.mentimeter.com/app/1".into(), "", "Mentimeter"),
        ("https://www.menti.com/1".into(), "", "Mentimeter"),
        (
            "https://unimelbcloud.sharepoint.com/x/embed.aspx".into(),
            "",
            "SharePoint",
        ),
        (
            "https://api.ap.kaltura.com/p/112/embedIframeJs".into(),
            "",
            "Kaltura",
        ),
        (
            "https://echo360.net.au/media/abc/public".into(),
            "",
            "Echo360",
        ),
        ("https://echo360.net.au/other".into(), "", "echo360.net.au"),
        (
            "https://canvas.lms.unimelb.edu.au/media_objects_iframe/m-1".into(),
            "",
            "Canvas media",
        ),
        ("https://www.xkcd.com/1/".into(), "", "xkcd.com"),
        ("https://notkaltura.com/".into(), "", "notkaltura.com"),
    ];
    for (src, title, want) in cases {
        assert_eq!(label(&src, title), want, "{src}");
    }
}
