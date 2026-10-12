//! `<iframe>` and `<details>`, the two Canvas constructs that carry more than
//! their text. The tags emitted here are the contract the renderer's sanitize
//! schema and `components` read (docs/sync.md).

use url::Url;

use super::blocks::block_md_where;
use super::escape::{escape_attr, escape_html, escape_md};
use super::labels::{embed_label, host_is};
use super::tree::{attr, is_tag, plain_text, squeeze, Ref};
use super::Ctx;
use crate::library::paths::CANVAS_BASE;

/// Iframe titles that name the player rather than what it plays.
const JUNK_TITLES: &[&str] = &[
    "Kaltura Player",
    "YouTube video player",
    "Vimeo video player",
    "embedded content",
];

/// An iframe as markdown, or `None` when it has no http(s) src. A video Oculus
/// can play becomes a tag; anything else a named link. In a table cell
/// everything stays on one line.
pub(super) fn iframe_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    let src = attr(&n, "src")?.trim();
    let url = Url::parse(CANVAS_BASE).ok()?.join(src).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let raw_title = squeeze(attr(&n, "title").unwrap_or_default());
    let title = strip_prefix_ci(&raw_title, "Video player for").unwrap_or(&raw_title);
    let title = useful_title(title, "").then(|| title.to_string());

    if let Some(link_id) = echo_link_id(&url) {
        let attrs = [
            ("link-id", Some(link_id)),
            ("src", Some(url.to_string())),
            ("title", title),
        ];
        return Some(video_tag("echo-video", &attrs, ctx));
    }
    if let Some(file_id) = media_attachment_id(&url) {
        let attrs = [("file-id", Some(file_id)), ("title", title)];
        return Some(video_tag("canvas-video", &attrs, ctx));
    }
    if let Some(v) = web_video(&url) {
        let attrs = [
            ("provider", Some(v.provider.to_string())),
            ("video-id", Some(v.id)),
            ("hash", v.hash),
            ("title", title),
        ];
        return Some(video_tag("web-video", &attrs, ctx));
    }

    let label = embed_label(&url, title.as_deref().unwrap_or_default());
    Some(match title.filter(|t| useful_title(t, &label)) {
        Some(t) => format!("[{label}: {}]({url})", escape_md(&t)),
        None => format!("[{label}]({url})"),
    })
}

fn strip_prefix_ci<'s>(s: &'s str, prefix: &str) -> Option<&'s str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| s[prefix.len()..].trim())
}

fn useful_title(title: &str, label: &str) -> bool {
    !title.is_empty()
        && !title.eq_ignore_ascii_case(label)
        && !JUNK_TITLES.iter().any(|j| title.eq_ignore_ascii_case(j))
}

/// A video tag with its attributes in order, skipping absent ones. CommonMark
/// starts an HTML block only on a line holding the open tag alone; the blank
/// lines around it split any paragraph it sits in.
fn video_tag(name: &str, attrs: &[(&str, Option<String>)], ctx: &Ctx) -> String {
    let attrs: String = attrs
        .iter()
        .filter_map(|(k, v)| Some(format!(" {k}=\"{}\"", escape_attr(v.as_deref()?))))
        .collect();
    let open = format!("<{name}{attrs}>");
    if ctx.in_cell {
        format!("{open}</{name}>")
    } else {
        format!("\n\n{open}\n</{name}>\n\n")
    }
}

/// The Echo360 `linkId` of a Canvas LTI launch
/// (`/courses/<c>/external_tools/retrieve?url=https://echo360.net.au/lti/<tool>?linkId=<id>`).
fn echo_link_id(url: &Url) -> Option<String> {
    if !url.path().ends_with("/external_tools/retrieve") {
        return None;
    }
    let (_, inner) = url.query_pairs().find(|(k, _)| k == "url")?;
    let inner = Url::parse(&inner).ok()?;
    if !host_is(inner.host_str()?, "echo360.net.au") || !inner.path().starts_with("/lti/") {
        return None;
    }
    let (_, id) = inner.query_pairs().find(|(k, _)| k == "linkId")?;
    (!id.is_empty()).then(|| id.into_owned())
}

/// The Canvas file id of a `/media_attachments_iframe/<id>` player: an
/// ordinary file, downloaded on demand like a module video.
fn media_attachment_id(url: &Url) -> Option<String> {
    let mut segs = url.path_segments()?;
    segs.find(|s| *s == "media_attachments_iframe")?;
    let id = segs.next()?;
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())).then(|| id.to_string())
}

struct WebVideo {
    provider: &'static str,
    id: String,
    /// Vimeo's `h`, without which an unlisted video won't play.
    hash: Option<String>,
}

/// A YouTube `/embed/<id>` or Vimeo `player.vimeo.com/video/<id>` player with
/// a well-formed id.
fn web_video(url: &Url) -> Option<WebVideo> {
    let host = url.host_str()?;
    let mut segs = url.path_segments()?;
    let ok = |s: &str, min: usize, max: usize, f: fn(&u8) -> bool| {
        (min..=max).contains(&s.len()) && s.bytes().all(|b| f(&b))
    };
    if host_is(host, "youtube.com") || host_is(host, "youtube-nocookie.com") {
        if segs.next()? != "embed" {
            return None;
        }
        let id = segs.next()?;
        let yt = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-';
        return ok(id, 6, 20, yt).then(|| WebVideo {
            provider: "youtube",
            id: id.to_string(),
            hash: None,
        });
    }
    if host == "player.vimeo.com" {
        if segs.next()? != "video" {
            return None;
        }
        let id = segs.next()?;
        if !ok(id, 1, 20, u8::is_ascii_digit) {
            return None;
        }
        let hash = url
            .query_pairs()
            .find(|(k, _)| k == "h")
            .map(|(_, v)| v.into_owned())
            .filter(|h| ok(h, 1, 40, u8::is_ascii_alphanumeric));
        return Some(WebVideo {
            provider: "vimeo",
            id: id.to_string(),
            hash,
        });
    }
    None
}

/// A native `<details>`: its `<summary>` as the label, the rest as the body.
pub(super) fn details_md(n: Ref<'_>, ctx: &Ctx) -> String {
    let summary = n.children().find(|c| is_tag(c, "summary"));
    let body = block_md_where(n, ctx, |c| Some(c.id()) != summary.map(|s| s.id()));
    details_block(summary.map(|s| plain_text(s, ctx)).as_deref(), &body)
}

/// A `<details>` kept as HTML around markdown. The summary is plain text: it
/// shares its line with the open tag, inside an HTML block, so markdown there
/// would not be parsed. The body needs the blank line after it to be parsed.
pub(super) fn details_block(summary: Option<&str>, body: &str) -> String {
    let mut out = String::from("<details>\n");
    if let Some(s) = summary {
        out.push_str(&format!(
            "<summary>{}</summary>\n",
            escape_html(&squeeze(s))
        ));
    }
    out.push_str(&format!("\n{}\n\n</details>\n\n", body.trim()));
    out
}
