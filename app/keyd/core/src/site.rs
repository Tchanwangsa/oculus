//! The university keyd serves: its Canvas origin, where Canvas's sign-in
//! starts, the Okta origin in front of it (when its IdP is Okta) and Ed's
//! origin. The `canvas` and `ed` routes and the sign-in read it; the app sets
//! it from its institution profile with `site_set`, which drops every
//! credential meant for an origin it moves (`ops/site.rs`), so a session or a
//! password only ever goes where it was made.

use serde_json::{json, Value};

/// Where one university's services are. Origins are `https://host`, with no
/// port, path or trailing slash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub canvas: String,
    /// The path on Canvas the sign-in starts at.
    pub login_path: String,
    /// Okta's origin; `None` when Canvas does not sign in through Okta, and
    /// then nothing signs in headlessly.
    pub sso: Option<String>,
    pub ed: String,
}

impl Site {
    /// The site an install from before keyd held one was built for.
    pub fn unimelb() -> Site {
        Site {
            canvas: "https://canvas.lms.unimelb.edu.au".to_string(),
            login_path: "/login/saml".to_string(),
            sso: Some("https://sso.unimelb.edu.au".to_string()),
            ed: "https://edstem.org".to_string(),
        }
    }

    pub fn to_wire(&self) -> Value {
        json!({
            "canvas": self.canvas,
            "login_path": self.login_path,
            "sso": self.sso,
            "ed": self.ed,
        })
    }

    /// `None` when a field is missing or the wrong type; `check` judges the
    /// values.
    pub fn from_wire(v: &Value) -> Option<Site> {
        Some(Site {
            canvas: v.get("canvas")?.as_str()?.to_string(),
            login_path: v.get("login_path")?.as_str()?.to_string(),
            sso: match v.get("sso")? {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                _ => return None,
            },
            ed: v.get("ed")?.as_str()?.to_string(),
        })
    }

    /// Why this cannot be a site: an origin that is not a bare `https://host`,
    /// or a sign-in path that could leave Canvas's root.
    pub fn check(&self) -> Result<(), String> {
        check_origin("canvas", &self.canvas)?;
        check_origin("ed", &self.ed)?;
        if let Some(sso) = &self.sso {
            check_origin("sso", sso)?;
        }
        check_login_path(&self.login_path)
    }

    /// Okta's host, which keys the sign-in's cookie jar.
    pub fn sso_host(&self) -> Option<String> {
        self.sso.as_deref().and_then(host_of)
    }
}

pub fn host_of(origin: &str) -> Option<String> {
    url::Url::parse(origin)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
}

fn check_origin(field: &str, origin: &str) -> Result<(), String> {
    let refused = || format!("{field} must be https://host with nothing after it");
    let url = url::Url::parse(origin).map_err(|_| refused())?;
    let host = match url.host() {
        Some(url::Host::Domain(host)) if host.contains('.') => host,
        _ => return Err(refused()),
    };
    if url.scheme() != "https" || origin != format!("https://{host}") {
        return Err(refused());
    }
    Ok(())
}

fn check_login_path(path: &str) -> Result<(), String> {
    let usable = path.starts_with('/')
        && path.len() <= 256
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
        && !path.contains("//")
        && !path.split('/').any(|seg| seg == "." || seg == "..");
    if usable {
        Ok(())
    } else {
        Err("login_path must be a plain path on Canvas".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unimelb_is_a_valid_site_and_round_trips() {
        let site = Site::unimelb();
        site.check().unwrap();
        assert_eq!(Site::from_wire(&site.to_wire()), Some(site.clone()));
        assert_eq!(site.sso_host().as_deref(), Some("sso.unimelb.edu.au"));

        let no_okta = Site { sso: None, ..site };
        no_okta.check().unwrap();
        assert_eq!(Site::from_wire(&no_okta.to_wire()), Some(no_okta));
    }

    #[test]
    fn an_origin_is_a_bare_https_host() {
        for bad in [
            "http://canvas.foo.edu",
            "https://canvas.foo.edu/",
            "https://canvas.foo.edu/x",
            "https://canvas.foo.edu:8443",
            "https://user@canvas.foo.edu",
            "https://canvas.foo.edu?x=1",
            "https://127.0.0.1",
            "https://localhost",
            "https://CANVAS.foo.edu",
            "canvas.foo.edu",
            "",
        ] {
            let site = Site {
                canvas: bad.to_string(),
                ..Site::unimelb()
            };
            assert!(site.check().is_err(), "{bad}");
            let site = Site {
                sso: Some(bad.to_string()),
                ..Site::unimelb()
            };
            assert!(site.check().is_err(), "sso {bad}");
        }
    }

    #[test]
    fn the_login_path_stays_a_plain_path() {
        for good in ["/login", "/login/saml", "/login/openid_connect"] {
            let site = Site {
                login_path: good.to_string(),
                ..Site::unimelb()
            };
            site.check().unwrap();
        }
        for bad in [
            "login",
            "//evil.example/x",
            "/login/../x",
            "/login/./x",
            "/login?next=x",
            "/login%2f",
            "/ログイン",
            "",
        ] {
            let site = Site {
                login_path: bad.to_string(),
                ..Site::unimelb()
            };
            assert!(site.check().is_err(), "{bad}");
        }
    }

    #[test]
    fn a_malformed_wire_site_is_none() {
        let mut v = Site::unimelb().to_wire();
        v["sso"] = json!(5);
        assert_eq!(Site::from_wire(&v), None);
        for field in ["canvas", "login_path", "sso", "ed"] {
            let mut v = Site::unimelb().to_wire();
            v.as_object_mut().unwrap().remove(field);
            assert_eq!(Site::from_wire(&v), None, "{field}");
        }
    }
}
