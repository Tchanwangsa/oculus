//! Getting a link's player properties with as few launches as possible: each
//! launch records a view in Echo360, and only a launch can mint a Bearer.

use std::path::Path;

use super::state::EmbedBearer;
use crate::db::store::EchoEmbed;
use crate::library::paths;
use crate::sources::canvas::Canvas;
use crate::sources::echo360::embed::{self, EmbedInfo, PropertiesError};

pub(super) struct Resolved {
    pub media_id: String,
    pub secure_link_id: String,
    pub info: EmbedInfo,
}

impl Resolved {
    /// The cache row for this resolution; `file_rel` is left to
    /// `set_echo_embed_file`.
    pub fn record(&self, link_id: &str, subject_id: i64, poster_rel: Option<String>) -> EchoEmbed {
        EchoEmbed {
            link_id: link_id.to_string(),
            subject_id,
            media_id: self.media_id.clone(),
            secure_link_id: self.secure_link_id.clone(),
            name: self.info.name.clone(),
            duration_s: self.info.duration_s,
            restrict_download: self.info.restrict_download,
            has_captions: self.info.captions_url.is_some(),
            poster_rel,
            file_rel: None,
            extra_sources: i64::from(self.info.extra_sources),
        }
    }
}

/// `known` is the link's cached (media id, secure-link id). With those and a
/// usable Bearer, no launch runs. Otherwise the link is launched; a Bearer
/// the properties call turns down is replaced once, from that launch's
/// session, which is redeemed at once because it goes stale in minutes.
pub(super) fn properties_for(
    data_dir: &Path,
    retrieve: &str,
    known: Option<(String, String)>,
    bearer: &EmbedBearer,
) -> Result<Resolved, String> {
    let with = |jwt: &str, media_id: &str, sl: &str| {
        embed::properties(jwt, sl, media_id).map(|info| Resolved {
            media_id: media_id.to_string(),
            secure_link_id: sl.to_string(),
            info,
        })
    };
    if let (Some((media_id, sl)), Some(b)) = (&known, bearer.usable()) {
        match with(&b.jwt, media_id, sl) {
            Err(PropertiesError::Rejected(_)) => bearer.clear(),
            other => return other.map_err(String::from),
        }
    }
    let agent = embed::agent();
    let launch = embed::launch(&Canvas::open(data_dir), &agent, retrieve)?;
    if let Some(b) = bearer.usable() {
        match with(&b.jwt, &launch.media_id, &launch.secure_link_id) {
            Err(PropertiesError::Rejected(_)) => bearer.clear(),
            other => return other.map_err(String::from),
        }
    }
    let fresh = embed::redeem(&agent, &launch.session_id)?;
    bearer.set(fresh.clone());
    with(&fresh.jwt, &launch.media_id, &launch.secure_link_id).map_err(String::from)
}

/// Save the poster as `courses/<code>/images/echo-<mediaId>.jpg`, since its
/// signed URL expires; the data-dir-relative path, or `None` without one.
pub(super) fn save_poster(
    data_dir: &Path,
    code: &str,
    resolved: &Resolved,
) -> Result<Option<String>, String> {
    let Some(url) = &resolved.info.poster_url else {
        return Ok(None);
    };
    let bytes = embed::get_bytes(
        &embed::sign(url, &resolved.info.query_strings)?,
        "the poster",
    )?;
    let rel = paths::course_rel_path(code, &format!("images/echo-{}.jpg", resolved.media_id))
        .ok_or("invalid poster path")?;
    let dest = data_dir.join(&rel);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = dest.with_extension("jpg.part");
    crate::runtime::atomic_write::write(&dest, &tmp, &bytes)?;
    Ok(Some(rel))
}
