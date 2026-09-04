//! Ed Discussion access: token auth, course mapping, thread fetching, and the
//! `<document>` XML → Markdown converter.
//!
//! Ed has no OAuth for third parties; the web app authenticates every API call
//! with an `x-token` JWT. The sync engine mints that token itself by walking
//! the Canvas → Ed LTI 1.3 launch (see [`Ed::connect_via_canvas`]), so holding
//! a Canvas session is enough — nothing needs to be pasted. Tokens live ~2
//! weeks, `POST /api/renew_token` extends them, and a dead one is simply
//! re-minted from Canvas on the next sync. `oculus auth ed <TOKEN>` remains as
//! a manual override.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ego_tree::NodeRef;
use scraper::node::Node;
use scraper::Html;

const ED_BASE: &str = "https://edstem.org/api";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
/// A course's board rarely exceeds a few hundred threads in a semester; this
/// is a hard stop, not a target.
const MAX_THREADS: usize = 1000;

pub fn token_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ed-session.token")
}

/// One Ed course as `/api/user` lists it, reduced to what course matching uses.
#[derive(Debug, Clone)]
struct EdCourse {
    id: i64,
    code: String,
    year: String,
    session: String,
    created_at: String,
}

pub struct Ed {
    token: Mutex<String>,
    token_path: PathBuf,
    /// `/api/user` result, fetched once per process — every subject in a run
    /// maps against the same enrolment list.
    courses: Mutex<Option<Vec<EdCourse>>>,
}

impl Ed {
    /// Load the persisted token. Succeeds even without one — callers check
    /// [`Ed::has_session`], and a session-less `Ed` simply syncs nothing.
    pub fn open(data_dir: &Path) -> Self {
        let token_path = token_path(data_dir);
        let token = std::fs::read_to_string(&token_path).unwrap_or_default();
        Ed {
            token: Mutex::new(token.trim().to_string()),
            token_path,
            courses: Mutex::new(None),
        }
    }

    pub fn has_session(&self) -> bool {
        !self.token.lock().unwrap().is_empty()
    }

    /// Validate a pasted token against `/api/user`, then persist it.
    /// Returns the account's display name.
    pub fn set_token(data_dir: &Path, token: &str) -> Result<String, String> {
        let token = token.trim();
        let user = get_json(token, "/user")?;
        let name = user["user"]["name"].as_str().unwrap_or("Ed user").to_string();
        std::fs::write(token_path(data_dir), token).map_err(|e| e.to_string())?;
        Ok(name)
    }

    /// The signed-in user's display name, or an error describing why not.
    pub fn whoami(&self) -> Result<String, String> {
        if !self.has_session() {
            return Err("No saved Ed token.".to_string());
        }
        let user = self.get("/user")?;
        Ok(user["user"]["name"].as_str().unwrap_or("Ed user").to_string())
    }

    fn get(&self, path: &str) -> Result<serde_json::Value, String> {
        let token = self.token.lock().unwrap().clone();
        get_json(&token, path)
    }

    /// Extend the session and persist the fresh token. Best-effort: a failed
    /// renewal only means the current token keeps being used until it expires.
    fn renew(&self) {
        let token = self.token.lock().unwrap().clone();
        if token.is_empty() {
            return;
        }
        let Ok(resp) = ureq::post(&format!("{ED_BASE}/renew_token"))
            .timeout(TIMEOUT)
            .set("x-token", &token)
            .send_string("")
        else {
            return;
        };
        let Some(new) = resp
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .and_then(|v| v["token"].as_str().map(str::to_string))
            .filter(|t| !t.is_empty())
        else {
            return;
        };
        if new != token {
            if let Err(e) = std::fs::write(&self.token_path, &new) {
                eprintln!("[oculus] ed token write failed: {e}");
            }
            *self.token.lock().unwrap() = new;
        }
    }

    // ── LTI auto-connect ─────────────────────────────────────────────────────

    /// The Ed course backing a Canvas course, connecting as needed.
    ///
    /// The fast path matches against the saved session's enrolment list. But
    /// Ed only creates an enrolment when a board is first *opened* — a course
    /// whose Ed tab the user never clicked is invisible to `/api/user` — so a
    /// miss (or a dead session) falls back to launching the course's own LTI
    /// tool, which both enrols the account and names the Ed course in the
    /// launch's final redirect. `Ok(None)` means the course has no Ed tool.
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

