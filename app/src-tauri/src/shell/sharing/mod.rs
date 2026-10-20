//! The desktop Share action opens the OS picker; choosing its destination and
//! completing or cancelling the share stay entirely in AppKit.

#[cfg(target_os = "macos")]
mod macos;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareAnchor {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_width: f64,
    viewport_height: f64,
}

impl ShareAnchor {
    fn validate(&self) -> Result<(), String> {
        let values = [
            self.x,
            self.y,
            self.width,
            self.height,
            self.viewport_width,
            self.viewport_height,
        ];
        if values.iter().any(|v| !v.is_finite())
            || self.width <= 0.0
            || self.height <= 0.0
            || self.viewport_width <= 0.0
            || self.viewport_height <= 0.0
        {
            return Err(
                "The share button has no visible position. Try opening the dialog again.".into(),
            );
        }
        Ok(())
    }
}

fn validate_link(link: &str) -> Result<(), String> {
    let valid = link.len() <= 8192
        && !link.chars().any(char::is_control)
        && url::Url::parse(link).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        });
    if valid {
        Ok(())
    } else {
        Err("This link cannot be shared. Open the canvas sharing dialog again.".into())
    }
}

#[tauri::command]
pub async fn native_share_link(
    webview: tauri::Webview,
    link: String,
    anchor: ShareAnchor,
) -> Result<(), String> {
    if webview.label() != super::browser::MAIN {
        return Err("Sharing is only available from Oculus.".into());
    }
    validate_link(&link)?;
    anchor.validate()?;
    #[cfg(target_os = "macos")]
    {
        macos::show(webview, link, anchor).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (webview, link, anchor);
        Err("Native sharing is available on macOS.".into())
    }
}

#[tauri::command]
pub async fn native_share_close(webview: tauri::Webview) -> Result<(), String> {
    if webview.label() != super::browser::MAIN {
        return Err("Sharing is only available from Oculus.".into());
    }
    #[cfg(target_os = "macos")]
    {
        webview
            .with_webview(|_| macos::close())
            .map_err(|_| "Could not close the share picker.".to_owned())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_http_and_https_urls_with_fragments_but_refuses_other_schemes_and_credentials() {
        for link in [
            "http://192.168.1.2:43821/c/#secret",
            "http://[fd7a::1]:43821/c/#secret",
            "https://example.com/c/#secret",
        ] {
            assert!(validate_link(link).is_ok());
        }
        for link in [
            "file:///private/tmp/secret",
            "javascript:alert(1)",
            "http://user:password@192.168.1.2/",
            "http://example.com/\nsecret",
            "not a link",
        ] {
            assert!(validate_link(link).is_err());
        }
    }

    #[test]
    fn invisible_or_non_finite_anchors_are_refused() {
        let mut anchor = ShareAnchor {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 32.,
            viewport_width: 1000.,
            viewport_height: 700.,
        };
        assert!(anchor.validate().is_ok());
        anchor.viewport_width = 0.;
        assert!(anchor.validate().is_err());
        anchor.viewport_width = f64::NAN;
        assert!(anchor.validate().is_err());
    }
}
