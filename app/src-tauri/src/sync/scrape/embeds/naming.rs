//! Where a downloaded embed lands in the library.

use crate::library::paths::course_rel_path;

/// Extensions a media name may carry that the muxed MP4 replaces.
const VIDEO_EXTS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "avi"];

/// The data-dir-relative path for `media_name` under the course's `files/`,
/// always `.mp4`. When another file holds that name, the media id's first
/// eight characters go before the extension; that name carries this media's
/// id, so if it too exists it is this video's earlier copy and is replaced.
pub(super) fn video_rel(
    code: &str,
    media_name: &str,
    media_id: &str,
    exists: &dyn Fn(&str) -> bool,
) -> Result<String, String> {
    let short: String = media_id.chars().take(8).collect();
    let name = media_name.trim();
    let stem = match name.rsplit_once('.') {
        Some((stem, ext)) if VIDEO_EXTS.contains(&ext.to_ascii_lowercase().as_str()) => stem,
        _ => name,
    }
    .trim();
    let stem = if stem.is_empty() {
        format!("echo-{short}")
    } else {
        stem.to_string()
    };
    let plain = course_rel_path(code, &format!("files/{stem}.mp4"))
        .ok_or_else(|| format!("invalid file name: {media_name}"))?;
    if !exists(&plain) {
        return Ok(plain);
    }
    course_rel_path(code, &format!("files/{stem}-{short}.mp4"))
        .ok_or_else(|| format!("invalid file name: {media_name}"))
}