    /// Mint an Ed session from the Canvas session by walking the LTI 1.3
    /// launch the way a browser would: Canvas's tool page auto-submits a form
    /// to Ed's `oidc_login`, Ed bounces through Canvas's authorize endpoint,
    /// Canvas posts the signed `id_token` back to Ed's `launch`, and the final
    /// redirect lands on the Ed course carrying a one-shot `_logintoken` —
    /// which `POST /api/login_token` exchanges for the x-token JWT. Returns
    /// the Ed course id the launch landed on.
    pub fn connect_via_canvas(
        &self,
        canvas: &crate::canvas::Canvas,
        canvas_course_id: i64,
    ) -> Result<i64, String> {
        let tool_path = find_ed_tool(canvas, canvas_course_id)?;
        let (login_token, destination) = walk_lti_chain(&canvas.cookie_header(), &tool_path)?;

        let resp = ureq::post(&format!("{ED_BASE}/login_token"))
            .timeout(TIMEOUT)
            .set("Content-Type", "application/json")
            .send_string(&serde_json::json!({ "login_token": login_token }).to_string())
            .map_err(|e| format!("login_token exchange failed: {e}"))?;
        let body: serde_json::Value = resp
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .ok_or("login_token exchange returned no JSON")?;
        let token = body["token"]
            .as_str()
            .filter(|t| !t.is_empty())
            .ok_or("login_token exchange returned no token")?
            .to_string();

        std::fs::write(&self.token_path, &token).map_err(|e| e.to_string())?;
        *self.token.lock().unwrap() = token;
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

    /// The Ed course matching a Canvas course code, or `None`.
    ///
    /// Ed codes are staff-typed free text ("INFO30006", "comp10002 2024s2",
    /// "SWEN20003_2025_S2"), so matching is on the leading subject token, with
    /// the year and semester from the Canvas code ("INFO30006_2026_SM2")
    /// breaking ties between offerings, then recency.
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
            let Some(threads) = batch["threads"].as_array() else { break };
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
        let thread = if detail["thread"].is_object() { &detail["thread"] } else { listing };

        // Names for user_ids, from whichever level of the response carries the
        // roster; comments may also embed their author directly.
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
        if let (Some(id), Some(name)) = (listing["user"]["id"].as_i64(), listing["user"]["name"].as_str()) {
            users.insert(id, name.to_string());
        }

        let title = thread["title"].as_str().or(listing["title"].as_str()).unwrap_or("Thread");
        let mut md = format!("# {title}\n\n");

        let mut meta = Vec::new();
        if let Some(n) = thread["number"].as_i64() {
            meta.push(format!("#{n}"));
        }
        let kind = thread["type"].as_str().unwrap_or("");
        if !kind.is_empty() {
            meta.push(kind.to_string());
        }
        // Questions carry Ed's answered flags; surfaced as resolved/unresolved
        // so the board view can badge them. Re-synced every run, so a thread
        // answered later flips on the next sync.
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

fn get_json(token: &str, path: &str) -> Result<serde_json::Value, String> {
    if token.is_empty() {
        return Err("No saved Ed token.".to_string());
    }
    let url = format!("{ED_BASE}{path}");
    let resp = ureq::get(&url).timeout(TIMEOUT).set("x-token", token).call();
    match resp {
        Ok(r) => r
            .into_string()
            .map_err(|e| format!("unreadable response: {e}"))
            .and_then(|s| serde_json::from_str(&s).map_err(|e| format!("bad JSON: {e}"))),
        Err(ureq::Error::Status(401, _)) => {
            Err("Ed rejected the token (401) — run `oculus auth ed <TOKEN>` with a fresh one".to_string())
        }
        Err(ureq::Error::Status(code, _)) => Err(format!("HTTP {code} for {path}")),
        Err(e) => Err(e.to_string()),
    }
}

// ── LTI chain ────────────────────────────────────────────────────────────────

/// The Canvas path of the course's "Ed Discussion" tool, from the course tabs.
/// Tool ids differ per sub-account, so this is discovered, never assumed.
fn find_ed_tool(canvas: &crate::canvas::Canvas, course_id: i64) -> Result<String, String> {
    let tabs = canvas.get_json(&format!("/api/v1/courses/{course_id}/tabs"))?;
    tabs.as_array()
        .into_iter()
        .flatten()
        .find(|t| {
            t["label"].as_str().is_some_and(|l| l.contains("Ed Discussion"))
                && t["html_url"].as_str().is_some_and(|u| u.contains("/external_tools/"))
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

/// Follow the launch chain — redirects and auto-submit forms — until a
/// redirect carries `_logintoken`; returns the token and where the launch was
/// headed (the Ed course page). Cookies are kept per host and the Canvas
/// session is only ever attached to Canvas requests; Ed's own state cookies
/// (set during `oidc_login`, checked at `launch`) ride in the same jar.
fn walk_lti_chain(canvas_cookie: &str, tool_path: &str) -> Result<(String, url::Url), String> {
    let mut jar: HashMap<String, HashMap<String, String>> = HashMap::new();
    let canvas_host = url::Url::parse(crate::paths::CANVAS_BASE)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .ok_or("bad CANVAS_BASE")?;
    jar.insert(
        canvas_host,
        canvas_cookie
            .split(';')
            .filter_map(|p| p.trim().split_once('='))
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect(),
    );

    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let mut url = url::Url::parse(crate::paths::CANVAS_BASE)
        .and_then(|b| b.join(tool_path))
        .map_err(|e| format!("bad tool path: {e}"))?;
    let mut form: Option<String> = None;

    for _ in 0..12 {
        let host = url.host_str().unwrap_or("").to_string();
        let mut req = match &form {
            Some(_) => agent.post(url.as_str()).set("Content-Type", "application/x-www-form-urlencoded"),
            None => agent.get(url.as_str()),
        }
        .timeout(TIMEOUT)
        .set("User-Agent", UA);
        if let Some(cookies) = jar.get(&host).filter(|c| !c.is_empty()) {
            let header = cookies.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("; ");
            req = req.set("Cookie", &header);
        }

        let resp = match match form.take() {
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

        if (300..400).contains(&resp.status()) {
            let loc = resp.header("Location").ok_or("redirect without Location")?;
            let next = url.join(loc).map_err(|e| format!("bad redirect target: {e}"))?;
            if let Some((_, token)) = next.query_pairs().find(|(k, _)| k == "_logintoken") {
                let token = token.into_owned();
                return Ok((token, next));
            }
            url = next;
            continue;
        }

        let html = resp.into_string().map_err(|e| e.to_string())?;
        let (action, fields) = parse_lti_form(&html).ok_or_else(|| {
            format!("no LTI form at {url} — the Canvas session may have lapsed")
        })?;
        url = url.join(&action).map_err(|e| format!("bad form action: {e}"))?;
        form = Some(
            fields
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}={}",
                        url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
                        url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
                    )
                })
                .collect::<Vec<_>>()
                .join("&"),
        );
    }
    Err("LTI launch never produced a login token (redirect loop?)".to_string())
}

/// The auto-submit form on a launch page: prefer the one aimed at Ed, since
/// Canvas pages carry unrelated forms too.
fn parse_lti_form(html: &str) -> Option<(String, Vec<(String, String)>)> {
    let doc = Html::parse_document(html);
    let form_sel = scraper::Selector::parse("form").ok()?;
    let input_sel = scraper::Selector::parse("input[name]").ok()?;

    let forms: Vec<_> = doc.select(&form_sel).collect();
    let form = forms
        .iter()
        .find(|f| f.value().attr("action").is_some_and(|a| a.contains("edstem")))
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
        .or_else(|| item["user_id"].as_i64().and_then(|id| users.get(&id).cloned()))
        .unwrap_or_else(|| "Anonymous".to_string())
}

/// A reply (answer or comment) and its nested children, each level one
/// blockquote deeper so the conversation shape survives in Markdown.
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
    for line in std::iter::once(head.as_str()).chain(std::iter::once("")).chain(body.lines()) {
        out.push_str(&quote);
        out.push_str(line);
        out.push('\n');
    }

    for child in item["comments"].as_array().into_iter().flatten() {
        render_reply(child, users, false, depth + 1, out);
    }
}

/// A post's body: the `<document>` XML when present (it carries images and
/// links the plain-text `document` field drops), that field otherwise.
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
// Ed bodies are a custom XML dialect (<paragraph>, <bold>, <link href>,
// <image src>, <list style>, <callout>, <break/>). Parsed with the same HTML
// parser as Canvas bodies — which means void-style tags like <break/> swallow
// their following siblings as children, so every renderer below emits its own
// marker and then still recurses into children. Nothing is lost, whichever way
// the parser nested it.

type Ref<'a> = NodeRef<'a, Node>;

fn tag<'a>(n: &Ref<'a>) -> Option<&'a str> {
    n.value().as_element().map(|e| e.name())
}

