//! How a sign-in outcome and the saved-credentials status travel between
//! keyd and its clients, so the client rebuilds the exact `LoginError` and
//! its `Display` text matches an in-process sign-in byte for byte.

use serde_json::{json, Value};

use super::LoginError;

/// Which credentials are on file, for Settings; the password and the seed
/// never leave keyd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OktaStatus {
    pub username: Option<String>,
    pub has_password: bool,
    pub has_totp: bool,
}

impl OktaStatus {
    pub fn to_wire(&self) -> Value {
        json!({
            "username": self.username,
            "has_password": self.has_password,
            "has_totp": self.has_totp,
        })
    }

    pub fn from_wire(v: &Value) -> Option<OktaStatus> {
        Some(OktaStatus {
            username: match v.get("username")? {
                Value::Null => None,
                Value::String(s) => Some(s.clone()),
                _ => return None,
            },
            has_password: v.get("has_password")?.as_bool()?,
            has_totp: v.get("has_totp")?.as_bool()?,
        })
    }
}

/// An outcome as keyd's reply: `{"result":"signed_in"}` or
/// `{"result":"error","code":…}` with the variant's payload beside it
/// (`detail`, `wait_secs` or `factors`). The session cookie a success holds
/// stays in keyd: it is in the cookie files, never in a reply.
pub fn outcome_to_wire(outcome: &Result<String, LoginError>) -> Value {
    match outcome {
        Ok(_) => json!({"result": "signed_in"}),
        Err(e) => {
            let mut v = json!({"result": "error", "code": e.code()});
            match e {
                LoginError::NotConfigured | LoginError::SignedOut => {}
                LoginError::UnreadableCredentials(d)
                | LoginError::BadPassword(d)
                | LoginError::BadTotp(d)
                | LoginError::Locked(d)
                | LoginError::Network(d)
                | LoginError::Unexpected(d)
                | LoginError::Broker(d)
                | LoginError::Paused(d) => v["detail"] = json!(d),
                LoginError::UnsupportedFactor(factors) => v["factors"] = json!(factors),
                LoginError::Waiting(secs) => v["wait_secs"] = json!(secs),
            }
            v
        }
    }
}

/// `None` when `v` is not an outcome this code knows (a newer keyd's code, or
/// a malformed reply).
pub fn outcome_from_wire(v: &Value) -> Option<Result<(), LoginError>> {
    match v.get("result")?.as_str()? {
        "signed_in" => Some(Ok(())),
        "error" => {
            let detail = || Some(v.get("detail")?.as_str()?.to_string());
            Some(Err(match v.get("code")?.as_str()? {
                "not_configured" => LoginError::NotConfigured,
                "signed_out" => LoginError::SignedOut,
                "unreadable_credentials" => LoginError::UnreadableCredentials(detail()?),
                "bad_password" => LoginError::BadPassword(detail()?),
                "bad_totp" => LoginError::BadTotp(detail()?),
                "locked" => LoginError::Locked(detail()?),
                "network" => LoginError::Network(detail()?),
                "unexpected" => LoginError::Unexpected(detail()?),
                "broker" => LoginError::Broker(detail()?),
                "paused" => LoginError::Paused(detail()?),
                "unsupported_factor" => LoginError::UnsupportedFactor(
                    v.get("factors")?
                        .as_array()?
                        .iter()
                        .map(|f| f.as_str().map(str::to_string))
                        .collect::<Option<_>>()?,
                ),
                "waiting" => LoginError::Waiting(v.get("wait_secs")?.as_u64()?),
                _ => return None,
            }))
        }
        _ => None,
    }
}

