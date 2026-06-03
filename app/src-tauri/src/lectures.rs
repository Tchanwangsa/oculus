use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

// ── Session cache (in-memory, per course) ─────────────────────────────────────

pub struct Echo360Cache(pub Arc<Mutex<HashMap<i64, CachedSession>>>);

pub struct CachedSession {
    session: Echo360Session,
    saved_unix: u64,
}

struct Echo360Session {
    jwt: String,
    play_session: String,
    cf_key_pair_id: String,
    cf_policy: String,
    cf_signature: String,
    cf_tracking: String,
    section_id: String,
}

impl Echo360Session {
    fn cookie_header(&self) -> String {
        format!(
            "ECHO_JWT={}; PLAY_SESSION={}; CloudFront-Key-Pair-Id={}; \
             CloudFront-Policy={}; CloudFront-Signature={}; CloudFront-Tracking2={}",
            self.jwt, self.play_session, self.cf_key_pair_id,
            self.cf_policy, self.cf_signature, self.cf_tracking
        )
    }
    fn clone_fields(&self) -> Echo360Session {
        Echo360Session {
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

const LTI_TOOL_PATH: &str = "/external_tools/701";
const ECHO360_LTI_PROFILE: &str = "9381284d-5253-4e5b-b173-fb47a63576cb";
const TRIM_SECS: f64 = 14.0;
const SESSION_TTL_SECS: u64 = 11 * 3600;

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Auth ──────────────────────────────────────────────────────────────────────

fn get_or_auth(
    app: &AppHandle,
    cache: &Echo360Cache,
    course_id: i64,
) -> Result<Echo360Session, String> {
    {
        let g = cache.0.lock().unwrap();
        if let Some(c) = g.get(&course_id) {
            if now_unix() - c.saved_unix < SESSION_TTL_SECS {
                eprintln!("[oculus] echo360: reusing cached session for course {course_id}");
                return Ok(c.session.clone_fields());
            }
        }
    }
    let session = auth_echo360(app, course_id)?;
    {
        let mut g = cache.0.lock().unwrap();
        g.insert(course_id, CachedSession {
            session: session.clone_fields(),
            saved_unix: now_unix(),
        });
    }
    Ok(session)
}

fn auth_echo360(app: &AppHandle, course_id: i64) -> Result<Echo360Session, String> {
    let win = app
        .get_webview_window("canvas-auth")
        .ok_or_else(|| "Canvas window unavailable — reconnect Canvas first".to_string())?;

    let lti_url = format!(
        "https://canvas.lms.unimelb.edu.au/courses/{course_id}{LTI_TOOL_PATH}"
    );

    win.navigate(lti_url.parse::<url::Url>().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    eprintln!("[oculus] echo360 auth: navigating to LTI page, waiting for ECHO_JWT...");

    // Poll for ECHO_JWT cookie (LTI iframe completes asynchronously)
    let start = Instant::now();
    let mut jwt = String::new();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        if let Ok(cookies) = win.cookies() {
            if let Some(c) = cookies.iter().find(|c| c.name() == "ECHO_JWT") {
                jwt = c.value().to_string();
                eprintln!("[oculus] echo360 auth: ECHO_JWT acquired");
                break;
            }
        }
        if start.elapsed() > Duration::from_secs(25) {
            return Err("Echo360 auth timed out — LTI iframe did not load in 25s".to_string());
        }
    }

    std::thread::sleep(Duration::from_secs(1)); // let remaining cookies settle

    let all = win.cookies().map_err(|e| e.to_string())?;
    let get = |name: &str| -> String {
        all.iter()
            .find(|c| c.name() == name)
            .map(|c| c.value().to_string())
            .unwrap_or_default()
    };

    let play_session   = get("PLAY_SESSION");
    let cf_key_pair_id = get("CloudFront-Key-Pair-Id");
    let cf_policy      = get("CloudFront-Policy");
    let cf_signature   = get("CloudFront-Signature");
    let cf_tracking    = get("CloudFront-Tracking2");

    // Get sectionId via LTI links redirect
    let context_id = extract_play_session_field(&play_session, "ltiContextId")
        .ok_or_else(|| "ltiContextId not found in PLAY_SESSION".to_string())?;

    let cookie_hdr = format!(
        "ECHO_JWT={jwt}; PLAY_SESSION={play_session}; CloudFront-Key-Pair-Id={cf_key_pair_id}; \
         CloudFront-Policy={cf_policy}; CloudFront-Signature={cf_signature}; CloudFront-Tracking2={cf_tracking}"
    );

    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let links_url = format!(
        "https://echo360.net.au/lti/{ECHO360_LTI_PROFILE}/links/{context_id}"
    );
    let section_id = match agent.get(&links_url).set("Cookie", &cookie_hdr).call() {
        Err(ureq::Error::Status(302, r)) | Err(ureq::Error::Status(301, r)) => {
            extract_section_id(r.header("location").unwrap_or(""))?
        }
        Ok(r) => extract_section_id(r.get_url())?,
        Err(e) => return Err(format!("LTI links request failed: {e}")),
    };

    eprintln!("[oculus] echo360 auth done: section={section_id}");

    Ok(Echo360Session {
        jwt,
        play_session,
        cf_key_pair_id,
        cf_policy,
        cf_signature,
        cf_tracking,
        section_id,
    })
}

fn extract_play_session_field(ps: &str, field: &str) -> Option<String> {
    // PLAY_SESSION = {hash}-{url-encoded data}
    let data = ps.splitn(2, '-').nth(1).unwrap_or(ps);
    url::form_urlencoded::parse(data.as_bytes())
        .find(|(k, _)| k == field)
        .map(|(_, v)| v.into_owned())
}

fn extract_section_id(path: &str) -> Result<String, String> {
    let segs: Vec<&str> = path.split('/').collect();
    let idx = segs.iter().position(|&s| s == "section")
        .ok_or_else(|| format!("No 'section' segment in: {path}"))?;
    segs.get(idx + 1)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Empty sectionId in: {path}"))
}

// ── Syllabus ──────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct LectureData {
    pub id: String,
    pub lesson_id: String,
    pub title: String,
    pub date: String,
    pub duration_seconds: i64,
}

fn fetch_syllabus(session: &Echo360Session) -> Result<Vec<LectureData>, String> {
    let url = format!(
        "https://echo360.net.au/section/{}/syllabus",
        session.section_id
    );
    let raw = ureq::get(&url)
        .set("Cookie", &session.cookie_header())
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| e.to_string())?;

    let items = body["data"].as_array()
        .ok_or_else(|| "syllabus: no data array".to_string())?;

    let mut lectures = Vec::new();
    for item in items {
        let lesson = &item["lesson"];
        let inner  = &lesson["lesson"];

        if !lesson["hasVideo"].as_bool().unwrap_or(false)
            || !lesson["medias"][0]["isAvailable"].as_bool().unwrap_or(false)
            || lesson["medias"][0]["isProcessing"].as_bool().unwrap_or(true)
            || !lesson["isPast"].as_bool().unwrap_or(false)
        {
            continue;
        }

        let media_id = lesson["medias"][0]["id"].as_str().unwrap_or("").to_string();
        if media_id.is_empty() { continue; }

        let raw_dur = duration_between(
            lesson["captureStartedAt"].as_str().unwrap_or(""),
            lesson["captureEndedAt"].as_str().unwrap_or(""),
        );

        lectures.push(LectureData {
            id: media_id,
            lesson_id: inner["id"].as_str().unwrap_or("").to_string(),
            title: inner["name"].as_str().unwrap_or("").to_string(),
            date: inner["timing"]["start"].as_str().unwrap_or("").to_string(),
            duration_seconds: (raw_dur - TRIM_SECS as i64).max(0),
        });
    }

    eprintln!("[oculus] echo360 syllabus: {} lectures", lectures.len());
    Ok(lectures)
}

fn duration_between(start: &str, end: &str) -> i64 {
    fn secs(s: &str) -> Option<i64> {
        let t = s.split('T').nth(1)?;
        let t = t.split('.').next().unwrap_or(t);
        let p: Vec<i64> = t.split(':').filter_map(|x| x.parse().ok()).collect();
        if p.len() < 3 { return None; }
        Some(p[0] * 3600 + p[1] * 60 + p[2])
    }
    let s = secs(start).unwrap_or(0);
    let e = secs(end).unwrap_or(0);
    if e >= s { e - s } else { e + 86400 - s }
}

// ── Video download ────────────────────────────────────────────────────────────

fn get_redirect_url(session: &Echo360Session, media_id: &str, lesson_id: &str) -> Result<String, String> {
    let url = format!(
        "https://echo360.net.au/media/download/{media_id}/hd1.mp4?lessonId={lesson_id}"
    );
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    match agent.get(&url).set("Cookie", &session.cookie_header()).call() {
        Err(ureq::Error::Status(302, r)) | Err(ureq::Error::Status(301, r)) => {
            r.header("location")
                .map(|s| s.to_string())
                .ok_or_else(|| "No Location header in download redirect".to_string())
        }
        Ok(r) => Ok(r.get_url().to_string()),
        Err(e) => Err(format!("Download redirect failed: {e}")),
    }
}

fn stream_to_file(
    url: &str,
    cookie: &str,
    dest: &std::path::Path,
    app: &AppHandle,
    media_id: &str,
) -> Result<(), String> {
    let resp = ureq::get(url)
        .set("Cookie", cookie)
        .call()
        .map_err(|e| e.to_string())?;

    let total = resp.header("content-length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut buf = [0u8; 65536];
    let mut done = 0u64;
    let mid = media_id.to_string();

    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                done += n as u64;
                if total > 0 {
                    app.emit("lecture-download-progress", serde_json::json!({
                        "mediaId": &mid,
                        "percent": (done * 100 / total) as u8,
                        "phase": "downloading",
                    })).ok();
                }
            }
            Err(e) => return Err(format!("Stream read error: {e}")),
        }
    }
    Ok(())
}

