//! Minting an Ed session from the Canvas session by walking the LTI 1.3
//! launch like a browser.

use super::client::keyd_failure;
use super::{Ed, ED_BASE, TIMEOUT};
use scraper::Html;
use std::collections::HashMap;

use keyd_core::client::SessionKind;

impl Ed {
    /// The Ed course backing a Canvas course. Ed only enrols an account when a
    /// board is first opened, so a miss in `/api/user` falls back to the LTI
    /// launch, which enrols and names the course. `Ok(None)`: no Ed tool.
    pub fn resolve_course(
        &self,
        canvas: &crate::sources::canvas::Canvas,
        canvas_course_id: i64,
        canvas_code: &str,
    ) -> Result<Option<i64>, String> {
        if self.has_session() {
            if let Ok(Some(id)) = self.course_for(canvas_code) {
                return Ok(Some(id));
            }
        }
        match self.connect_via_canvas(canvas, canvas_course_id) {
            Ok(course_id) => Ok(Some(course_id)),
            Err(e) if e.contains("no Ed Discussion tool") => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Mint an Ed session by walking the LTI 1.3 launch like a browser; the
    /// final redirect carries a one-shot `_logintoken` that `login_token`
    /// exchanges for the x-token. Returns the Ed course id it landed on.
    pub fn connect_via_canvas(
        &self,
        canvas: &crate::sources::canvas::Canvas,
        canvas_course_id: i64,
    ) -> Result<i64, String> {
        let tool_path = find_ed_tool(canvas, canvas_course_id)?;
        let (login_token, destination) = walk_lti_chain(canvas, &tool_path)?;

        let token = exchange_login_token(ED_BASE, &login_token)?;
        self.keyd
            .session_put(SessionKind::Ed, &token)
            .map_err(keyd_failure)?;
        // The old session's enrolment list must not outlive it.
        *self.courses.lock().unwrap() = None;

        ed_course_in_url(&destination)
            .ok_or_else(|| format!("launch landed somewhere unexpected: {destination}"))
    }
}

/// Ed's one unauthenticated request: the one-shot `_logintoken` from the LTI
/// launch for the x-token. It has no session to attach, so it is not sent
/// through oculus-keyd.
pub(super) fn exchange_login_token(base: &str, login_token: &str) -> Result<String, String> {
    let resp = ureq::post(&format!("{base}/login_token"))
        .timeout(TIMEOUT)
        .set("Content-Type", "application/json")
        .send_string(&serde_json::json!({ "login_token": login_token }).to_string())
        .map_err(|e| format!("login_token exchange failed: {e}"))?;
    let body: serde_json::Value = resp
        .into_string()
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or("login_token exchange returned no JSON")?;
    body["token"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "login_token exchange returned no token".to_string())
}

/// The course's "Ed Discussion" tool path — tool ids differ per sub-account.
fn find_ed_tool(canvas: &crate::sources::canvas::Canvas, course_id: i64) -> Result<String, String> {
    let tabs = canvas.get_json(&format!("/api/v1/courses/{course_id}/tabs"))?;
    tabs.as_array()
        .into_iter()
        .flatten()
        .find(|t| {
            t["label"]
                .as_str()
                .is_some_and(|l| l.contains("Ed Discussion"))
                && t["html_url"]
                    .as_str()
                    .is_some_and(|u| u.contains("/external_tools/"))
        })
        .and_then(|t| t["html_url"].as_str().map(str::to_string))
        .ok_or_else(|| "course has no Ed Discussion tool".to_string())
}

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";

/// `https://edstem.org/au/courses/38809?_logintoken=…` → `38809`.
pub(super) fn ed_course_in_url(u: &url::Url) -> Option<i64> {
    let mut segments = u.path_segments()?;
    segments.find(|s| *s == "courses")?;
    segments.next()?.parse().ok()
}

/// One leg of the launch: what the host answered.
struct Leg {
    status: u16,
    location: Option<String>,
    body: String,
}

/// Follow redirects and auto-submit forms until a redirect carries
/// `_logintoken`; returns it and the Ed course URL. Canvas legs go through
/// oculus-keyd, which attaches the session; the other hosts' cookies are
/// jarred here per host and never go anywhere else.
pub(super) fn walk_lti_chain(
    canvas: &crate::sources::canvas::Canvas,
    tool_path: &str,
) -> Result<(String, url::Url), String> {
    let mut jar: HashMap<String, HashMap<String, String>> = HashMap::new();
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut url = crate::sources::canvas::resolve(tool_path)?;
    let mut form: Option<String> = None;

    for _ in 0..12 {
        let sent = form.take();
        let leg = if crate::sources::canvas::is_canvas(&url) {
            let hop = match &sent {
                Some(body) => canvas.hop(
                    "POST",
                    &url,
                    &[("content-type", "application/x-www-form-urlencoded")],
                    body.as_bytes(),
                ),
                None => canvas.hop("GET", &url, &[], b""),
            }?;
            Leg {
                status: hop.status,
                location: hop.header("location").map(str::to_string),
                body: String::from_utf8_lossy(&hop.body).into_owned(),
            }
        } else {
            let host = url.host_str().unwrap_or("").to_string();
            let mut req = match &sent {
                Some(_) => agent
                    .post(url.as_str())
                    .set("Content-Type", "application/x-www-form-urlencoded"),
                None => agent.get(url.as_str()),
            }
            .timeout(TIMEOUT)
            .set("User-Agent", UA);
            if let Some(cookies) = jar.get(&host).filter(|c| !c.is_empty()) {
                let header = cookies
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; ");
                req = req.set("Cookie", &header);
            }
            let resp = match match sent {
                Some(body) => req.send_string(&body),
                None => req.call(),
            } {
                Ok(r) => r,
                // 4xx/5xx mid-chain is an answer about the launch, not transport.
                Err(ureq::Error::Status(code, _)) => {
                    return Err(format!("LTI launch got HTTP {code} at {url}"));
                }
                Err(e) => return Err(format!("LTI launch failed at {url}: {e}")),
            };
            let host_jar = jar.entry(host).or_default();
            for sc in resp.all("set-cookie") {
                if let Some((k, v)) = sc.split(';').next().and_then(|f| f.split_once('=')) {
                    host_jar.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            let status = resp.status();
            let location = resp.header("Location").map(str::to_string);
            let body = if (300..400).contains(&status) {
                String::new()
            } else {
                resp.into_string().map_err(|e| e.to_string())?
            };
            Leg {
                status,
                location,
                body,
            }
        };
        if leg.status >= 400 {
            return Err(format!("LTI launch got HTTP {} at {url}", leg.status));
        }

        if (300..400).contains(&leg.status) {
            let loc = leg.location.ok_or("redirect without Location")?;
            let next = url
                .join(&loc)
                .map_err(|e| format!("bad redirect target: {e}"))?;
            if let Some((_, token)) = next.query_pairs().find(|(k, _)| k == "_logintoken") {
                let token = token.into_owned();
                return Ok((token, next));
            }
            url = next;
            continue;
        }

        let (action, fields) = parse_lti_form(&leg.body)
            .ok_or_else(|| format!("no LTI form at {url} — the Canvas session may have lapsed"))?;
        url = url
            .join(&action)
            .map_err(|e| format!("bad form action: {e}"))?;
        form = Some(
            url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(&fields)
                .finish(),
        );
    }
    Err("LTI launch never produced a login token (redirect loop?)".to_string())
}

/// The auto-submit form on a launch page, preferring one aimed at Ed.
fn parse_lti_form(html: &str) -> Option<(String, Vec<(String, String)>)> {
    let doc = Html::parse_document(html);
    let form_sel = scraper::Selector::parse("form").ok()?;
    let input_sel = scraper::Selector::parse("input[name]").ok()?;

    let forms: Vec<_> = doc.select(&form_sel).collect();
    let form = forms
        .iter()
        .find(|f| {
            f.value()
                .attr("action")
                .is_some_and(|a| a.contains("edstem"))
        })
        .or_else(|| forms.iter().find(|f| f.value().attr("action").is_some()))?;

    let action = form.value().attr("action")?.to_string();
    let fields = form
        .select(&input_sel)
        .filter_map(|i| {
            Some((
                i.value().attr("name")?.to_string(),
                i.value().attr("value").unwrap_or("").to_string(),
            ))
        })
        .collect();
    Some((action, fields))
}