impl LoginError {
    /// The variant's wire name.
    pub fn code(&self) -> &'static str {
        match self {
            LoginError::NotConfigured => "not_configured",
            LoginError::SignedOut => "signed_out",
            LoginError::UnreadableCredentials(_) => "unreadable_credentials",
            LoginError::BadPassword(_) => "bad_password",
            LoginError::BadTotp(_) => "bad_totp",
            LoginError::UnsupportedFactor(_) => "unsupported_factor",
            LoginError::Locked(_) => "locked",
            LoginError::Network(_) => "network",
            LoginError::Unexpected(_) => "unexpected",
            LoginError::Broker(_) => "broker",
            LoginError::Waiting(_) => "waiting",
            LoginError::Paused(_) => "paused",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_variant() -> Vec<LoginError> {
        vec![
            LoginError::NotConfigured,
            LoginError::SignedOut,
            LoginError::UnreadableCredentials("OSStatus -128 \"quoted\"".into()),
            LoginError::BadPassword("Password is incorrect".into()),
            LoginError::BadTotp("Invalid code — é".into()),
            LoginError::UnsupportedFactor(vec!["Okta Verify".into(), "Security Key".into()]),
            LoginError::UnsupportedFactor(vec![]),
            LoginError::Locked("Too many attempts".into()),
            LoginError::Network("dns error\nline two".into()),
            LoginError::Unexpected("identify, enroll-authenticator".into()),
            LoginError::Broker("oculus-keyd refused this program (outside the bundle)".into()),
            LoginError::Waiting(0),
            LoginError::Waiting(599),
            LoginError::Paused("Okta rejected the password: x".into()),
            LoginError::BadPassword(String::new()),
        ]
    }

    #[test]
    fn every_outcome_survives_the_wire_with_the_same_text() {
        for e in every_variant() {
            let line = outcome_to_wire(&Err(e.clone())).to_string();
            let back = outcome_from_wire(&serde_json::from_str(&line).unwrap())
                .unwrap_or_else(|| panic!("{line}"))
                .unwrap_err();
            assert_eq!(back, e, "{line}");
            assert_eq!(back.to_string(), e.to_string());
        }
        let cookie = "canvas_session=real; _csrf_token=c".to_string();
        let wire = outcome_to_wire(&Ok(cookie.clone()));
        assert_eq!(outcome_from_wire(&wire), Some(Ok(())));
        assert!(!wire.to_string().contains(&cookie));
    }

    #[test]
    fn the_wire_names_are_stable() {
        assert_eq!(
            outcome_to_wire(&Err(LoginError::Waiting(5))),
            json!({"result": "error", "code": "waiting", "wait_secs": 5})
        );
        assert_eq!(
            outcome_to_wire(&Err(LoginError::UnsupportedFactor(vec!["a".into()]))),
            json!({"result": "error", "code": "unsupported_factor", "factors": ["a"]})
        );
        assert_eq!(
            outcome_to_wire(&Err(LoginError::SignedOut)),
            json!({"result": "error", "code": "signed_out"})
        );
        assert_eq!(
            outcome_to_wire(&Ok("c".into())),
            json!({"result": "signed_in"})
        );
    }

    #[test]
    fn an_unknown_or_malformed_outcome_is_none() {
        for v in [
            json!({}),
            json!({"result": "maybe"}),
            json!({"result": "error"}),
            json!({"result": "error", "code": "teapot"}),
            json!({"result": "error", "code": "bad_password"}),
            json!({"result": "error", "code": "waiting", "wait_secs": "5"}),
            json!({"result": "error", "code": "unsupported_factor", "factors": [1]}),
        ] {
            assert_eq!(outcome_from_wire(&v), None, "{v}");
        }
    }

    #[test]
    fn the_status_survives_the_wire_and_has_no_secret_fields() {
        for status in [
            OktaStatus {
                username: Some("s1234567".into()),
                has_password: true,
                has_totp: false,
            },
            OktaStatus {
                username: None,
                has_password: false,
                has_totp: false,
            },
        ] {
            let wire = status.to_wire();
            assert_eq!(wire.as_object().unwrap().len(), 3);
            assert_eq!(OktaStatus::from_wire(&wire), Some(status));
        }
        assert_eq!(OktaStatus::from_wire(&json!({"username": 5})), None);
    }
}
