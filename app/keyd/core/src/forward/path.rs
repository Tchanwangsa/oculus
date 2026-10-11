//! Which paths `forward` sends. A path is checked so no URL parser, proxy or
//! server can read it as another origin, another directory or a second
//! request.

use super::Route;
use crate::ops::OpError;

pub(super) const MAX_PATH: usize = 2048;

/// `path` against `route`'s prefix and rules; see `strict` and `session`.
pub(super) fn check_path(path: &str, route: &Route) -> Result<(), OpError> {
    let bad = |why: &str| Err(OpError::new("request", format!("path {why}")));
    if path.len() > MAX_PATH {
        return bad("is too long");
    }
    if !path.starts_with(route.prefix) {
        return bad(&format!("must start with {}", route.prefix));
    }
    let checked = if route.is_session() {
        session(path, route.prefix)
    } else {
        strict(path)
    };
    checked.map_err(|why| OpError::new("request", format!("path {why}")))
}

/// The cloud routes: unreserved characters, `/`, and a query of `=` and `&`.
/// No `%`, because a parser may decode `%2e%2e` to `..`.
fn strict(path: &str) -> Result<(), &'static str> {
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b"-._~/?=&".contains(&b);
    if !path.bytes().all(allowed) {
        return Err("has a character outside [A-Za-z0-9-._~/?=&]");
    }
    let route_part = path.split('?').next().unwrap_or("");
    if route_part.contains("//") {
        return Err("has an empty segment");
    }
    if route_part.split('/').any(|seg| seg == "." || seg == "..") {
        return Err("has a dot segment");
    }
    Ok(())
}

/// The session routes, whose real URLs carry `include[]=term`, `%5B` and
/// whole encoded URLs in `next`. Beyond `strict`'s characters, `[ ] % : + ; ,`
/// pass, but the path part (before the `?`) is judged as it reads once
/// percent-decoded:
/// - no escape may make a `/`, `\`, NUL, control or non-ASCII byte, or a `%`
///   (the second layer of a double encoding, so `%252e` is refused);
/// - no segment may be `.` or `..`, with or without a `;parameter` after it;
/// - the decoded path still starts with the route's prefix.
///
/// The query may hold any escape (`%2F` is how a `next` URL travels) except
/// one that decodes to NUL or a control character.
fn session(path: &str, prefix: &str) -> Result<(), &'static str> {
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b"-._~/?=&[]%:+;,".contains(&b);
    if !path.bytes().all(allowed) {
        return Err("has a character outside [A-Za-z0-9-._~/?=&[]%:+;,]");
    }
    let (route_part, query) = path.split_once('?').unwrap_or((path, ""));
    if route_part.contains("//") {
        return Err("has an empty segment");
    }

    // Segment by segment, so an escaped `/` is caught where it appears.
    let mut decoded = Vec::new();
    for (i, raw) in route_part.split('/').enumerate() {
        let segment = decode(raw).ok_or("has a malformed percent escape")?;
        if segment
            .iter()
            .any(|&b| b == b'/' || b == b'\\' || !(0x20..0x7f).contains(&b))
        {
            return Err("has an escaped slash, backslash or control character");
        }
        if segment.contains(&b'%') {
            return Err("has a double-encoded character");
        }
        let name = segment.split(|&b| b == b';').next().unwrap_or(&[]);
        if name == b"." || name == b".." {
            return Err("has a dot segment");
        }
        if i > 0 {
            decoded.push(b'/');
        }
        decoded.extend(segment);
    }
    if !decoded.starts_with(prefix.as_bytes()) {
        return Err("leaves its prefix once decoded");
    }

    let query = decode(query).ok_or("has a malformed percent escape")?;
    if query.iter().any(|&b| b < 0x20 || b == 0x7f) {
        return Err("has an escaped control character");
    }
    Ok(())
}

