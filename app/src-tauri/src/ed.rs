//! Ed Discussion access: token auth, course mapping, thread fetching, and the
//! `<document>` XML → Markdown converter.
//!
//! Ed authenticates API calls with an `x-token` JWT that oculus-keyd holds and
//! attaches to its `ed` route; this module never reads it back. The token is
//! minted from the Canvas session by walking the LTI 1.3 launch
//! ([`Ed::connect_via_canvas`]); `renew_token` extends it and a dead one is
//! re-minted on the next sync. `oculus auth ed <TOKEN>` is a manual override.
//! See `docs/auth.md`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use keyd_core::client::{SessionKind, SESSION_TIMEOUT};

use crate::credentials::{Credentialed, KeydError};

use ego_tree::NodeRef;
use scraper::node::Node;
use scraper::Html;

/// Ed's API origin, for the two requests that do not go through oculus-keyd:
/// the one-shot `login_token` exchange (it has no session yet) and checking a
/// token a person just pasted.
const ED_BASE: &str = "https://edstem.org/api";
/// The path prefix oculus-keyd's `ed` route takes.
const ED_API: &str = "/api";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// A hard stop, not a target.
const MAX_THREADS: usize = 1000;

#[derive(Debug, Clone)]
struct EdCourse {
    id: i64,
    code: String,
    year: String,
    session: String,
    created_at: String,
}

pub struct Ed {
    keyd: Credentialed,
    /// `/api/user` enrolments, fetched once per process.
    courses: Mutex<Option<Vec<EdCourse>>>,
}

/// Why an Ed request has no answer, in words for the log and the CLI.
fn keyd_failure(error: KeydError) -> String {
    match error {
        KeydError::Absent => "oculus-keyd is not running or not installed, and Ed is reached \
                              only through it (`oculus keyd status`)."
            .to_string(),
        KeydError::Missing(_) | KeydError::NoSession(..) => {
            "No saved Ed token — run `oculus auth ed <TOKEN>`, or sync a course with an Ed \
             Discussion tool."
                .to_string()
        }
        other => other.to_string(),
    }
}

impl Ed {
    /// A client of the oculus-keyd serving `data_dir`.
    pub fn open(data_dir: &Path) -> Self {
        Ed {
            keyd: Credentialed::at(data_dir),
            courses: Mutex::new(None),
        }
    }

    /// Whether oculus-keyd holds an Ed token right now.
    pub fn has_session(&self) -> bool {
        self.keyd.session_status().is_ok_and(|s| s.ed)
    }

    /// Validate a pasted token against `/api/user`, hand it to oculus-keyd,
    /// return the name. The check is a direct request: the token is not
    /// stored yet, and a bad paste must not replace a working session.
    pub fn set_token(data_dir: &Path, token: &str) -> Result<String, String> {
        store_token(&Credentialed::at(data_dir), ED_BASE, token)
    }

    pub fn whoami(&self) -> Result<String, String> {
        let user = self.get("/user")?;
        Ok(user["user"]["name"]
            .as_str()
            .unwrap_or("Ed user")
            .to_string())
    }

