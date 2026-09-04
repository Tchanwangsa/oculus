//! Echo360 lecture capture, independent of Tauri.
//!
//! Access is not an API key — it is an LTI launch. Canvas mints an OAuth-signed
//! form on the course's external-tool page; POSTing that form to Echo360 is
//! what mints the Echo360 session, and the CloudFront cookies that come back
//! are what the media CDN accepts. So everything here starts from the Canvas
//! session cookie we already hold.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const LTI_TOOL_PATH: &str = "/external_tools/701";

/// Echo360 pads every recording with a fixed lead-in before the lecture starts.
pub const TRIM_SECS: f64 = 14.0;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36";
const MIN_VIDEO_BYTES: u64 = 1_000_000;

pub struct Session {
    pub jwt: String,
    play_session: String,
    cf_key_pair_id: String,
    cf_policy: String,
    cf_signature: String,
    cf_tracking: String,
    pub section_id: String,
}

impl Session {
    pub fn cookie_header(&self) -> String {
        format!(
            "ECHO_JWT={}; PLAY_SESSION={}; CloudFront-Key-Pair-Id={}; \
             CloudFront-Policy={}; CloudFront-Signature={}; CloudFront-Tracking2={}",
            self.jwt, self.play_session, self.cf_key_pair_id,
            self.cf_policy, self.cf_signature, self.cf_tracking
        )
    }

