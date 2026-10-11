//! Cookies as a client keeps them: the one rule for what a `Set-Cookie` does
//! to a stored `Cookie` header, shared by the sign-in's jar and by keyd
//! absorbing a rotation from Canvas.
//!
//! Only the leading `name=value` is read. A cookie is removed, as a browser
//! removes it, when its value is empty, its `Max-Age` is zero or negative, or
//! (without a `Max-Age`) its `Expires` is not in the future. No other
//! attribute matters: keyd talks to one origin, so path, domain and `Secure`
//! decide nothing.

use crate::clock::{days_from_civil, now_secs};

/// One `Set-Cookie`, reduced to what changes a stored header.
#[derive(Debug, PartialEq, Eq)]
pub struct SetCookie<'a> {
    pub name: &'a str,
    pub value: &'a str,
    /// The origin asked for the cookie to go.
    pub remove: bool,
}

/// `None` for a line with no usable `name=value`: no `=`, an empty name, or a
/// control or non-ASCII character that must not reach a header.
pub fn parse_set_cookie(raw: &str, now: u64) -> Option<SetCookie<'_>> {
    let mut parts = raw.split(';');
    let (name, value) = parts.next()?.split_once('=')?;
    let (name, value) = (name.trim(), value.trim());
    let header_safe = |s: &str| s.bytes().all(|b| !b.is_ascii_control() && b < 0x80);
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    if !header_safe(name) || !header_safe(value) {
        return None;
    }

    let (mut max_age, mut expires) = (None, None);
    for attribute in parts {
        let (key, text) = attribute.split_once('=').unwrap_or((attribute, ""));
        let (key, text) = (key.trim(), text.trim());
        if key.eq_ignore_ascii_case("max-age") {
            max_age = text.parse::<i64>().ok();
        } else if key.eq_ignore_ascii_case("expires") {
            expires = http_date(text);
        }
    }
    let gone = match (max_age, expires) {
        (Some(seconds), _) => seconds <= 0,
        (None, Some(at)) => at <= now,
        (None, None) => false,
    };
    Some(SetCookie {
        name,
        value,
        remove: value.is_empty() || gone,
    })
}

/// A cookie header's `name=value` pairs, in order. A piece with no `=` is
/// dropped.
pub fn parse_cookie_header(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|piece| {
            let (name, value) = piece.trim().split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| (name.to_string(), value.trim().to_string()))
        })
        .collect()
}

/// `header` after the origin's `set_cookies`: a rotated cookie keeps its
/// place, a new one goes last, a removed one is gone. `None` when nothing
/// changed, so the caller need not write, and for an empty `header`: with no
/// session to rotate, a stray cookie must not make one. `Some("")` means
/// every cookie was removed.
pub fn merge_set_cookie(header: &str, set_cookies: &[impl AsRef<str>]) -> Option<String> {
    merge_at(header, set_cookies, now_secs())
}

pub(crate) fn merge_at(header: &str, set_cookies: &[impl AsRef<str>], now: u64) -> Option<String> {
    if header.trim().is_empty() {
        return None;
    }
    let mut pairs = parse_cookie_header(header);
    let mut changed = false;
    for raw in set_cookies {
        let Some(cookie) = parse_set_cookie(raw.as_ref(), now) else {
            continue;
        };
        if cookie.remove {
            let before = pairs.len();
            pairs.retain(|(name, _)| name != cookie.name);
            changed |= pairs.len() != before;
            continue;
        }
        match pairs.iter_mut().find(|(name, _)| name == cookie.name) {
            Some((_, value)) if value == cookie.value => {}
            Some((_, value)) => {
                *value = cookie.value.to_string();
                changed = true;
            }
            None => {
                pairs.push((cookie.name.to_string(), cookie.value.to_string()));
                changed = true;
            }
        }
    }
    changed.then(|| {
        pairs
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ")
    })
}

