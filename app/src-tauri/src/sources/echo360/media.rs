//! Transcripts and video downloads, and where they live on disk.

use super::{Session, SourceNum};

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MIN_VIDEO_BYTES: u64 = 1_000_000;

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

/// The signed CDN URL from the download endpoint's 302 (redirects disabled).
/// Doubles as the availability probe: a missing source answers 500, not a
/// redirect.
pub fn download_url(
    session: &Session,
    media_id: &str,
    lesson_id: &str,
    source: SourceNum,
) -> Result<String, String> {
    let url = format!(
        "https://echo360.net.au/media/download/{media_id}/hd{source}.mp4?lessonId={lesson_id}"
    );
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    match agent
        .get(&url)
        .set("Cookie", &session.cookie_header())
        .call()
    {
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
        Err(ureq::Error::Status(code, _)) => {
            Err(format!("Echo360 download endpoint returned HTTP {code}"))
        }
        Err(e) => Err(format!("Download redirect request failed: {e}")),
    }
}

/// The error `stream_to_file` returns when `should_cancel` stopped it.
pub const CANCELLED: &str = "cancelled";

/// Stream `url` to `dest`, reporting whole-percent progress; `should_cancel`
/// is polled per 64 KB chunk.
pub fn stream_to_file(
    url: &str,
    dest: &Path,
    on_progress: &dyn Fn(u8),
    should_cancel: &dyn Fn() -> bool,
) -> Result<u64, String> {
    let resp = ureq::get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;
    let total = resp
        .header("content-length")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = resp.into_reader();
    let mut file =
        std::fs::File::create(dest).map_err(|e| format!("Failed to create file: {e}"))?;
    let mut buf = [0u8; 65536];
    let mut done = 0u64;
    let mut last_pct = u8::MAX;

    loop {
        if should_cancel() {
            return Err(CANCELLED.to_string());
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                file.write_all(&buf[..n])
                    .map_err(|e| format!("Write error after {done} bytes: {e}"))?;
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

    // A truncated download still looks like a file.
    if done < MIN_VIDEO_BYTES {
        return Err(format!("Download incomplete: {done} bytes received"));
    }
    Ok(done)
}

/// Remove untrimmed `raw*.mp4` downloads left by an interrupted run.
pub fn cleanup_partial_downloads(data_dir: &Path) {
    let dir = data_dir.join("lectures");
    let Ok(lectures) = std::fs::read_dir(&dir) else {
        return;
    };
    for lecture in lectures.flatten() {
        let Ok(files) = std::fs::read_dir(lecture.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("raw") && name.ends_with(".mp4") {
                eprintln!(
                    "[oculus] cleanup: removing orphaned {}",
                    file.path().display()
                );
                std::fs::remove_file(file.path()).ok();
            }
        }
    }
}

pub fn lecture_dir(data_dir: &Path, media_id: &str) -> PathBuf {
    data_dir.join("lectures").join(media_id)
}

/// Where a trimmed stream lives.
pub fn source_path(dir: &Path, source: SourceNum) -> PathBuf {
    dir.join(format!("source{source}.mp4"))
}

/// The untrimmed download, one per source so both can run at once.
pub fn partial_path(dir: &Path, source: SourceNum) -> PathBuf {
    dir.join(format!("raw{source}.mp4"))
}