fn trim_video(raw: &std::path::Path, out: &std::path::Path) -> bool {
    std::process::Command::new("ffmpeg")
        .args([
            "-y", "-ss", "14",
            "-i", raw.to_str().unwrap_or(""),
            "-c", "copy",
            out.to_str().unwrap_or(""),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── VTT processing ────────────────────────────────────────────────────────────

fn shift_vtt(vtt: &str) -> String {
    let mut out = String::with_capacity(vtt.len());
    let mut skip = false;

    for line in vtt.lines() {
        if line.contains(" --> ") {
            let mut parts = line.splitn(2, " --> ");
            let start_str = parts.next().unwrap_or("").trim();
            let rest = parts.next().unwrap_or("");
            let end_str = rest.split_whitespace().next().unwrap_or("").trim();
            let settings = rest.trim_start_matches(end_str).trim();

            if let (Some(s), Some(e)) = (parse_time(start_str), parse_time(end_str)) {
                if s < TRIM_SECS {
                    skip = true;
                    continue;
                }
                skip = false;
                let ns = s - TRIM_SECS;
                let ne = (e - TRIM_SECS).max(0.0);
                if settings.is_empty() {
                    out.push_str(&format!("{} --> {}\n", fmt_time(ns), fmt_time(ne)));
                } else {
                    out.push_str(&format!("{} --> {} {settings}\n", fmt_time(ns), fmt_time(ne)));
                }
                continue;
            }
        }

        if line.is_empty() { skip = false; }

        if !skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn parse_time(s: &str) -> Option<f64> {
    let p: Vec<&str> = s.split(':').collect();
    match p.as_slice() {
        [h, m, sec] => Some(h.parse::<f64>().ok()? * 3600.0 + m.parse::<f64>().ok()? * 60.0 + sec.parse::<f64>().ok()?),
        [m, sec]    => Some(m.parse::<f64>().ok()? * 60.0 + sec.parse::<f64>().ok()?),
        _ => None,
    }
}

fn fmt_time(secs: f64) -> String {
    let h = (secs / 3600.0) as u32;
    let m = ((secs % 3600.0) / 60.0) as u32;
    let s = secs % 60.0;
    format!("{h:02}:{m:02}:{s:06.3}")
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn echo360_sync_lectures(
    app: AppHandle,
    cache: tauri::State<'_, Echo360Cache>,
    canvas_course_id: i64,
) -> Result<Vec<LectureData>, String> {
    let session = get_or_auth(&app, &cache, canvas_course_id)?;
    fetch_syllabus(&session)
}

#[tauri::command]
pub async fn echo360_download_video(
    app: AppHandle,
    cache: tauri::State<'_, Echo360Cache>,
    media_id: String,
    lesson_id: String,
    canvas_course_id: i64,
) -> Result<String, String> {
    let session = get_or_auth(&app, &cache, canvas_course_id)?;
    let redirect = get_redirect_url(&session, &media_id, &lesson_id)?;

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?
        .join("lectures").join(&media_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let raw   = dir.join("raw.mp4");
    let final_ = dir.join("source1.mp4");

    // The redirect URL is a CloudFront signed URL — auth is inline in the URL params.
    // Also send CF cookies for belt-and-suspenders.
    let cf_cookie = format!(
        "CloudFront-Key-Pair-Id={}; CloudFront-Policy={}; CloudFront-Signature={}; CloudFront-Tracking2={}",
        session.cf_key_pair_id, session.cf_policy, session.cf_signature, session.cf_tracking
    );

    stream_to_file(&redirect, &cf_cookie, &raw, &app, &media_id)?;

    app.emit("lecture-download-progress", serde_json::json!({
        "mediaId": &media_id, "percent": 100u8, "phase": "trimming"
    })).ok();

    if trim_video(&raw, &final_) {
        std::fs::remove_file(&raw).ok();
        eprintln!("[oculus] trimmed 14s: {}", final_.display());
    } else {
        eprintln!("[oculus] ffmpeg unavailable — keeping raw video");
        std::fs::rename(&raw, &final_).map_err(|e| e.to_string())?;
    }

    app.emit("lecture-download-progress", serde_json::json!({
        "mediaId": &media_id, "percent": 100u8, "phase": "complete"
    })).ok();

    Ok(final_.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn echo360_download_transcript(
    app: AppHandle,
    cache: tauri::State<'_, Echo360Cache>,
    lesson_id: String,
    media_id: String,
    canvas_course_id: i64,
) -> Result<String, String> {
    let session = get_or_auth(&app, &cache, canvas_course_id)?;

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

    let shifted = shift_vtt(&vtt);

    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?
        .join("lectures").join(&media_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = dir.join("transcript.vtt");
    std::fs::write(&path, shifted.as_bytes()).map_err(|e| e.to_string())?;
    eprintln!("[oculus] transcript saved: {}", path.display());

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn echo360_read_transcript(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}
