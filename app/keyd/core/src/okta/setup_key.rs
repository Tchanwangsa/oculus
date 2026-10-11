//! The TOTP setup key as a student pastes it: the key Okta shows under
//! *Can't scan?* (spaced, lowercase groups), or the whole `otpauth://` link a
//! QR code holds, of which only `secret=` is kept. Google Authenticator's
//! export link carries no single key and is refused by name.

use super::totp::base32_decode;

pub(super) const BAD_SEED: &str =
    "That does not look like a TOTP setup key: it may only contain the letters \
     A–Z and the digits 2–7.";

const EXPORT_LINK: &str =
    "That is a Google Authenticator export link, not a setup key. Paste the key \
     Okta shows under \u{201c}Can't scan?\u{201d} instead.";

/// The bare base32 secret in `input`, with spaces removed. An `otpauth://`
/// link (any case of the scheme) gives its percent-decoded `secret`
/// parameter; one without it is a bad key. Errors never hold any of `input`:
/// the decoder's own error names the offending character, and a key is a
/// secret.
pub fn setup_key(input: &str) -> Result<String, String> {
    let input = input.trim();
    let key = if has_scheme(input, "otpauth-migration://") {
        return Err(EXPORT_LINK.to_string());
    } else if has_scheme(input, "otpauth://") {
        secret_param(input).ok_or_else(|| BAD_SEED.to_string())?
    } else {
        input.to_string()
    };
    let key = key.trim().replace(' ', "");
    base32_decode(&key).map_err(|_| BAD_SEED.to_string())?;
    Ok(key)
}

fn has_scheme(input: &str, scheme: &str) -> bool {
    input
        .get(..scheme.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
}

/// The link's `secret` query parameter, percent-decoded. Every other
/// parameter (`issuer`, `algorithm`, `digits`, `period`) is ignored: Okta's
/// factor is SHA-1, six digits, thirty seconds, which `totp_code` assumes.
fn secret_param(uri: &str) -> Option<String> {
    let (_, query) = uri.split_once('?')?;
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(name, _)| name == "secret")
        .map(|(_, value)| value.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "JBSWY3DPEHPK3PXP";

    #[test]
    fn a_plain_key_is_kept_and_a_spaced_lowercase_one_loses_its_spaces() {
        assert_eq!(setup_key(KEY).unwrap(), KEY);
        assert_eq!(
            setup_key("  jbsw y3dp ehpk 3pxp ").unwrap(),
            "jbswy3dpehpk3pxp"
        );
    }

    #[test]
    fn an_otpauth_link_gives_only_its_secret() {
        let link =
            format!("otpauth://totp/UniMelb:me@student.unimelb.edu.au?secret={KEY}&issuer=UniMelb");
        assert_eq!(setup_key(&link).unwrap(), KEY);
        // The scheme in any case, `secret` after other parameters, spaces
        // around the paste.
        let link = format!(" OTPAuth://totp/x?issuer=UniMelb&digits=6&secret={KEY} ");
        assert_eq!(setup_key(&link).unwrap(), KEY);
    }

    #[test]
    fn a_link_is_percent_decoded() {
        let link = "otpauth://totp/University%20of%20Melbourne%3Ame%40student.unimelb.edu.au\
                    ?secret=jbsw%20y3dp%20ehpk%203pxp&issuer=University%20of%20Melbourne";
        assert_eq!(setup_key(link).unwrap(), "jbswy3dpehpk3pxp");
    }

    #[test]
    fn a_link_without_a_usable_secret_is_a_bad_key() {
        for link in [
            "otpauth://totp/UniMelb:me@student.unimelb.edu.au?issuer=UniMelb",
            "otpauth://totp/UniMelb:me@student.unimelb.edu.au",
            "otpauth://totp/UniMelb?secret=&issuer=UniMelb",
            "otpauth://totp/UniMelb?secret=NOT-BASE32-1890",
        ] {
            assert_eq!(setup_key(link).unwrap_err(), BAD_SEED, "{link}");
        }
    }

    #[test]
    fn an_export_link_is_refused_as_what_it_is_without_echoing_it() {
        let link = "otpauth-migration://offline?data=CjEKCkhlbGxvId6tvu8SGEV4YW1wbGU%3D";
        let message = setup_key(link).unwrap_err();
        assert!(message.contains("export link"), "{message}");
        for part in ["offline", "data", "CjEK", "%3D"] {
            assert!(!message.contains(part), "{part:?} echoed in {message}");
        }
        assert_eq!(
            setup_key("OTPAUTH-MIGRATION://offline?data=x").unwrap_err(),
            message
        );
    }
}
