//! The LTI launch that mints an Echo360 session from the Canvas one. The
//! course's own tabs name the tool, and the launch lands on the university's
//! Echo360 region (`echo360.org`, `echo360.net.au`, …), which every later
//! call uses.

use super::{Session, NO_TOOL};

pub(super) const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub fn connect(canvas: &crate::sources::canvas::Canvas, course_id: i64) -> Result<Session, String> {
    let (lti_url, action, fields) = launch_form(canvas, course_id)?;
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(&fields)
        .finish();

    // The session cookies are spread across the redirect chain; the agent jars them.
    let agent = ureq::AgentBuilder::new().build();
    let resp = agent
        .post(&action)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Referer", &lti_url)
        .set("User-Agent", UA)
        .send_string(&body)
        .map_err(|e| format!("Echo360 LTI POST failed: {e}"))?;

    let section_id = extract_section_id(resp.get_url())?;
    let base = url::Url::parse(resp.get_url())
        .map(|u| u.origin().ascii_serialization())
        .map_err(|e| format!("Echo360 landed somewhere unreadable: {e}"))?;

    let mut s = Session {
        base,
        jwt: String::new(),
        play_session: String::new(),
        cf_key_pair_id: String::new(),
        cf_policy: String::new(),
        cf_signature: String::new(),
        cf_tracking: String::new(),
        section_id,
    };
    for cookie in agent.cookie_store().iter_unexpired() {
        match cookie.name() {
            "ECHO_JWT" => s.jwt = cookie.value().to_string(),
            "PLAY_SESSION" => s.play_session = cookie.value().to_string(),
            "CloudFront-Key-Pair-Id" => s.cf_key_pair_id = cookie.value().to_string(),
            "CloudFront-Policy" => s.cf_policy = cookie.value().to_string(),
            "CloudFront-Signature" => s.cf_signature = cookie.value().to_string(),
            "CloudFront-Tracking2" => s.cf_tracking = cookie.value().to_string(),
            _ => {}
        }
    }
    if s.jwt.is_empty() {
        return Err(
            "Echo360 auth failed — no ECHO_JWT cookie; the LTI POST was rejected".to_string(),
        );
    }

    elog!(
        "[oculus] echo360 auth done: {} section={}",
        s.base,
        s.section_id
    );
    Ok(s)
}

/// The launch page's URL, where its form posts, and the form's fields.
pub(super) type LaunchForm = (String, String, Vec<(String, String)>);

/// The course's Echo360 tool form. The tabs labelled for lecture capture come
/// first, then the profile's fallback tool (a tab can be hidden); the first
/// launch page carrying an Echo360 form wins, and [`NO_TOOL`] when none does;
/// a launch page Canvas fails to serve is an error. The pages are Canvas's,
/// so they come through oculus-keyd with the session.
pub(super) fn launch_form(
    canvas: &crate::sources::canvas::Canvas,
    course_id: i64,
) -> Result<LaunchForm, String> {
    let mut candidates =
        tool_candidates(&canvas.get_json(&format!("/api/v1/courses/{course_id}/tabs"))?);
    if let Some(tool) = canvas.institution().and_then(|i| i.echo360_tool.as_deref()) {
        let path = format!("/courses/{course_id}{tool}");
        if !candidates.contains(&path) {
            candidates.push(path);
        }
    }
    for path in &candidates {
        elog!("[oculus] echo360 auth: trying {path}");
        let page = canvas.get(path)?;
        // A tool this course or student cannot launch is not a tool; any other
        // failure is Canvas's, and surfaces rather than reading as "no lectures".
        if matches!(page.status, 401 | 403 | 404) {
            continue;
        }
        if !page.ok() {
            return Err(format!("Canvas LTI page {path}: HTTP {}", page.status));
        }
        if let Some((action, fields)) = parse_lti_form(&String::from_utf8_lossy(&page.body)) {
            return Ok((format!("{}{path}", canvas.base()?), action, fields));
        }
    }
    Err(NO_TOOL.to_string())
}

/// Course tabs that launch an external tool and are labelled for lecture
/// capture, as Canvas-relative paths.
pub(super) fn tool_candidates(tabs: &serde_json::Value) -> Vec<String> {
    tabs.as_array()
        .into_iter()
        .flatten()
        .filter(|t| {
            let label = t["label"].as_str().unwrap_or("").to_lowercase();
            label.contains("echo")
                || label.contains("lecture recording")
                || label.contains("lecture capture")
        })
        .filter_map(|t| t["html_url"].as_str())
        .filter(|u| u.contains("/external_tools/"))
        .map(|u| match url::Url::parse(u) {
            // `html_url` is usually relative; an absolute one keeps its path.
            Ok(abs) => abs.path().to_string(),
            Err(_) => u.to_string(),
        })
        .collect()
}

pub(super) fn parse_lti_form(html: &str) -> Option<(String, Vec<(String, String)>)> {
    let marker = html.find("action=\"https://echo360")?;
    let form_start = html[..marker].rfind('<')?;
    let chunk = &html[form_start..];

    let a_start = chunk.find("action=\"")? + 8;
    let a_end = a_start + chunk[a_start..].find('"')?;
    let action = html_unescape(&chunk[a_start..a_end]);

    let form_end = chunk.find("</form>").unwrap_or(chunk.len());
    let body = &chunk[..form_end];

    let mut fields = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest.find("<input") {
        rest = &rest[pos + 6..];
        let end = rest.find('>').unwrap_or(rest.len());
        let elem = &rest[..end];
        if elem.contains("type=\"hidden\"") {
            if let (Some(n), Some(v)) = (attr(elem, "name"), attr(elem, "value")) {
                fields.push((n, v));
            }
        }
    }
    (!fields.is_empty()).then_some((action, fields))
}

fn attr(elem: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let s = elem.find(&pat)? + pat.len();
    let e = s + elem[s..].find('"')?;
    Some(html_unescape(&elem[s..e]))
}

pub(super) fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

pub(super) fn extract_section_id(path: &str) -> Result<String, String> {
    let segs: Vec<&str> = path.split('/').collect();
    let idx = segs
        .iter()
        .position(|&s| s == "section")
        .ok_or_else(|| format!("No 'section' segment in: {path}"))?;
    segs.get(idx + 1)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Empty sectionId in: {path}"))
}
