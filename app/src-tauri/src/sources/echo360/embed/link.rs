//! The `src` an `<echo-video>` tag carries, checked before Canvas sees it.

use url::Url;

use super::super::auth::html_unescape;

const LTI_HOST: &str = "echo360.net.au";

/// Whether `id` can sit in a URL path or a file name as it is: Echo360's ids
/// are UUIDs.
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// The Canvas retrieve URL that launches `link_id`, rebuilt from `src`. The
/// tag comes from page markdown, so only an external-tool launch on `canvas`
/// (the profile's `https://host`) of an Echo360 LTI link naming that same
/// `link_id` is accepted; anything else never reaches Canvas with the
/// student's session.
pub fn retrieve_url(canvas: &str, src: &str, link_id: &str) -> Result<String, String> {
    let refuse = |why: &str| Err(format!("Not an Echo360 embed link: {why}."));
    if !is_safe_id(link_id) {
        return refuse("the link id is malformed");
    }
    // An attribute value that kept its HTML escaping still names the same URL.
    let src = html_unescape(src);
    let Ok(url) = Url::parse(&src) else {
        return refuse("not a URL");
    };
    let canvas = Url::parse(canvas).map_err(|e| format!("Canvas address unreadable: {e}"))?;
    if url.origin() != canvas.origin() {
        return refuse("not on Canvas");
    }
    let segments: Vec<&str> = url
        .path_segments()
        .map(Iterator::collect)
        .unwrap_or_default();
    let course = match segments.as_slice() {
        ["courses", course, "external_tools", "retrieve"]
            if !course.is_empty() && course.bytes().all(|b| b.is_ascii_digit()) =>
        {
            *course
        }
        _ => return refuse("not a Canvas external-tool launch"),
    };
    let Some(tool) = url
        .query_pairs()
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.into_owned())
    else {
        return refuse("no tool URL");
    };
    let Ok(tool) = Url::parse(&tool) else {
        return refuse("the tool URL is not a URL");
    };
    if tool.scheme() != "https"
        || tool.host_str() != Some(LTI_HOST)
        || !tool.path().starts_with("/lti/")
    {
        return refuse("the tool is not Echo360's LTI");
    }
    let named = tool
        .query_pairs()
        .find(|(k, _)| k == "linkId")
        .map(|(_, v)| v.into_owned());
    if named.as_deref() != Some(link_id) {
        return refuse("it launches a different link");
    }

    let mut out = canvas;
    out.set_path(&format!("/courses/{course}/external_tools/retrieve"));
    out.query_pairs_mut()
        .append_pair("display", "borderless")
        .append_pair("url", tool.as_str());
    Ok(out.to_string())
}
