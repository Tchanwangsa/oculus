//! Where Canvas's sign-in leads with no session, for the app's detection of a
//! university's identity provider.

use super::http::{state_token_candidates, walk};
use crate::okta::jar::Jar;

/// Follows `start`'s redirects with no session and reports where they land
/// and whether that page is Okta's sign-in widget.
pub fn probe_sign_in_page(start: &str) -> Result<(url::Url, bool), String> {
    let mut jar = Jar::default();
    let (landed, body) = walk(&mut jar, start, 10).map_err(|e| e.to_string())?;
    Ok((landed, is_okta_page(&body)))
}

fn is_okta_page(html: &str) -> bool {
    html.contains("okta-signin-widget")
        || (html.contains("OktaUtil") && !state_token_candidates(html).is_empty())
}

#[cfg(test)]
mod tests {
    use super::is_okta_page;

    #[test]
    fn okta_is_known_by_its_widget_or_its_state_token() {
        assert!(is_okta_page(
            r#"<div id="okta-signin-widget-container"></div>"#
        ));
        assert!(is_okta_page(
            r#"<script>OktaUtil.x(); var config = {"stateToken":"00abcDEF123456789_-.~xyzXYZ0123"};</script>"#
        ));
        assert!(!is_okta_page("<script>OktaUtil.x();</script>"));
        assert!(!is_okta_page(
            "<form action=/idp/profile/SAML2/Redirect/SSO>"
        ));
    }
}
