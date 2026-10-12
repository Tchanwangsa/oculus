//! A link's player properties: its name, length, download policy, captions,
//! poster, HLS playlists and the query strings that sign CDN URLs. Every
//! field is read leniently (missing or null is the default), since only a
//! few responses have been seen.

use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryString {
    pub uri_pattern: String,
    pub query_string: String,
}

/// What Oculus uses of one link's player properties. URLs are unsigned.
#[derive(Debug, Clone, PartialEq)]
pub struct EmbedInfo {
    pub name: String,
    pub duration_s: Option<f64>,
    pub restrict_download: bool,
    pub captions_url: Option<String>,
    pub poster_url: Option<String>,
    /// The HLS master of the lowest-numbered source with a video track.
    pub video_master: Option<String>,
    /// The audio-only HLS master.
    pub audio_master: Option<String>,
    /// Video sources beyond the one taken (a room camera, say).
    pub extra_sources: u32,
    pub query_strings: Vec<QueryString>,
}

/// Why the properties call failed: `Rejected` is the Bearer turned down
/// (401/403), which a fresh launch can fix.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertiesError {
    Rejected(u16),
    Failed(String),
}

impl From<PropertiesError> for String {
    fn from(error: PropertiesError) -> String {
        match error {
            PropertiesError::Rejected(code) => {
                format!("Echo360 turned down the player token (HTTP {code})")
            }
            PropertiesError::Failed(why) => why,
        }
    }
}

pub fn fetch(
    jwt: &str,
    secure_link_id: &str,
    media_id: &str,
) -> Result<EmbedInfo, PropertiesError> {
    let url = format!(
        "https://echo360.net.au/api/ui/echoplayer/secure-link-access-datas/{secure_link_id}/media/{media_id}/player-properties"
    );
    let resp = match ureq::get(&url)
        .set("Authorization", &format!("Bearer {jwt}"))
        .set("Accept", "application/json")
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code @ (401 | 403), _)) => {
            return Err(PropertiesError::Rejected(code))
        }
        Err(ureq::Error::Status(code, _)) => {
            return Err(PropertiesError::Failed(format!(
                "Echo360 answered HTTP {code} for the video's details"
            )))
        }
        Err(e) => {
            return Err(PropertiesError::Failed(format!(
                "Echo360 video details request failed: {e}"
            )))
        }
    };
    let body = resp
        .into_string()
        .map_err(|e| PropertiesError::Failed(format!("Echo360 video details unreadable: {e}")))?;
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| PropertiesError::Failed(format!("Echo360 video details are not JSON: {e}")))?;
    parse(&json).map_err(PropertiesError::Failed)
}

/// `{"data": {...}}`, or the object itself.
pub(super) fn parse(json: &serde_json::Value) -> Result<EmbedInfo, String> {
    let data = json.get("data").filter(|d| d.is_object()).unwrap_or(json);
    let raw = Raw::deserialize(data)
        .map_err(|e| format!("Echo360 video details have an unexpected shape: {e}"))?;
    let (video_master, audio_master, extra_sources) =
        pick_masters(&raw.playable_audio_video.playable_medias);
    Ok(EmbedInfo {
        name: raw.media_name.trim().to_string(),
        duration_s: match &raw.playable_audio_video.duration {
            serde_json::Value::String(s) => iso_duration(s),
            other => other.as_f64(),
        },
        restrict_download: raw.copyright_data.restrict_download,
        captions_url: uri_in(&raw.captions),
        poster_url: raw
            .poster_media
            .iter()
            .find(|p| !p.uri.is_empty())
            .map(|p| p.uri.clone()),
        video_master,
        audio_master,
        extra_sources,
        query_strings: raw.source_query_strings.query_strings,
    })
}