/// An `Expires` date as Unix seconds, read the way RFC 6265 reads one: the
/// time, day, month and year are found by shape among the tokens, whatever
/// the separators and the weekday.
fn http_date(text: &str) -> Option<u64> {
    let (mut time, mut day, mut month, mut year) = (None, None, None, None);
    for token in text.split(|c: char| !c.is_ascii_alphanumeric() && c != ':') {
        if token.is_empty() {
            continue;
        }
        if time.is_none() && token.contains(':') {
            let parts: Vec<&str> = token.split(':').collect();
            if let [h, m, s] = parts[..] {
                if let (Ok(h), Ok(m), Ok(s)) =
                    (h.parse::<u64>(), m.parse::<u64>(), s.parse::<u64>())
                {
                    time = Some((h, m, s));
                    continue;
                }
            }
            return None;
        }
        let digits = token.bytes().all(|b| b.is_ascii_digit());
        if digits && day.is_none() && token.len() <= 2 {
            day = token.parse::<i64>().ok();
        } else if digits && year.is_none() && (2..=4).contains(&token.len()) {
            year = token.parse::<i64>().ok();
        } else if month.is_none() && token.len() >= 3 && token.is_char_boundary(3) {
            const MONTHS: [&str; 12] = [
                "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
            ];
            let head = token[..3].to_ascii_lowercase();
            month = MONTHS.iter().position(|m| *m == head).map(|i| i as i64 + 1);
        }
    }
    let ((h, m, s), day, month, mut year) = (time?, day?, month?, year?);
    if year < 100 {
        year += if year >= 70 { 1900 } else { 2000 };
    }
    if !(1..=31).contains(&day) || year < 1601 || h > 23 || m > 59 || s > 59 {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + (h * 3600 + m * 60 + s) as i64;
    Some(secs.max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-10T00:00:00Z.
    const NOW: u64 = 1_791_590_400;

    fn merged(header: &str, set: &[&str]) -> Option<String> {
        merge_at(header, set, NOW)
    }

    #[test]
    fn a_rotated_cookie_keeps_its_place() {
        assert_eq!(
            merged(
                "a=1; canvas_session=OLD; z=9",
                &["canvas_session=NEW; path=/; secure; httponly"]
            )
            .unwrap(),
            "a=1; canvas_session=NEW; z=9"
        );
    }

    #[test]
    fn new_names_go_last_in_the_order_they_arrive() {
        assert_eq!(
            merged("a=1", &["b=2; path=/", "c=3"]).unwrap(),
            "a=1; b=2; c=3"
        );
    }

    #[test]
    fn nothing_changed_is_none() {
        assert_eq!(merged("a=1; b=2", &["b=2; path=/"]), None);
        assert_eq!(merged("a=1", &[] as &[&str]), None);
        assert_eq!(merged("a=1", &["a=1; Max-Age=60", "b=; path=/"]), None);
    }

    #[test]
    fn no_session_means_nothing_to_rotate() {
        assert_eq!(merged("", &["a=1"]), None);
        assert_eq!(merged("  ", &["a=1"]), None);
    }

    #[test]
    fn junk_is_ignored() {
        for raw in [
            "novalue; path=/",
            "=x",
            " =x",
            "a b=c",
            "a=\u{fc}",
            "a=1\r\nb=2",
            "",
        ] {
            assert_eq!(merged("a=1", &[raw]), None, "{raw:?}");
        }
    }

    #[test]
    fn an_empty_value_removes_the_cookie_and_the_rest_keep_their_order() {
        assert_eq!(
            merged("a=1; b=2; c=3", &["b=; path=/"]).unwrap(),
            "a=1; c=3"
        );
        assert_eq!(merged("a=1", &["a=; path=/"]).unwrap(), "");
        assert_eq!(merged("a=1; a=2; b=3", &["a="]).unwrap(), "b=3");
    }

    #[test]
    fn max_age_zero_or_negative_removes_and_a_positive_one_keeps() {
        assert_eq!(merged("a=1; b=2", &["a=x; Max-Age=0"]).unwrap(), "b=2");
        assert_eq!(merged("a=1; b=2", &["a=x; max-age=-5"]).unwrap(), "b=2");
        assert_eq!(merged("a=1", &["a=x; Max-Age=3600"]).unwrap(), "a=x");
    }

    #[test]
    fn an_expires_in_the_past_removes_and_one_in_the_future_keeps() {
        let past = "a=x; Expires=Thu, 01 Jan 1970 00:00:01 GMT";
        assert_eq!(merged("a=1; b=2", &[past]).unwrap(), "b=2");
        let past_2 = "a=x; expires=Sun, 06 Nov 1994 08:49:37 GMT";
        assert_eq!(merged("a=1; b=2", &[past_2]).unwrap(), "b=2");
        let future = "a=x; Expires=Fri, 01 Jan 2100 00:00:00 GMT";
        assert_eq!(merged("a=1", &[future]).unwrap(), "a=x");
        // An unreadable date is no date.
        assert_eq!(merged("a=1", &["a=x; Expires=whenever"]).unwrap(), "a=x");
    }

    #[test]
    fn max_age_outranks_expires() {
        let line = "a=x; Max-Age=3600; Expires=Thu, 01 Jan 1970 00:00:01 GMT";
        assert_eq!(merged("a=1", &[line]).unwrap(), "a=x");
        let line = "a=x; Max-Age=0; Expires=Fri, 01 Jan 2100 00:00:00 GMT";
        assert_eq!(merged("a=1; b=2", &[line]).unwrap(), "b=2");
    }

    #[test]
    fn the_dates_the_wild_sends_are_read() {
        for (text, expected) in [
            ("Thu, 01 Jan 1970 00:00:00 GMT", Some(0)),
            ("Thursday, 01-Jan-70 00:00:00 GMT", Some(0)),
            ("Thu Jan  1 00:00:00 1970", Some(0)),
            ("Fri, 10 Oct 2026 00:00:00 GMT", Some(NOW)),
            ("Fri, 10 Oct 2026 01:02:03 +0000", Some(NOW + 3723)),
            ("10 Oct 26 00:00:00", Some(NOW)),
            ("nonsense", None),
            ("Thu, 32 Jan 1970 00:00:00 GMT", None),
            ("Thu, 01 Jan 1970 25:00:00 GMT", None),
            ("Thu, 01 Jan 1970", None),
        ] {
            assert_eq!(http_date(text), expected, "{text}");
        }
    }

    #[test]
    fn values_keep_their_equals_signs_and_quotes() {
        assert_eq!(merged("s=old", &["s=abc==; path=/"]).unwrap(), "s=abc==");
        assert_eq!(merged("s=old", &["s=\"q\"; path=/"]).unwrap(), "s=\"q\"");
    }

    #[test]
    fn a_header_piece_with_no_name_is_dropped_on_rewrite() {
        assert_eq!(merged("a=1; junk; b=2", &["c=3"]).unwrap(), "a=1; b=2; c=3");
    }

    #[test]
    fn parse_reads_only_the_leading_pair_and_the_two_removal_attributes() {
        let c =
            parse_set_cookie("sid=abc; Path=/; Domain=x.example; Secure; HttpOnly", NOW).unwrap();
        assert_eq!(
            c,
            SetCookie {
                name: "sid",
                value: "abc",
                remove: false
            }
        );
        assert!(parse_set_cookie("sid=; Path=/", NOW).unwrap().remove);
    }
}
