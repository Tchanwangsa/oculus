//! `<iframe>` and `<details>`, the two Canvas constructs that carry more than
//! their text. The tags emitted here are the contract the renderer's sanitize
//! schema and `components` read (docs/sync.md).

use url::Url;

use super::blocks::block_md_where;
use super::escape::{escape_attr, escape_html};
use super::tree::{attr, is_tag, text_content, Ref};
use super::Ctx;

/// An iframe as markdown, or `None` when it has no http(s) src. In a table
/// cell everything stays on one line.
pub(super) fn iframe_md(n: Ref<'_>, ctx: &Ctx) -> Option<String> {
    let src = attr(&n, "src")?.trim();
    let url = Url::options().base_url(ctx.canvas).parse(src).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let Some(link_id) = echo_link_id(&url) else {
        let host = url.host_str()?;
        return Some(format!("[Embedded: {host}]({url})"));
    };
    let open = format!(
        "<echo-video link-id=\"{}\" src=\"{}\">",
        escape_attr(&link_id),
        escape_attr(url.as_str())
    );
    // CommonMark starts an HTML block only on a line holding the open tag
    // alone; the blank lines around it split any paragraph it sits in.
    Some(if ctx.in_cell {
        format!("{open}</echo-video>")
    } else {
        format!("\n\n{open}\n</echo-video>\n\n")
    })
}

/// The Echo360 `linkId` of a Canvas LTI launch
/// (`/courses/<c>/external_tools/retrieve?url=https://echo360.net.au/lti/<tool>?linkId=<id>`).
fn echo_link_id(url: &Url) -> Option<String> {
    if !url.path().ends_with("/external_tools/retrieve") {
        return None;
    }
    let (_, inner) = url.query_pairs().find(|(k, _)| k == "url")?;
    let inner = Url::parse(&inner).ok()?;
    let host = inner.host_str()?;
    if !(host == "echo360.net.au" || host.ends_with(".echo360.net.au"))
        || !inner.path().starts_with("/lti/")
    {
        return None;
    }
    let (_, id) = inner.query_pairs().find(|(k, _)| k == "linkId")?;
    (!id.is_empty()).then(|| id.into_owned())
}

/// A `<details>` kept as HTML around markdown. The summary is plain text: it
/// shares its line with the open tag, inside an HTML block, so markdown there
/// would not be parsed. The body needs the blank line after it to be parsed.
pub(super) fn details_md(n: Ref<'_>, ctx: &Ctx) -> String {
    let summary = n.children().find(|c| is_tag(c, "summary"));
    let mut out = String::from("<details>\n");
    if let Some(s) = summary {
        let text = text_content(s)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!("<summary>{}</summary>\n", escape_html(&text)));
    }
    let body = block_md_where(n, ctx, |c| Some(c.id()) != summary.map(|s| s.id()));
    out.push_str(&format!("\n{}\n\n</details>\n\n", body.trim()));
    out
}
