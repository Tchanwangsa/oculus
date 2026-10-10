//! Favicons.
//!
//! WebKit has no public favicon API, so icons are fetched over HTTP by host:
//! `/favicon.ico` first, then the document's `<link rel=icon>`.

use super::*;

pub(super) const FAVICON_MAX: usize = 256 * 1024;
/// Enough of a document to hold its `<head>`.
pub(super) const FAVICON_HTML_MAX: u64 = 512 * 1024;

pub(super) fn favicon_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .user_agent(PAGE_USER_AGENT)
        .build()
}

pub(super) fn ensure_favicon(app: &AppHandle, url: &url::Url) {
    let Some(host) = url.host_str().map(str::to_owned) else {
        return;
    };
    let first = with_state(app, |s| s.favicons_tried.insert(host.clone()));
    if !first {
        return;
    }
    let origin = url.origin().ascii_serialization();
    let page_url = url.to_string();
    let app = app.clone();
    std::thread::spawn(move || {
        let Some((mime, bytes)) = favicon_for(&origin, &page_url) else {
            return;
        };
        let icon = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        );
        app.emit_to(
            EventTarget::webview(MAIN),
            "browser-favicon",
            FaviconFound { host, icon },
        )
        .ok();
    });
}

pub(super) fn favicon_for(origin: &str, page_url: &str) -> Option<(String, Vec<u8>)> {
    if let Some(found) = fetch_icon(&format!("{origin}/favicon.ico")) {
        return Some(found);
    }
    let html = fetch_head(page_url)?;
    let href = declared_icon(&html)?;
    let resolved = url::Url::parse(page_url).ok()?.join(&href).ok()?;
    if !matches!(resolved.scheme(), "http" | "https") {
        return None;
    }
    fetch_icon(resolved.as_str())
}

pub(super) fn fetch_icon(url: &str) -> Option<(String, Vec<u8>)> {
    let response = favicon_agent().get(url).call().ok()?;
    let content_type = response.header("content-type").unwrap_or("").to_lowercase();
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(FAVICON_MAX as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.is_empty() || bytes.len() > FAVICON_MAX {
        return None;
    }
    let mime = sniff_image(&content_type, &bytes)?;
    Some((mime, bytes))
}

pub(super) fn fetch_head(url: &str) -> Option<String> {
    let response = favicon_agent().get(url).call().ok()?;
    let mut body = Vec::new();
    response
        .into_reader()
        .take(FAVICON_HTML_MAX)
        .read_to_end(&mut body)
        .ok()?;
    Some(String::from_utf8_lossy(&body).into_owned())
}

/// Magic numbers before the header: many servers answer `/favicon.ico` with
/// an HTML 404 page under a 200.
pub(super) fn sniff_image(content_type: &str, bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"\x89PNG") {
        return Some("image/png".into());
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif".into());
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg".into());
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("image/webp".into());
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon".into());
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
    if head.contains("<svg") {
        return Some("image/svg+xml".into());
    }
    if head.contains("<html") || head.contains("<!doctype html") {
        return None;
    }
    content_type
        .starts_with("image/")
        .then(|| {
            content_type
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .filter(|mime| !mime.is_empty())
}

/// The largest icon a document declares. `rel~="icon"` is a whole-word match,
/// so `apple-touch-icon` is skipped.
pub(super) fn declared_icon(html: &str) -> Option<String> {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse(r#"link[rel~="icon"]"#).ok()?;
    let mut best: Option<(u32, String)> = None;
    for link in document.select(&selector) {
        let Some(href) = link.value().attr("href").map(str::trim) else {
            continue;
        };
        if href.is_empty() {
            continue;
        }
        // "any" is what an SVG declares.
        let size = match link.value().attr("sizes").map(str::to_lowercase) {
            Some(s) if s.contains("any") => u32::MAX,
            Some(s) => s
                .split_whitespace()
                .filter_map(|pair| pair.split(['x', 'X']).next()?.parse::<u32>().ok())
                .max()
                .unwrap_or(0),
            None => 0,
        };
        if best.as_ref().is_none_or(|(seen, _)| size > *seen) {
            best = Some((size, href.to_string()));
        }
    }
    best.map(|(_, href)| href)
}
