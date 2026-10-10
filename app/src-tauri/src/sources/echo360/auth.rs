//! The LTI launch that mints an Echo360 session from the Canvas one.

use super::Session;

const LTI_TOOL_PATH: &str = "/external_tools/701";

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";

pub fn connect(canvas: &crate::sources::canvas::Canvas, course_id: i64) -> Result<Session, String> {
    let lti_url = format!(
        "{}/courses/{course_id}{LTI_TOOL_PATH}",
        crate::library::paths::CANVAS_BASE
    );
    let (action, fields) = launch_form(canvas, course_id)?;
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

    let mut s = Session {
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

    eprintln!("[oculus] echo360 auth done: section={}", s.section_id);
    Ok(s)
}

/// The form Canvas serves for the Echo360 tool: where it posts and its fields.
/// The page is Canvas's, so it comes through oculus-keyd with the session.
pub(super) fn launch_form(
    canvas: &crate::sources::canvas::Canvas,
    course_id: i64,
) -> Result<(String, Vec<(String, String)>), String> {
    eprintln!("[oculus] echo360 auth: fetching LTI page for course {course_id}");
    let page = canvas
        .get(&format!("/courses/{course_id}{LTI_TOOL_PATH}"))
        .map_err(|e| format!("Canvas LTI page fetch failed: {e}"))?;
    if !page.ok() {
        return Err(format!(
            "Canvas LTI page fetch failed: HTTP {}",
            page.status
        ));
    }
    parse_lti_form(&String::from_utf8_lossy(&page.body)).ok_or_else(|| {
        "Could not parse the Echo360 LTI form — the course may not use Echo360, \
         or the Canvas session has lapsed"
            .to_string()
    })
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

fn html_unescape(s: &str) -> String {
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
