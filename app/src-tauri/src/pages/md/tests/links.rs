use super::super::canvas_links;
use super::super::links::normalize_href;
use super::md;

#[test]
fn hrefs_are_repaired() {
    let n = |h: &str, e: Option<&str>| normalize_href(h, e);
    assert_eq!(
        n("/courses/1/pages/a", None).as_deref(),
        Some("/courses/1/pages/a")
    );
    assert_eq!(
        n(
            "https://www.google.com/url?q=https://example.org/a%3Fb%3D1&sa=D&usg=x",
            None
        )
        .as_deref(),
        Some("https://example.org/a?b=1")
    );
    assert_eq!(
        n("https://google.com.au/url?url=http://x.org/", None).as_deref(),
        Some("http://x.org/")
    );
    // Not a redirect, or a target that isn't a web page: kept as written.
    assert_eq!(
        n("https://www.google.com/search?q=x", None).as_deref(),
        Some("https://www.google.com/search?q=x")
    );
    assert_eq!(
        n("https://www.google.com/url?q=javascript:alert(1)", None).as_deref(),
        Some("https://www.google.com/url?q=javascript:alert(1)")
    );
    assert_eq!(
        n(
            "https://canvas.lms.unimelb.edu.au/mailto:a.b@unimelb.edu.au",
            None
        )
        .as_deref(),
        Some("mailto:a.b@unimelb.edu.au")
    );
    let mimecast = "https://url.au.m.mimecastprotect.com/s/abc?domain=example.org";
    assert_eq!(n(mimecast, None).as_deref(), Some(mimecast));
    assert_eq!(n("  ", None), None);
}

#[test]
fn canvas_placeholders_resolve_through_the_api_endpoint() {
    let n = |e: Option<&str>| normalize_href("$CANVAS_OBJECT_REFERENCE$/files/g9e5", e);
    assert_eq!(
        n(Some(
            "https://canvas.lms.unimelb.edu.au/api/v1/courses/12/files/345"
        ))
        .as_deref(),
        Some("/courses/12/files/345")
    );
    assert_eq!(
        n(Some("/api/v1/courses/12/pages/week-one?x=1")).as_deref(),
        Some("/courses/12/pages/week-one")
    );
    assert_eq!(n(Some("/api/v1/courses/197499/modules/937223")), None);
    assert_eq!(
        n(Some("/api/v1/courses/$CANVAS_COURSE_REFERENCE$/files/1")),
        None
    );
    assert_eq!(n(None), None);
}

#[test]
fn links_render_repaired_or_as_text() {
    assert_eq!(
        md(r#"<p><a href="https://www.google.com/url?q=https://x.org/&amp;sa=D">x</a></p>"#),
        "[x](https://x.org/)"
    );
    assert_eq!(
        md(
            r#"<p><a href="$CANVAS_OBJECT_REFERENCE$/modules/g9e5" data-api-endpoint="https://canvas.lms.unimelb.edu.au/api/v1/courses/197499/modules/937223">Module 1</a> notes</p>"#
        ),
        "Module 1 notes"
    );
    assert_eq!(md("<p><a name='top'>Top</a></p>"), "Top");
}

#[test]
fn the_crawl_follows_repaired_links() {
    let (pages, files) = canvas_links(
        r#"<a href="$CANVAS_COURSE_REFERENCE$/file_ref/g1" data-api-endpoint="https://canvas.lms.unimelb.edu.au/api/v1/courses/42/files/7">f</a>
           <a href="$CANVAS_OBJECT_REFERENCE$/pages/g2" data-api-endpoint="/api/v1/courses/42/pages/intro">p</a>
           <a href="$CANVAS_OBJECT_REFERENCE$/files/g3">dead</a>
           <a href="https://www.google.com/url?q=https://canvas.lms.unimelb.edu.au/courses/42/files/8">g</a>"#,
        42,
    );
    assert_eq!(pages, vec!["intro"]);
    assert_eq!(files, vec!["7", "8"]);
}