/// `text` with each `%XX` replaced by its byte; `None` for a `%` not followed
/// by two hex digits.
fn decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = |b: u8| (b as char).to_digit(16);
            let high = hex(*bytes.get(i + 1)?)?;
            let low = hex(*bytes.get(i + 2)?)?;
            out.push((high * 16 + low) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forward::Routes;

    fn route(name: &str) -> Route {
        Routes::compiled().get(name).unwrap().clone()
    }

    fn passes(name: &str, path: &str) -> bool {
        check_path(path, &route(name)).is_ok()
    }

    #[test]
    fn cloud_paths_keep_the_strict_charset() {
        for ok in [
            "/v1/multimodalembeddings",
            "/v1/models?limit=5&x=y",
            "/v1/a-b_c.d~e/f",
        ] {
            assert!(passes("voyage", ok), "{ok}");
        }
        for bad in [
            "/v1/a[]=1",
            "/v1/a%2Fb",
            "/v1/a:b",
            "/v1/a+b",
            "/v1/a;b",
            "/v1/a,b",
            "/v1/%2e%2e/admin",
            "/v1/../admin",
            "/v1//x",
        ] {
            assert!(!passes("voyage", bad), "{bad}");
        }
    }

    #[test]
    fn the_real_urls_the_app_sends_pass() {
        for (route, path) in [
            (
                "canvas",
                "/api/v1/courses?include[]=term&include[]=account&per_page=100",
            ),
            (
                "canvas",
                "/api/v1/courses/12/modules?include[]=items&per_page=50",
            ),
            (
                "canvas",
                "/api/v1/calendar_events?context_codes[]=course_12&start_date=2026-01-01",
            ),
            ("canvas", "/files/123/download?download_frd=1&verifier=abc"),
            ("canvas", "/courses/12/external_tools/34?display=borderless"),
            ("canvas", "/api/v1/courses?include%5B%5D=term&per_page=100"),
            (
                "canvas",
                "/api/v1/courses/12/files?page=bookmark:WyJ4Il0&per_page=100",
            ),
            (
                "canvas",
                "/api/v1/users/self/todo?next=https%3A%2F%2Fcanvas.lms.unimelb.edu.au%2Fapi%2Fv1%2Fx%3Fa%3D1%26b%3D%255B",
            ),
            ("canvas", "/api/v1/courses/12/pages/week%201%20notes"),
            ("canvas", "/api/v1/search?q=a+b,c;d"),
            ("canvas", "/"),
            ("canvas", "/?x=1"),
            ("ed", "/api/courses/9/threads?limit=100&offset=0&sort=new"),
            ("ed", "/api/threads/77?view=1"),
            ("ed", "/api/renew_token"),
            ("ed", "/api/login_token"),
        ] {
            assert!(passes(route, path), "{route} {path}");
        }
    }

    #[test]
    fn a_session_path_must_stay_under_its_prefix() {
        for bad in [
            "/v1/x",
            "/api",
            "/apiary/x",
            "api/x",
            "/%61pi/x",
            "/ap%69/x",
            "/user",
        ] {
            assert!(!passes("ed", bad), "ed {bad}");
        }
        assert!(passes("ed", "/api/x"));
        assert!(!passes("canvas", "api/v1/courses"));
        assert!(!passes("canvas", ""));
    }

    #[test]
    fn tricks_that_could_leave_the_prefix_the_directory_or_the_origin_are_refused() {
        for path in [
            // dot segments, raw and escaped, in either case
            "/api/../x",
            "/api/./x",
            "/api/x/..",
            "/api/x/.",
            "/api/%2e%2e/x",
            "/api/%2E%2E/x",
            "/api/%2e./x",
            "/api/.%2E/x",
            "/api/%2e/x",
            "/api/x/%2e%2e",
            "/api/..;/x",
            "/api/..;a=b/x",
            "/api/%2e%2e;/x",
            // double encoding
            "/api/%252e%252e/x",
            "/api/%252E/x",
            "/api/%25/x",
            "/api/%2525/x",
            // separators and controls produced by an escape
            "/api/x%2fy",
            "/api/x%2Fy",
            "/api/%2f..%2fx",
            "/api/x%5cy",
            "/api/x%5Cy",
            "/api/x%00y",
            "/api/x%0ay",
            "/api/x%0D%0AHost:evil",
            "/api/x%7f",
            "/api/x%80",
            "/api/%c0%ae%c0%ae/x",
            // an escape that is not one
            "/api/x%",
            "/api/x%2",
            "/api/x%zz",
            "/api/x%+1",
            "/api/x%-1",
            // an empty segment, a second origin, other characters
            "/api//x",
            "//evil.example/x",
            "/api/x//",
            "/api/@evil.example",
            "/api/x@y",
            "/api/x\\y",
            "/api/x#frag",
            "/api/x y",
            "/api/x\ty",
            "/api/x\r\nHost: evil",
            "/api/x\u{0}",
            "/api/ü",
            "/api/x'y",
            "/api/x\"y",
            "/api/x<y",
            "/api/x|y",
            "/api/x{y}",
            "/api/x*y",
            "/api/x!y",
            "/api/x(y)",
            "/api/x$y",
            // a control smuggled into the query
            "/api/x?a=%00",
            "/api/x?a=%0d%0aHost:evil",
            "/api/x?a=%7f",
            "/api/x?a=%",
            "/api/x?a=%g1",
            "/api/x?a=b#frag",
            "/api/x?a=b\r\nHost: evil",
            "/api/x?a=ü",
        ] {
            let err = check_path(path, &route("ed")).unwrap_err();
            assert_eq!(err.kind, "request", "{path:?}");
        }
        let long = format!("/api/{}", "a".repeat(MAX_PATH));
        assert!(!passes("ed", &long));
        assert!(!passes("canvas", &long));
    }

    #[test]
    fn the_same_tricks_are_refused_on_canvas() {
        for path in [
            "/api/v1/../../x",
            "/api/v1/%2e%2e/x",
            "/api/v1/%2E%2e/x",
            "/api/v1/%252e%252e/x",
            "/api/v1/x%2fy",
            "/api/v1/x%5cy",
            "/api/v1/x%00",
            "/api/v1//x",
            "//evil.example/",
            "/\\evil.example/",
            "/api/v1/x#y",
            "/api/v1/@x",
            "/api/v1/..;/x",
            "/api/v1/x?a=%00",
            "/ü",
        ] {
            assert!(!passes("canvas", path), "{path:?}");
        }
    }

    #[test]
    fn a_query_may_carry_what_a_path_may_not() {
        // `next` URLs travel encoded, slashes and all.
        assert!(passes(
            "canvas",
            "/api/v1/x?next=%2Fapi%2Fv1%2Fy&u=%5C&p=%25"
        ));
        assert!(passes("canvas", "/api/v1/x?a=//b&c=../d"));
        assert!(passes("ed", "/api/x?redirect=%2e%2e%2f"));
    }

    /// Every ASCII byte and every two-digit escape, in a path and in a
    /// query: whatever the checker lets through must be a path under the
    /// prefix that has no dot segment and no escaped separator or control.
    #[test]
    fn what_passes_is_safe_for_every_single_byte_and_escape() {
        let canvas = route("canvas");
        let mut pieces: Vec<String> = (0u8..=255).map(|b| (b as char).to_string()).collect();
        pieces.extend((0u8..=255).map(|b| format!("%{b:02x}")));
        pieces.extend((0u8..=255).map(|b| format!("%{b:02X}")));
        for piece in &pieces {
            for path in [
                format!("/api/{piece}x"),
                format!("/api/x{piece}"),
                format!("/api/{piece}/x"),
                format!("/api/x/{piece}"),
                format!("/api/x/{piece}{piece}"),
                format!("/api/x?a={piece}"),
                format!("/{piece}"),
            ] {
                if check_path(&path, &canvas).is_err() {
                    continue;
                }
                let route_part = path.split('?').next().unwrap();
                let decoded = decode(route_part).unwrap();
                assert!(decoded.starts_with(b"/"), "{path:?}");
                assert!(
                    !decoded.iter().any(|&b| b == b'\\' || b < 0x20 || b >= 0x7f),
                    "{path:?}"
                );
                let raw_slashes = route_part.bytes().filter(|&b| b == b'/').count();
                let decoded_slashes = decoded.iter().filter(|&&b| b == b'/').count();
                assert_eq!(raw_slashes, decoded_slashes, "{path:?}");
                assert!(
                    !decoded
                        .split(|&b| b == b'/')
                        .any(|s| s.split(|&b| b == b';').next() == Some(b".")
                            || s.split(|&b| b == b';').next() == Some(b"..")),
                    "{path:?}"
                );
                assert!(!route_part.contains("//"), "{path:?}");
                assert!(!decoded.contains(&b'%'), "{path:?}");
                assert!(path.bytes().all(|b| b.is_ascii_graphic()), "{path:?}");
            }
        }
    }
}