/// Seconds in an ISO 8601 duration such as `PT1358.393S` or `PT1H2M3S`.
/// Years and months have no fixed length and are refused.
pub(super) fn iso_duration(s: &str) -> Option<f64> {
    let rest = s.trim().strip_prefix('P')?;
    let (days, time) = match rest.split_once('T') {
        Some((d, t)) => (d, t),
        None => (rest, ""),
    };
    let mut total = 0.0;
    let mut any = false;
    for (part, units) in [
        (days, &[('W', 604_800.0), ('D', 86_400.0)][..]),
        (time, &[('H', 3_600.0), ('M', 60.0), ('S', 1.0)][..]),
    ] {
        let mut number = String::new();
        for c in part.chars() {
            if c.is_ascii_digit() || c == '.' {
                number.push(c);
                continue;
            }
            let scale = units.iter().find(|(u, _)| *u == c)?.1;
            total += number.parse::<f64>().ok()? * scale;
            number.clear();
            any = true;
        }
        if !number.is_empty() {
            return None;
        }
    }
    any.then_some(total)
}

/// A URL given as a string, an object with `uri`/`url`, or a list of either.
fn uri_in(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Array(items) => items.iter().find_map(uri_in),
        serde_json::Value::Object(o) => o.get("uri").or_else(|| o.get("url")).and_then(uri_in),
        _ => None,
    }
}

/// The video master (lowest source index with a video track, video-only
/// preferred), the audio-only master, and how many other video sources exist.
fn pick_masters(medias: &[RawMedia]) -> (Option<String>, Option<String>, u32) {
    let hls: Vec<(&RawMedia, i64, bool, bool)> = medias
        .iter()
        .filter(|m| {
            m.uri
                .split(['?', '#'])
                .next()
                .is_some_and(|p| p.ends_with(".m3u8"))
        })
        .map(|m| {
            let kind = match &m.track_type {
                serde_json::Value::Array(parts) => parts
                    .iter()
                    .filter_map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join("+"),
                serde_json::Value::String(s) => s.clone(),
                _ => String::new(),
            }
            .to_ascii_lowercase();
            let index = m
                .source_index
                .as_i64()
                .or_else(|| m.source_index.as_str().and_then(|s| s.parse().ok()))
                .unwrap_or(i64::MAX);
            (m, index, kind.contains("audio"), kind.contains("video"))
        })
        .collect();

    let mut video_sources: Vec<i64> = hls.iter().filter(|m| m.3).map(|m| m.1).collect();
    video_sources.sort_unstable();
    video_sources.dedup();
    let video = video_sources.first().and_then(|&lowest| {
        let of_source = hls.iter().filter(|m| m.3 && m.1 == lowest);
        of_source
            .clone()
            .find(|m| !m.2)
            .or_else(|| of_source.clone().next())
            .map(|m| m.0.uri.clone())
    });
    let audio = hls
        .iter()
        .filter(|m| m.2 && !m.3)
        .min_by_key(|m| m.1)
        .map(|m| m.0.uri.clone());
    let extra = video_sources.len().saturating_sub(1) as u32;
    (video, audio, extra)
}

fn nullable<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(d).map(Option::unwrap_or_default)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Raw {
    #[serde(deserialize_with = "nullable")]
    media_name: String,
    #[serde(deserialize_with = "nullable")]
    playable_audio_video: RawAudioVideo,
    #[serde(deserialize_with = "nullable")]
    copyright_data: RawCopyright,
    captions: serde_json::Value,
    #[serde(deserialize_with = "nullable")]
    poster_media: Vec<RawUri>,
    #[serde(deserialize_with = "nullable")]
    source_query_strings: RawQueryStrings,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawAudioVideo {
    duration: serde_json::Value,
    #[serde(deserialize_with = "nullable")]
    playable_medias: Vec<RawMedia>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawMedia {
    #[serde(deserialize_with = "nullable")]
    uri: String,
    source_index: serde_json::Value,
    track_type: serde_json::Value,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawCopyright {
    #[serde(deserialize_with = "nullable")]
    restrict_download: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawUri {
    #[serde(deserialize_with = "nullable")]
    uri: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawQueryStrings {
    #[serde(deserialize_with = "nullable")]
    query_strings: Vec<QueryString>,
}
