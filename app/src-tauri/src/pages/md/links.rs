//! `<a href>` repair, shared by the converter and the link crawl so a repaired
//! Canvas link is both rendered and fetched.

use url::Url;

/// The href a link keeps, or `None` when it is dead and only its text stays.
/// Unwraps Google redirects, repairs a `mailto:` Canvas prefixed with its
/// origin, and resolves a course-copy `$CANVAS_…_REFERENCE$` placeholder
/// through the element's `data-api-endpoint` when that names a file or page.
pub(super) fn normalize_href(href: &str, api_endpoint: Option<&str>) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    if href.contains("$CANVAS_") {
        return api_endpoint
            .filter(|e| !e.contains("$CANVAS_"))
            .and_then(canvas_path);
    }
    if let Some((_, addr)) = href.split_once("/mailto:") {
        return Some(format!("mailto:{addr}"));
    }
    Some(google_target(href).unwrap_or_else(|| href.to_string()))
}

/// `/api/v1/courses/<c>/files/<id>` → `/courses/<c>/files/<id>`, and the same
/// for pages.
fn canvas_path(endpoint: &str) -> Option<String> {
    let (_, rest) = endpoint.split_once("/api/v1/courses/")?;
    let rest = rest.split(['?', '#']).next()?;
    let mut parts = rest.split('/');
    let (course, kind, id) = (parts.next()?, parts.next()?, parts.next()?);
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(course) {
        return None;
    }
    match kind {
        "files" if digits(id) => Some(format!("/courses/{course}/files/{id}")),
        "pages" if !id.is_empty() => Some(format!("/courses/{course}/pages/{id}")),
        _ => None,
    }
}

/// The http(s) target of a `google.<tld>/url?q=…` redirect.
fn google_target(href: &str) -> Option<String> {
    let url = Url::parse(href).ok()?;
    let host = url.host_str()?;
    if !host.trim_start_matches("www.").starts_with("google.") || url.path() != "/url" {
        return None;
    }
    let (_, target) = url
        .query_pairs()
        .find(|(k, v)| (k == "q" || k == "url") && !v.is_empty())?;
    let parsed = Url::parse(&target).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| target.into_owned())
}