    pub fn clone_fields(&self) -> Session {
        Session {
            jwt: self.jwt.clone(),
            play_session: self.play_session.clone(),
            cf_key_pair_id: self.cf_key_pair_id.clone(),
            cf_policy: self.cf_policy.clone(),
            cf_signature: self.cf_signature.clone(),
            cf_tracking: self.cf_tracking.clone(),
            section_id: self.section_id.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Lecture {
    pub id: String,
    pub lesson_id: String,
    pub title: String,
    pub date: String,
    pub duration_seconds: i64,
}

// ── Auth ─────────────────────────────────────────────────────────────────────

pub fn connect(canvas_cookie: &str, course_id: i64) -> Result<Session, String> {
    if canvas_cookie.is_empty() {
        return Err("Canvas session not found — connect Canvas first".to_string());
    }

    let lti_url = format!(
        "{}/courses/{course_id}{LTI_TOOL_PATH}",
        crate::paths::CANVAS_BASE
    );

    eprintln!("[oculus] echo360 auth: fetching LTI page for course {course_id}");
    let html = ureq::get(&lti_url)
        .set("Cookie", canvas_cookie)
        .set("User-Agent", UA)
        .call()
        .map_err(|e| format!("Canvas LTI page fetch failed: {e}"))?
        .into_string()
        .map_err(|e| e.to_string())?;

    let (action, fields) = parse_lti_form(&html).ok_or_else(|| {
        "Could not parse the Echo360 LTI form — the course may not use Echo360, \
         or the Canvas session has lapsed"
            .to_string()
    })?;

    let body: String = fields
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
                url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("&");

    // The agent follows redirects and accumulates Set-Cookie along the way —
    // the session is spread across that redirect chain, not in one response.
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
        return Err("Echo360 auth failed — no ECHO_JWT cookie; the LTI POST was rejected".to_string());
    }

    eprintln!("[oculus] echo360 auth done: section={}", s.section_id);
    Ok(s)
}

fn parse_lti_form(html: &str) -> Option<(String, Vec<(String, String)>)> {
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

fn extract_section_id(path: &str) -> Result<String, String> {
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

// ── Syllabus ─────────────────────────────────────────────────────────────────

/// Every past lesson with a finished, available recording. Anything still
/// processing is skipped: its media id resolves, but the download 404s.
pub fn syllabus(session: &Session) -> Result<Vec<Lecture>, String> {
    let url = format!("https://echo360.net.au/section/{}/syllabus", session.section_id);
    let raw = ureq::get(&url)
        .set("Cookie", &session.cookie_header())
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let items = body["data"].as_array().ok_or("syllabus: no data array")?;

    let mut out = Vec::new();
    for item in items {
        let lesson = &item["lesson"];
        let inner = &lesson["lesson"];

        if !lesson["hasVideo"].as_bool().unwrap_or(false)
            || !lesson["medias"][0]["isAvailable"].as_bool().unwrap_or(false)
            || lesson["medias"][0]["isProcessing"].as_bool().unwrap_or(true)
            || !lesson["isPast"].as_bool().unwrap_or(false)
        {
            continue;
        }
        let media_id = lesson["medias"][0]["id"].as_str().unwrap_or("").to_string();
        if media_id.is_empty() {
            continue;
        }
        let raw_dur = duration_between(
            lesson["captureStartedAt"].as_str().unwrap_or(""),
            lesson["captureEndedAt"].as_str().unwrap_or(""),
        );
        out.push(Lecture {
            id: media_id,
            lesson_id: inner["id"].as_str().unwrap_or("").to_string(),
            title: inner["name"].as_str().unwrap_or("").to_string(),
            date: inner["timing"]["start"].as_str().unwrap_or("").to_string(),
            // The stored duration is post-trim, so it matches the file on disk.
            duration_seconds: (raw_dur - TRIM_SECS as i64).max(0),
        });
    }
    eprintln!("[oculus] echo360 syllabus: {} lectures", out.len());
    Ok(out)
}

fn duration_between(start: &str, end: &str) -> i64 {
    fn secs(s: &str) -> Option<i64> {
        let t = s.split('T').nth(1)?;
        // Keep only HH:MM:SS — stopping at the first character that is neither
        // a digit nor a colon drops the fraction and the zone in one pass.
        // Without this, a plain "…T10:00:00Z" leaves "00Z" in the seconds slot
        // and the whole duration silently reads as zero.
        let t: String = t.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
        let p: Vec<i64> = t.split(':').filter_map(|x| x.parse().ok()).collect();
        (p.len() >= 3).then(|| p[0] * 3600 + p[1] * 60 + p[2])
    }
    let s = secs(start).unwrap_or(0);
    let e = secs(end).unwrap_or(0);
    // Crossing midnight is rare but real for evening lectures.
    if e >= s { e - s } else { e + 86400 - s }
}

// ── Media ────────────────────────────────────────────────────────────────────

pub fn transcript(session: &Session, lesson_id: &str, media_id: &str) -> Result<String, String> {
    let url = format!(
        "https://echo360.net.au/api/ui/echoplayer/lessons/{lesson_id}/medias/{media_id}/transcript-file?format=vtt"
    );
    let mut vtt = String::new();
    ureq::get(&url)
        .set("Cookie", &session.cookie_header())
        .set("Authorization", &format!("Bearer {}", session.jwt))
        .call()
        .map_err(|e| e.to_string())?
        .into_reader()
        .read_to_string(&mut vtt)
        .map_err(|e| e.to_string())?;
    Ok(vtt)
}

/// The download endpoint answers with a 302 to a signed CDN URL rather than
/// the bytes, so redirects are disabled and the Location header is the result.
pub fn download_url(session: &Session, media_id: &str, lesson_id: &str) -> Result<String, String> {
    let url = format!("https://echo360.net.au/media/download/{media_id}/hd1.mp4?lessonId={lesson_id}");
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    match agent.get(&url).set("Cookie", &session.cookie_header()).call() {
        Ok(r) => {
            let status = r.status();
            if (301..=303).contains(&status) {
                r.header("location")
                    .map(str::to_string)
                    .ok_or_else(|| "Download redirect missing Location header".to_string())
            } else {
                Err(format!("Expected a redirect from Echo360, got {status}"))
            }
        }
        Err(ureq::Error::Status(code, _)) => Err(format!("Echo360 download endpoint returned HTTP {code}")),
        Err(e) => Err(format!("Download redirect request failed: {e}")),
    }
}

/// Stream a URL to disk, calling `on_progress` with a percentage as it goes.
pub fn stream_to_file(
    url: &str,
    dest: &Path,
    on_progress: &dyn Fn(u8),
) -> Result<u64, String> {
    let resp = ureq::get(url).call().map_err(|e| format!("HTTP request failed: {e}"))?;
    let total = resp
        .header("content-length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("Failed to create file: {e}"))?;
    let mut buf = [0u8; 65536];
    let mut done = 0u64;
    let mut last_pct = u8::MAX;

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                file.write_all(&buf[..n]).map_err(|e| format!("Write error after {done} bytes: {e}"))?;
                done += n as u64;
                if total > 0 {
                    let pct = (done * 100 / total) as u8;
                    if pct != last_pct {
                        last_pct = pct;
                        on_progress(pct);
                    }
                }
            }
            Err(e) => return Err(format!("Network read error after {done} bytes: {e}")),
        }
    }

    // A truncated download still looks like a file; the size check is what
    // stops a broken one from being trimmed and kept.
    if done < MIN_VIDEO_BYTES {
        return Err(format!("Download incomplete: {done} bytes received"));
    }
    Ok(done)
}

// ── ffmpeg ───────────────────────────────────────────────────────────────────

fn is_runnable(path: &Path) -> bool {
    std::process::Command::new(path)
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// The ffmpeg we ship, else a system install. `resource_dir` is only known
/// inside the app; the CLI passes `None` and finds the dev copy or the system.
pub fn find_ffmpeg(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    let name = format!("ffmpeg{}", std::env::consts::EXE_SUFFIX);
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Bundled app: Tauri copies externalBin next to the main executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join(&name));
        }
    }
    if let Some(res) = resource_dir {
        candidates.push(res.join(&name));
    }