    /// GET `/api{path}` with oculus-keyd attaching the token.
    fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        let reply = self
            .keyd
            .send(
                "ed",
                "GET",
                &format!("{ED_API}{path}"),
                &[],
                b"",
                SESSION_TIMEOUT,
            )
            .map_err(keyd_failure)?;
        json_reply(reply.status, &reply.body, path)
    }

    /// Extend the session and keep the fresh token. Best-effort.
    fn renew(&self) {
        let Ok(reply) = self.keyd.send(
            "ed",
            "POST",
            &format!("{ED_API}/renew_token"),
            &[],
            b"",
            SESSION_TIMEOUT,
        ) else {
            return;
        };
        if !(200..300).contains(&reply.status) {
            return;
        }
        // The token is in the body, so this is the one place it reaches us.
        let Some(new) = serde_json::from_slice::<serde_json::Value>(&reply.body)
            .ok()
            .and_then(|v| v["token"].as_str().map(str::to_string))
            .filter(|t| !t.is_empty())
        else {
            return;
        };
        if let Err(e) = self.keyd.session_put(SessionKind::Ed, &new) {
            eprintln!("[oculus] ed token was not saved: {e}");
        }
    }

    // ── LTI auto-connect ─────────────────────────────────────────────────────

    /// The Ed course backing a Canvas course. Ed only enrols an account when a
    /// board is first opened, so a miss in `/api/user` falls back to the LTI
    /// launch, which enrols and names the course. `Ok(None)`: no Ed tool.
    pub fn resolve_course(
        &self,
        canvas: &crate::canvas::Canvas,
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
        canvas: &crate::canvas::Canvas,
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

    fn courses(&self) -> Result<Vec<EdCourse>, String> {
        let mut guard = self.courses.lock().unwrap();
        if let Some(cached) = guard.as_ref() {
            return Ok(cached.clone());
        }
        self.renew();
        let user = self.get("/user")?;
        let list: Vec<EdCourse> = user["courses"]
            .as_array()
            .map(|cs| {
                cs.iter()
                    .filter_map(|c| {
                        let c = &c["course"];
                        Some(EdCourse {
                            id: c["id"].as_i64()?,
                            code: c["code"].as_str().unwrap_or("").to_string(),
                            year: c["year"].as_str().unwrap_or("").to_string(),
                            session: c["session"].as_str().unwrap_or("").to_string(),
                            created_at: c["created_at"].as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        *guard = Some(list.clone());
        Ok(list)
    }

    /// The Ed course matching a Canvas course code. Ed codes are staff-typed
    /// free text, so match on the leading subject token; year and semester
    /// from the Canvas code break ties, then recency.
    pub fn course_for(&self, canvas_code: &str) -> Result<Option<i64>, String> {
        let want = code_token(canvas_code);
        if want.is_empty() {
            return Ok(None);
        }
        let canvas_upper = canvas_code.to_uppercase();
        let courses = self.courses()?;

        let mut best: Option<(i32, &EdCourse)> = None;
        for c in courses.iter().filter(|c| code_token(&c.code) == want) {
            let mut score = 0;
            if !c.year.is_empty() && canvas_upper.contains(&c.year) {
                score += 2;
            }
            if let Some(d) = c.session.chars().find(char::is_ascii_digit) {
                if canvas_upper.contains(&format!("SM{d}")) {
                    score += 1;
                }
            }
            let better = match &best {
                Some((s, b)) => score > *s || (score == *s && c.created_at > b.created_at),
                None => true,
            };
            if better {
                best = Some((score, c));
            }
        }
        Ok(best.map(|(_, c)| c.id))
    }

    /// Every thread on a course's board, newest first (list entries only —
    /// no replies; those come with [`Ed::thread_markdown`]).
    pub fn threads(&self, course_id: i64) -> Result<Vec<serde_json::Value>, String> {
        let mut out = Vec::new();
        loop {
            let batch = self.get(&format!(
                "/courses/{course_id}/threads?limit=100&offset={}&sort=new",
                out.len()
            ))?;
            let Some(threads) = batch["threads"].as_array() else {
                break;
            };
            let n = threads.len();
            out.extend(threads.iter().cloned());
            if n < 100 || out.len() >= MAX_THREADS {
                break;
            }
        }
        Ok(out)
    }

    /// One thread with its replies, rendered to Markdown.
    pub fn thread_markdown(&self, listing: &serde_json::Value) -> Result<String, String> {
        let id = listing["id"].as_i64().ok_or("thread without id")?;
        let detail = self.get(&format!("/threads/{id}?view=1"))?;
        let thread = if detail["thread"].is_object() {
            &detail["thread"]
        } else {
            listing
        };

        // The roster may sit at either level; comments may embed their author.
        let mut users: HashMap<i64, String> = HashMap::new();
        for list in [&detail["users"], &detail["thread"]["users"]] {
            if let Some(arr) = list.as_array() {
                for u in arr {
                    if let (Some(id), Some(name)) = (u["id"].as_i64(), u["name"].as_str()) {
                        users.insert(id, name.to_string());
                    }
                }
            }
        }
        if let (Some(id), Some(name)) = (
            listing["user"]["id"].as_i64(),
            listing["user"]["name"].as_str(),
        ) {
            users.insert(id, name.to_string());
        }

        let title = thread["title"]
            .as_str()
            .or(listing["title"].as_str())
            .unwrap_or("Thread");
        let mut md = format!("# {title}\n\n");

        let mut meta = Vec::new();
        if let Some(n) = thread["number"].as_i64() {
            meta.push(format!("#{n}"));
        }
        let kind = thread["type"].as_str().unwrap_or("");
        if !kind.is_empty() {
            meta.push(kind.to_string());
        }
        // Surfaced so the board view can badge questions.
        if kind == "question" {
            let answered = [thread, listing].iter().any(|t| {
                t["is_answered"].as_bool().unwrap_or(false)
                    || t["is_staff_answered"].as_bool().unwrap_or(false)
            });
            meta.push(if answered { "resolved" } else { "unresolved" }.to_string());
        }
        let category = [
            thread["category"].as_str(),
            thread["subcategory"].as_str(),
            thread["subsubcategory"].as_str(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
        if !category.is_empty() {
            meta.push(category);
        }
        md.push_str(&format!("**{}**  \n", meta.join(" · ")));
        md.push_str(&format!(
            "**By:** {} · {}\n\n---\n\n",
            author_name(thread, &users),
            fmt_ts(thread["created_at"].as_str().unwrap_or(""))
        ));

        md.push_str(&content_md(thread));
        md.push('\n');

        let mut replies = String::new();
        for a in thread["answers"].as_array().into_iter().flatten() {
            render_reply(a, &users, true, 0, &mut replies);
        }
        for c in thread["comments"].as_array().into_iter().flatten() {
            render_reply(c, &users, false, 0, &mut replies);
        }
        if !replies.is_empty() {
            md.push_str("\n## Replies\n");
            md.push_str(&replies);
        }

        Ok(crate::md::collapse_blank_lines(&md).trim().to_string() + "\n")
    }
}

/// An Ed API answer as JSON.
fn json_reply(status: u16, body: &[u8], path: &str) -> Result<serde_json::Value, String> {
    if (200..300).contains(&status) {
        return serde_json::from_slice(body).map_err(|e| format!("bad JSON: {e}"));
    }
    Err(status_error(status, path))
}

fn status_error(status: u16, path: &str) -> String {
    match status {
        401 => "Ed rejected the token (401) — run `oculus auth ed <TOKEN>` with a fresh one"
            .to_string(),
        code => format!("HTTP {code} for {path}"),
    }
}

/// Check `token` against `base`/user directly, then give it to oculus-keyd.
fn store_token(keyd: &Credentialed, base: &str, token: &str) -> Result<String, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("The Ed token is empty.".to_string());
    }
    let url = format!("{base}/user");
    let user: serde_json::Value = match ureq::get(&url)
        .timeout(TIMEOUT)
        .set("x-token", token)
        .call()
    {
        Ok(r) => r
            .into_string()
            .map_err(|e| format!("unreadable response: {e}"))
            .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("bad JSON: {e}")))?,
        Err(ureq::Error::Status(code, _)) => return Err(status_error(code, "/user")),
        Err(e) => return Err(e.to_string()),
    };
    let name = user["user"]["name"]
        .as_str()
        .unwrap_or("Ed user")
        .to_string();
    keyd.session_put(SessionKind::Ed, token)
        .map_err(keyd_failure)?;
    Ok(name)
}

/// Ed's one unauthenticated request: the one-shot `_logintoken` from the LTI
/// launch for the x-token. It has no session to attach, so it is not sent
/// through oculus-keyd.
fn exchange_login_token(base: &str, login_token: &str) -> Result<String, String> {
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

// ── LTI chain ────────────────────────────────────────────────────────────────

/// The course's "Ed Discussion" tool path — tool ids differ per sub-account.
fn find_ed_tool(canvas: &crate::canvas::Canvas, course_id: i64) -> Result<String, String> {
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
fn ed_course_in_url(u: &url::Url) -> Option<i64> {
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
fn walk_lti_chain(
    canvas: &crate::canvas::Canvas,
    tool_path: &str,
) -> Result<(String, url::Url), String> {
    let mut jar: HashMap<String, HashMap<String, String>> = HashMap::new();
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut url = crate::canvas::resolve(tool_path)?;
    let mut form: Option<String> = None;

    for _ in 0..12 {
        let sent = form.take();
        let leg = if crate::canvas::is_canvas(&url) {
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

// ── Rendering ────────────────────────────────────────────────────────────────

fn author_name(item: &serde_json::Value, users: &HashMap<i64, String>) -> String {
    if item["is_anonymous"].as_bool().unwrap_or(false) {
        return "Anonymous".to_string();
    }
    item["user"]["name"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            item["user_id"]
                .as_i64()
                .and_then(|id| users.get(&id).cloned())
        })
        .unwrap_or_else(|| "Anonymous".to_string())
}

/// A reply and its children, each level one blockquote deeper.
fn render_reply(
    item: &serde_json::Value,
    users: &HashMap<i64, String>,
    is_answer: bool,
    depth: usize,
    out: &mut String,
) {
    let mut badge = String::new();
    if is_answer {
        badge.push_str(" (answer)");
    }
    if item["is_endorsed"].as_bool().unwrap_or(false) {
        badge.push_str(" (endorsed)");
    }
    let head = format!(
        "**{}**{badge} · {}",
        author_name(item, users),
        fmt_ts(item["created_at"].as_str().unwrap_or(""))
    );
    let body = content_md(item);

    let quote = "> ".repeat(depth);
    out.push('\n');
    for line in std::iter::once(head.as_str())
        .chain(std::iter::once(""))
        .chain(body.lines())
    {
        out.push_str(&quote);
        out.push_str(line);
        out.push('\n');
    }

    for child in item["comments"].as_array().into_iter().flatten() {
        render_reply(child, users, false, depth + 1, out);
    }
}

/// The `<document>` XML when present (it keeps images and links), else the
/// plain-text `document` field.
fn content_md(item: &serde_json::Value) -> String {
    let xml = item["content"].as_str().unwrap_or("");
    if !xml.is_empty() {
        let md = document_md(xml);
        if !md.trim().is_empty() {
            return md;
        }
    }
    item["document"].as_str().unwrap_or("").trim().to_string()
}

// ── `<document>` XML → Markdown ──────────────────────────────────────────────
//
// Parsed with the HTML parser, so void-style tags like <break/> swallow their
// following siblings: every renderer emits its marker and still recurses.

type Ref<'a> = NodeRef<'a, Node>;

fn tag<'a>(n: &Ref<'a>) -> Option<&'a str> {
    n.value().as_element().map(|e| e.name())
}

fn attr<'a>(n: &Ref<'a>, name: &str) -> Option<&'a str> {
    n.value().as_element().and_then(|e| e.attr(name))
}

pub fn document_md(xml: &str) -> String {
    // `<link>` is void to an HTML parser and would strand its text; rename it.
    // (html5ever rewrites `<image>` to `<img>`, keeping `src`.)
    let xml = xml
        .replace("<link ", "<edlink ")
        .replace("</link>", "</edlink>");
    let doc = Html::parse_fragment(&xml);
    let mut out = String::new();
    render_nodes(doc.tree.root(), &mut out);
    crate::md::collapse_blank_lines(&out).trim().to_string()
}

fn render_nodes(n: Ref<'_>, out: &mut String) {
    for c in n.children() {
        render_node(c, out);
    }
}

fn inner(n: Ref<'_>) -> String {
    let mut s = String::new();
    render_nodes(n, &mut s);
    s
}

fn render_node(n: Ref<'_>, out: &mut String) {
    if let Some(t) = n.value().as_text() {
        out.push_str(t);
        return;
    }
    let Some(name) = tag(&n) else {
        render_nodes(n, out);
        return;
    };
    match name {
        "paragraph" | "figure" => {
            render_nodes(n, out);
            out.push_str("\n\n");
        }
        "heading" => {
            let level = attr(&n, "level")
                .and_then(|l| l.parse().ok())
                .unwrap_or(2usize);
            out.push_str(&"#".repeat(level.clamp(1, 6)));
            out.push(' ');
            render_nodes(n, out);
            out.push_str("\n\n");
        }
        "bold" => wrap(n, "**", out),
        "italic" => wrap(n, "*", out),
        "underline" | "spoiler" => render_nodes(n, out),
        "code" => wrap(n, "`", out),
        "edlink" => {
            let href = attr(&n, "href").unwrap_or("");
            let text = inner(n);
            let text = text.trim();
            if text.is_empty() {
                out.push_str(href);
            } else {
                out.push_str(&format!("[{text}]({href})"));
            }
        }
        "image" | "img" => {
            out.push_str(&format!("![]({})\n\n", attr(&n, "src").unwrap_or("")));
            render_nodes(n, out);
        }
        "break" => {
            out.push_str("  \n");
            render_nodes(n, out);
        }
        "list" => {
            let ordered = attr(&n, "style") == Some("number");
            let mut i = 1;
            for c in n.children() {
                if tag(&c) == Some("list-item") {
                    let marker = if ordered {
                        let m = format!("{i}.");
                        i += 1;
                        m
                    } else {
                        "-".to_string()
                    };
                    let body = inner(c);
                    out.push_str(&format!("{marker} {}\n", squeeze_item(&body)));
                } else {
                    render_node(c, out);
                }
            }
            out.push('\n');
        }
        "callout" => {
            let body = inner(n);
            for line in body.trim().lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        // Raw LaTeX, emitted as $$ display math.
        "math" => {
            let latex: String = n
                .descendants()
                .filter_map(|d| d.value().as_text().map(|t| t.to_string()))
                .collect();
            let latex = latex.trim();
            if !latex.is_empty() {
                out.push_str(&format!("\n$$\n{latex}\n$$\n\n"));
            }
        }
        "pre" | "snippet" => {
            let body: String = n
                .descendants()
                .filter_map(|d| d.value().as_text().map(|t| t.to_string()))
                .collect();
            out.push_str(&format!("```\n{}\n```\n\n", body.trim_end_matches('\n')));
        }
        _ => render_nodes(n, out),
    }
}

fn wrap(n: Ref<'_>, marks: &str, out: &mut String) {
    let body = inner(n);
    let trimmed = body.trim();
    if trimmed.is_empty() {
        out.push_str(&body);
    } else {
        out.push_str(&format!("{marks}{trimmed}{marks}"));
    }
}

/// A list item must stay on one line or the list breaks apart.
fn squeeze_item(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// The leading subject code: `"comp10002 2024s2"` → `"COMP10002"`.
fn code_token(code: &str) -> String {
    code.trim()
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

/// `"2026-08-07T15:42:01.522942+10:00"` → `"2026-08-07 15:42"` (already local).
fn fmt_ts(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes()[10] == b'T' {
        format!("{} {}", &iso[..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::{FakeKeyd, FakeServer, Reply, Scratch};
    use serde_json::json;

    /// An oculus-keyd that answers Ed's `/api/user` and records the rest.
    fn keyd_for_ed(dir: &Scratch) -> FakeKeyd {
        FakeKeyd::start(dir, |header, _| match header["op"].as_str().unwrap() {
            "forward" => match header["path"].as_str().unwrap() {
                "/api/user" => (
                    json!({"status": 200, "headers": []}),
                    br#"{"user":{"name":"Ada"},"courses":[]}"#.to_vec(),
                ),
                "/api/renew_token" => (
                    json!({"status": 200, "headers": []}),
                    br#"{"token":"RENEWED"}"#.to_vec(),
                ),
                _ => (json!({"status": 401, "headers": []}), b"{}".to_vec()),
            },
            "session_status" => (
                json!({"canvas": false, "sso": false, "ed": true,
                       "authenticated": false, "signed_out": false}),
                vec![],
            ),
            _ => (json!({}), vec![]),
        })
    }

    fn session_puts(keyd: &FakeKeyd) -> Vec<(String, String)> {
        keyd.requests()
            .into_iter()
            .filter(|(h, _)| h["op"] == "session_put")
            .map(|(h, _)| {
                (
                    h["kind"].as_str().unwrap().to_string(),
                    h["value"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn requests_go_to_the_ed_route_and_carry_no_token() {
        let dir = Scratch::new("ed-route");
        let keyd = keyd_for_ed(&dir);
        let ed = Ed::open(&dir);
        assert_eq!(ed.whoami().unwrap(), "Ada");
        let (header, _) = &keyd.requests()[0];
        assert_eq!(header["secret"], "ed");
        assert_eq!(header["method"], "GET");
        assert_eq!(header["path"], "/api/user");
        assert_eq!(header["headers"], json!([]));
        assert!(!header.to_string().contains("x-token"));
    }

    #[test]
    fn has_session_is_what_keyd_reports() {
        let dir = Scratch::new("ed-status");
        let _keyd = keyd_for_ed(&dir);
        assert!(Ed::open(&dir).has_session());
        assert!(!Ed::open(&Scratch::new("ed-no-keyd")).has_session());
    }

    #[test]
    fn renew_hands_the_fresh_token_to_keyd() {
        let dir = Scratch::new("ed-renew");
        let keyd = keyd_for_ed(&dir);
        Ed::open(&dir).renew();
        let (header, _) = &keyd.requests()[0];
        assert_eq!(header["method"], "POST");
        assert_eq!(header["path"], "/api/renew_token");
        assert_eq!(session_puts(&keyd), [("ed".into(), "RENEWED".into())]);
    }

    #[test]
    fn a_rejected_token_points_at_the_manual_override() {
        let dir = Scratch::new("ed-401");
        let _keyd = FakeKeyd::start(&dir, |_, _| {
            (json!({"status": 401, "headers": []}), b"{}".to_vec())
        });
        let err = Ed::open(&dir).whoami().unwrap_err();
        assert!(err.contains("oculus auth ed <TOKEN>"), "{err}");
    }

    #[test]
    fn no_token_and_no_keyd_are_told_apart() {
        let dir = Scratch::new("ed-missing");
        let _keyd = FakeKeyd::start(&dir, |_, _| {
            (
                json!({"error": "missing", "detail": "no ed session is stored"}),
                vec![],
            )
        });
        let err = Ed::open(&dir).whoami().unwrap_err();
        assert!(err.starts_with("No saved Ed token"), "{err}");

        let err = Ed::open(&Scratch::new("ed-absent")).whoami().unwrap_err();
        assert!(err.contains("not running or not installed"), "{err}");
    }

    #[test]
    fn a_pasted_token_is_checked_directly_and_then_given_to_keyd() {
        let ed = FakeServer::start(|hit| {
            if hit.header("x-token") == Some("good") {
                Reply::json(json!({"user": {"name": "Ada"}}))
            } else {
                Reply::status(401, json!({}))
            }
        });
        let dir = Scratch::new("ed-set");
        let keyd = keyd_for_ed(&dir);
        let broker = Credentialed::at(&dir);

        let err = store_token(&broker, &ed.origin(), "bad").unwrap_err();
        assert!(err.contains("oculus auth ed <TOKEN>"), "{err}");
        assert!(
            session_puts(&keyd).is_empty(),
            "a bad paste is never stored"
        );

        assert_eq!(
            store_token(&broker, &ed.origin(), " good\n").unwrap(),
            "Ada"
        );
        assert_eq!(session_puts(&keyd), [("ed".into(), "good".into())]);
        assert!(keyd.requests().iter().all(|(h, _)| h["op"] != "forward"));
    }

    #[test]
    fn the_login_token_exchange_is_direct_and_never_touches_keyd() {
        let ed = FakeServer::start(|_| Reply::json(json!({"token": "MINTED"})));
        let dir = Scratch::new("ed-login");
        let keyd = keyd_for_ed(&dir);
        assert_eq!(
            exchange_login_token(&ed.origin(), "once").unwrap(),
            "MINTED"
        );
        let hits = ed.hits();
        assert_eq!(hits[0].method, "POST");
        assert_eq!(hits[0].url, "/login_token");
        assert_eq!(hits[0].json(), json!({"login_token": "once"}));
        assert!(keyd.requests().is_empty());
    }

    #[test]
    fn the_launch_reaches_canvas_through_keyd_and_other_hosts_without_its_cookie() {
        let lti = FakeServer::start(|hit| {
            Reply::from((302, Vec::new())).with_header(
                "Location",
                &format!("{}/au/courses/38809?_logintoken=LT", hit.origin),
            )
        });
        let form = format!(
            r#"<form action="{}/lti/launch"><input name="id_token" value="jwt"/></form>"#,
            lti.origin()
        );
        let dir = Scratch::new("ed-lti");
        let keyd = FakeKeyd::start(&dir, move |_, _| {
            (
                json!({"status": 200, "headers": []}),
                form.clone().into_bytes(),
            )
        });
        let canvas = crate::canvas::Canvas::open(&dir);

        let (token, destination) = walk_lti_chain(&canvas, "/courses/5/external_tools/9").unwrap();

        assert_eq!(token, "LT");
        assert_eq!(ed_course_in_url(&destination), Some(38809));
        let requests = keyd.requests();
        assert_eq!(requests.len(), 1, "only the Canvas leg used keyd");
        assert_eq!(requests[0].0["secret"], "canvas");
        assert_eq!(requests[0].0["path"], "/courses/5/external_tools/9");
        let hits = lti.hits();
        assert_eq!(hits[0].method, "POST");
        assert_eq!(hits[0].body, b"id_token=jwt");
        assert!(hits[0].header("cookie").is_none());
    }

    #[test]
    fn a_dead_canvas_session_ends_the_launch_with_the_reason() {
        let dir = Scratch::new("ed-lti-dead");
        let _keyd = FakeKeyd::start(&dir, |_, _| {
            (
                json!({"error": "missing", "detail": "no canvas session is stored",
                       "signin": {"result": "error", "code": "signed_out"}}),
                vec![],
            )
        });
        let canvas = crate::canvas::Canvas::open(&dir);
        let err = walk_lti_chain(&canvas, "/courses/5/external_tools/9").unwrap_err();
        assert!(err.contains("Signed out"), "{err}");
        assert!(canvas.expired().is_some());
    }

    #[test]
    fn subject_tokens_ignore_staff_typed_suffixes() {
        assert_eq!(code_token("comp10002 2024s2"), "COMP10002");
        assert_eq!(code_token("SWEN20003_2025_S2"), "SWEN20003");
        assert_eq!(code_token("INFO30006"), "INFO30006");
        assert_eq!(code_token("  "), "");
    }

    #[test]
    fn timestamps_lose_the_offset_but_keep_the_minute() {
        assert_eq!(
            fmt_ts("2026-08-07T15:42:01.522942+10:00"),
            "2026-08-07 15:42"
        );
        assert_eq!(fmt_ts("junk"), "junk");
    }

    #[test]
    fn document_xml_becomes_markdown() {
        let md = document_md(
            r#"<document version="2.0"><paragraph><bold>CTF</bold> on <link href="https://x.test">this page</link></paragraph><paragraph>See below.</paragraph></document>"#,
        );
        assert_eq!(md, "**CTF** on [this page](https://x.test)\n\nSee below.");
    }

    #[test]
    fn void_tags_do_not_swallow_content() {
        let md = document_md(
            r#"<document><paragraph>one<break/>two <bold>three</bold></paragraph></document>"#,
        );
        assert!(md.contains("one"), "{md}");
        assert!(md.contains("two"), "{md}");
        assert!(md.contains("**three**"), "{md}");
    }

    #[test]
    fn math_blocks_become_display_latex() {
        let md = document_md(
            r#"<document><paragraph>So starting with</paragraph><math>\left(\begin{matrix}1&amp;0\\0&amp;1\end{matrix}\right)</math><paragraph>using dagger.</paragraph><math/></document>"#,
        );
        assert_eq!(
            md,
            "So starting with\n\n$$\n\\left(\\begin{matrix}1&0\\\\0&1\\end{matrix}\\right)\n$$\n\nusing dagger."
        );
    }

    #[test]
    fn images_lists_and_callouts_render() {
        let md = document_md(
            r#"<document><figure><image src="https://img.test/a.png" width="10"/></figure><list style="number"><list-item><paragraph>first</paragraph></list-item><list-item><paragraph>second</paragraph></list-item></list><callout type="info"><bold>note</bold></callout></document>"#,
        );
        assert!(md.contains("![](https://img.test/a.png)"), "{md}");
        assert!(md.contains("1. first"), "{md}");
        assert!(md.contains("2. second"), "{md}");
        assert!(md.contains("> **note**"), "{md}");
    }
}
