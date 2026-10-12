//! The two fMP4 files behind an embed's playlists: the best video and the
//! best audio, each signed and ready to fetch whole.

use url::Url;

use super::cdn;
use super::playlist::{best_variant, single_file, UNSUPPORTED};
use super::properties::{EmbedInfo, QueryString};
use super::sign::sign;

/// Signed URLs; they expire, so they are found right before a download.
#[derive(Debug, Clone)]
pub struct MediaFiles {
    pub video: String,
    pub audio: String,
}

pub fn media_files(info: &EmbedInfo) -> Result<MediaFiles, String> {
    let video = info
        .video_master
        .as_deref()
        .ok_or_else(|| format!("{UNSUPPORTED} (no HLS video stream)."))?;
    let audio = info
        .audio_master
        .as_deref()
        .ok_or_else(|| format!("{UNSUPPORTED} (no separate audio stream)."))?;
    Ok(MediaFiles {
        video: stream_file(video, &info.query_strings, "the video playlist")?,
        audio: stream_file(audio, &info.query_strings, "the audio playlist")?,
    })
}

/// Master → highest-bandwidth variant → the file its map names, signed.
fn stream_file(master: &str, patterns: &[QueryString], what: &str) -> Result<String, String> {
    let master_url =
        Url::parse(master).map_err(|e| format!("Echo360 gave {what} as a non-URL: {e}"))?;
    let master_text = cdn::get_text(&sign(master, patterns)?, what)?;
    let (variant_url, variant_text) = match best_variant(&master_text)? {
        Some(uri) => {
            let url = master_url
                .join(&uri)
                .map_err(|e| format!("A variant in {what} is not a URL: {e}"))?;
            let text = cdn::get_text(&sign(url.as_str(), patterns)?, what)?;
            (url, text)
        }
        None => (master_url, master_text),
    };
    let file = single_file(&variant_text, &variant_url)?;
    sign(file.as_str(), patterns)
}
