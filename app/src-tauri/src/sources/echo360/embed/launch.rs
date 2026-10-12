//! Launching one embed: Canvas signs the LTI form, and Echo360 answers its
//! POST with a redirect to the player naming the media, the secure link and a
//! session to redeem for a Bearer token. Unlike the course launch, no
//! `ECHO_JWT` cookie comes back; the only cookie is `PLAY_SESSION`.

use url::Url;

use super::super::auth::{parse_lti_form, UA};
use super::link::is_safe_id;
use super::region::is_region;
use crate::sources::canvas::Canvas;

/// What one launch learns. The media and secure-link ids are stable per link;
/// the session id goes stale within minutes, so it is redeemed at once.
#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    /// `https://host` of the Echo360 region the player is on.
    pub base: String,
    pub media_id: String,
    pub secure_link_id: String,
    pub session_id: String,
}

/// An agent for one launch: redirects off, so the player redirect can be
/// read, and its own cookie jar, which carries `PLAY_SESSION` from the POST
/// to the session redeem.
pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().redirects(0).build()
}

/// GET the retrieve URL through Canvas (oculus-keyd attaches the session),
/// then POST its form to Echo360. A lapsed Canvas session surfaces as
/// Canvas's own error, or as a page with no form.
pub fn launch(canvas: &Canvas, agent: &ureq::Agent, retrieve: &str) -> Result<Launch, String> {
    let page = canvas.get(retrieve)?;
    if !page.ok() {
        return Err(format!(
            "Canvas answered HTTP {} for the video's launch page",
            page.status
        ));
    }
    let (action, fields) =
        parse_lti_form(&String::from_utf8_lossy(&page.body)).ok_or_else(|| {
            "Could not find the Echo360 launch form on the Canvas page — the Canvas session \
         may have lapsed (sign in again from Settings → Canvas)"
                .to_string()
        })?;
    let action_url = Url::parse(&action)
        .map_err(|e| format!("The Echo360 launch form's target is not a URL: {e}"))?;
    if action_url.scheme() != "https" || !action_url.host_str().is_some_and(is_region) {
        return Err("The Canvas launch form does not post to Echo360".to_string());
    }

    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(&fields)
        .finish();
    let resp = match agent
        .post(&action)
        .set("Content-Type", "application/x-www-form-urlencoded")
        .set("Referer", retrieve)
        .set("User-Agent", UA)
        .send_string(&body)
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(code, _)) => {
            return Err(format!("Echo360 refused the video launch (HTTP {code})"))
        }
        Err(e) => return Err(format!("Echo360 video launch failed: {e}")),
    };
    let status = resp.status();
    if !(300..400).contains(&status) {
        return Err(format!(
            "Echo360 answered the video launch with HTTP {status}, not a redirect to its player"
        ));
    }
    let location = resp
        .header("location")
        .ok_or("Echo360's launch redirect has no Location")?;
    let player = action_url
        .join(location)
        .map_err(|e| format!("Echo360's launch redirect is not a URL: {e}"))?;
    parse_player_url(&player)
}

/// `https://<region>/ui/player/<mediaId>?sessionId=<s>&secureLinkAccessDataId=<sl>&…`
pub(super) fn parse_player_url(url: &Url) -> Result<Launch, String> {
    let segments: Vec<&str> = url
        .path_segments()
        .map(Iterator::collect)
        .unwrap_or_default();
    let media_id = segments
        .iter()
        .position(|&s| s == "player")
        .and_then(|i| segments.get(i + 1))
        .map(|s| s.to_string())
        .unwrap_or_default();
    let param = |name: &str| {
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default()
    };
    let launch = Launch {
        base: url.origin().ascii_serialization(),
        media_id,
        secure_link_id: param("secureLinkAccessDataId"),
        session_id: param("sessionId"),
    };
    if [&launch.media_id, &launch.secure_link_id, &launch.session_id]
        .iter()
        .all(|id| is_safe_id(id))
    {
        Ok(launch)
    } else {
        Err(format!(
            "Echo360's launch redirect did not name a media, secure link and session: {}",
            url.path()
        ))
    }
}
