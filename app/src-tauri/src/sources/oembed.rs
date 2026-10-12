//! YouTube and Vimeo metadata for a `<web-video>` card, from each site's
//! public oEmbed endpoint. Fetched here rather than in the webview, which
//! CORS would refuse. The id is checked before any URL is built, so a
//! crafted tag can only ever ask one of the two endpoints about one video.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// What the card shows; every field the provider left out is None.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WebVideoInfo {
    pub title: Option<String>,
    pub author_name: Option<String>,
    pub thumbnail_url: Option<String>,
    /// Vimeo's `duration`; YouTube's oEmbed has none.
    pub duration_s: Option<f64>,
}

#[derive(Deserialize)]
struct OEmbed {
    title: Option<String>,
    author_name: Option<String>,
    thumbnail_url: Option<String>,
    duration: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    YouTube,
    Vimeo,
}

impl Provider {
    fn parse(name: &str) -> Result<Self, String> {
        match name {
            "youtube" => Ok(Self::YouTube),
            "vimeo" => Ok(Self::Vimeo),
            _ => Err(format!("unknown video provider {name:?}")),
        }
    }

    fn valid_id(self, id: &str) -> bool {
        match self {
            Self::YouTube => {
                (6..=20).contains(&id.len())
                    && id
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            }
            Self::Vimeo => (1..=20).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit()),
        }
    }
}

/// The oEmbed endpoint for one video; the id and hash are checked first, so
/// they go in unescaped. `hash` is an unlisted Vimeo video's key.
fn oembed_url(provider: &str, video_id: &str, hash: Option<&str>) -> Result<String, String> {
    let provider = Provider::parse(provider)?;
    if !provider.valid_id(video_id) {
        return Err(format!("not a valid {provider:?} video id: {video_id:?}"));
    }
    let hash = hash.filter(|h| !h.is_empty());
    if let Some(h) = hash {
        let alnum = h.len() <= 40 && h.bytes().all(|b| b.is_ascii_alphanumeric());
        if provider != Provider::Vimeo || !alnum {
            return Err(format!("not a valid {provider:?} video hash: {h:?}"));
        }
    }
    Ok(match provider {
        Provider::YouTube => format!(
            "https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v={video_id}&format=json"
        ),
        Provider::Vimeo => {
            let path = match hash {
                Some(h) => format!("{video_id}/{h}"),
                None => video_id.to_string(),
            };
            format!("https://vimeo.com/api/oembed.json?url=https://vimeo.com/{path}")
        }
    })
}

fn fetch(url: &str) -> Result<WebVideoInfo, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .build();
    let text = agent
        .get(url)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(code, _) => format!("{url} answered {code}"),
            ureq::Error::Transport(t) => format!("could not reach {url}: {t}"),
        })?
        .into_string()
        .map_err(|e| format!("could not read {url}: {e}"))?;
    let body: OEmbed =
        serde_json::from_str(&text).map_err(|e| format!("{url} sent no oEmbed JSON: {e}"))?;
    Ok(WebVideoInfo {
        title: body.title.filter(|t| !t.trim().is_empty()),
        author_name: body.author_name.filter(|a| !a.trim().is_empty()),
        thumbnail_url: body.thumbnail_url.filter(|u| u.starts_with("https://")),
        duration_s: body.duration.filter(|d| *d > 0.0),
    })
}

/// One video's title, channel, thumbnail and (Vimeo only) length.
#[tauri::command]
pub async fn web_video_info(
    provider: String,
    video_id: String,
    hash: Option<String>,
) -> Result<WebVideoInfo, String> {
    let url = oembed_url(&provider, &video_id, hash.as_deref())?;
    crate::runtime::blocking::run(move || {
        fetch(&url).inspect_err(|e| elog!("[oculus] oembed: {e}"))
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_each_providers_endpoint() {
        assert_eq!(
            oembed_url("youtube", "ECZUvBvPZoM", None).unwrap(),
            "https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=ECZUvBvPZoM&format=json"
        );
        assert_eq!(
            oembed_url("vimeo", "123456", None).unwrap(),
            "https://vimeo.com/api/oembed.json?url=https://vimeo.com/123456"
        );
    }

    #[test]
    fn refuses_unknown_providers_and_bad_ids() {
        assert!(oembed_url("dailymotion", "x7tgad0", None).is_err());
        assert!(oembed_url("YouTube", "ECZUvBvPZoM", None).is_err());
        for bad in [
            "",
            "abc",
            "ECZUvBvPZoM&x=1",
            "a/b/../c1",
            "id with space",
            "aaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(oembed_url("youtube", bad, None).is_err(), "{bad}");
        }
        for bad in ["", "12a", "123/456", "-1", "123456789012345678901"] {
            assert!(oembed_url("vimeo", bad, None).is_err(), "{bad}");
        }
        assert!(oembed_url("youtube", "a-b_C9d", None).is_ok());
    }

    #[test]
    fn an_unlisted_vimeo_hash_joins_the_url() {
        assert_eq!(
            oembed_url("vimeo", "123456", Some("9f8e7d")).unwrap(),
            "https://vimeo.com/api/oembed.json?url=https://vimeo.com/123456/9f8e7d"
        );
        assert!(oembed_url("vimeo", "123456", Some("9f/../x")).is_err());
        assert!(oembed_url("youtube", "ECZUvBvPZoM", Some("9f8e7d")).is_err());
    }
}