fn attr<'a>(n: &Ref<'a>, name: &str) -> Option<&'a str> {
    n.value().as_element().and_then(|e| e.attr(name))
}

pub fn document_md(xml: &str) -> String {
    // `<link>` is a void element to an HTML parser, which would strand the
    // link text outside it; renamed before parsing so it nests normally.
    // (`<image>` needs no such help — html5ever rewrites it to `<img>`, which
    // keeps its `src`.)
    let xml = xml.replace("<link ", "<edlink ").replace("</link>", "</edlink>");
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

/// Children rendered into a fresh buffer — for wrappers (bold, links) that
/// only emit their markers around non-empty content.
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
            let level = attr(&n, "level").and_then(|l| l.parse().ok()).unwrap_or(2usize);
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
        // Raw LaTeX in a block element. Emitted as $$ display math, which the
        // viewer's remark-math + KaTeX pipeline renders. (The HTML parser puts
        // <math> children in foreign content, but they're all text nodes.)
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

/// List items must stay on one line; a paragraph inside one otherwise breaks
/// the list apart.
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

/// `"2026-08-07T15:42:01.522942+10:00"` → `"2026-08-07 15:42"`. Ed timestamps
/// arrive in the course's local timezone, so the offset can simply drop.
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

    #[test]
    fn subject_tokens_ignore_staff_typed_suffixes() {
        assert_eq!(code_token("comp10002 2024s2"), "COMP10002");
        assert_eq!(code_token("SWEN20003_2025_S2"), "SWEN20003");
        assert_eq!(code_token("INFO30006"), "INFO30006");
        assert_eq!(code_token("  "), "");
    }

    #[test]
    fn timestamps_lose_the_offset_but_keep_the_minute() {
        assert_eq!(fmt_ts("2026-08-07T15:42:01.522942+10:00"), "2026-08-07 15:42");
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
        // The HTML parser nests everything after <break/> inside it; the
        // converter must still emit that content.
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
        // Entities decode and the double backslash survives; empty <math/> is dropped.
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