    // Dev: `bun run ffmpeg` writes src-tauri/binaries/ffmpeg-<target-triple>.
    // The triple is not known at runtime, so take whatever the script left.
    let dev_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
    if let Ok(entries) = std::fs::read_dir(&dev_dir) {
        for entry in entries.flatten() {
            let file = entry.file_name();
            let file = file.to_string_lossy();
            if file.starts_with("ffmpeg-") && !file.ends_with(".part") {
                candidates.push(entry.path());
            }
        }
    }

    const SYSTEM: &[&str] = &[
        "ffmpeg",
        r"C:\ProgramData\scoop\shims\ffmpeg.exe",
        r"C:\ffmpeg\bin\ffmpeg.exe",
        r"C:\Program Files\ffmpeg\bin\ffmpeg.exe",
        "/opt/homebrew/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/usr/bin/ffmpeg",
    ];
    candidates.extend(SYSTEM.iter().map(PathBuf::from));

    candidates.into_iter().find(|p| is_runnable(p))
}

/// Drop the lead-in with a stream copy — no re-encode, so it costs seconds.
pub fn trim_video(ffmpeg: &Path, raw: &Path, out: &Path) -> bool {
    std::process::Command::new(ffmpeg)
        .args([
            "-y",
            "-ss",
            &TRIM_SECS.to_string(),
            "-i",
            raw.to_str().unwrap_or(""),
            "-c",
            "copy",
            out.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Remove `raw.mp4` files left behind by an interrupted download.
pub fn cleanup_partial_downloads(data_dir: &Path) {
    let dir = data_dir.join("lectures");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for entry in entries.flatten() {
        let raw = entry.path().join("raw.mp4");
        if raw.exists() {
            eprintln!("[oculus] cleanup: removing orphaned {}", raw.display());
            std::fs::remove_file(&raw).ok();
        }
    }
}

pub fn lecture_dir(data_dir: &Path, media_id: &str) -> PathBuf {
    data_dir.join("lectures").join(media_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_section_id_out_of_a_launch_url() {
        assert_eq!(
            extract_section_id("https://echo360.net.au/section/abc-123/home").unwrap(),
            "abc-123"
        );
        assert!(extract_section_id("https://echo360.net.au/home").is_err());
        assert!(extract_section_id("https://echo360.net.au/section/").is_err());
    }

    #[test]
    fn pulls_hidden_fields_out_of_the_lti_form() {
        let html = r#"<div><form action="https://echo360.net.au/lti" method="POST">
            <input type="hidden" name="oauth_nonce" value="abc&amp;1"/>
            <input type="hidden" name="lti_version" value="LTI-1p0"/>
            <input type="submit" value="Go"/></form></div>"#;
        let (action, fields) = parse_lti_form(html).unwrap();
        assert_eq!(action, "https://echo360.net.au/lti");
        assert_eq!(fields, vec![
            ("oauth_nonce".to_string(), "abc&1".to_string()),
            ("lti_version".to_string(), "LTI-1p0".to_string()),
        ]);
    }

    #[test]
    fn no_echo360_form_means_no_launch() {
        assert!(parse_lti_form("<form action=\"https://elsewhere\"></form>").is_none());
    }

    #[test]
    fn durations_handle_midnight_rollover() {
        assert_eq!(duration_between("2026-01-01T10:00:00Z", "2026-01-01T11:30:00Z"), 5400);
        assert_eq!(duration_between("2026-01-01T23:30:00Z", "2026-01-02T00:30:00Z"), 3600);
    }

    #[test]
    fn durations_read_every_timestamp_shape_echo360_sends() {
        // Fractional seconds, a zone suffix, or neither.
        assert_eq!(duration_between("2026-01-01T10:00:00.000Z", "2026-01-01T10:50:00.000Z"), 3000);
        assert_eq!(duration_between("2026-01-01T10:00:00+11:00", "2026-01-01T10:50:00+11:00"), 3000);
        assert_eq!(duration_between("2026-01-01T10:00:00", "2026-01-01T10:50:00"), 3000);
    }
}
