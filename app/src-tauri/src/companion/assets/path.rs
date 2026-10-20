//! The asset allow-list on request paths. Only `index.html` and files under
//! `assets/` exist; every segment is plain characters, so `..`, `.`, empty
//! segments, backslashes, NUL, percent-escapes and drive letters never reach
//! a lookup.

/// The bundle-relative path for the part of a URL after `/c/`, or `None`.
pub fn normalise(rest: &str) -> Option<String> {
    if rest.is_empty() || rest == "index.html" {
        return Some("index.html".into());
    }
    let inner = rest.strip_prefix("assets/")?;
    let segments: Vec<&str> = inner.split('/').collect();
    let plain = segments.iter().all(|segment| {
        !segment.is_empty()
            && !segment.starts_with('.')
            && segment.bytes().all(|b| {
                b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'~' | b'@' | b'+')
            })
    });
    plain.then(|| format!("assets/{inner}"))
}

#[cfg(test)]
mod tests {
    use super::normalise;

    #[test]
    fn only_plain_paths_under_the_allow_list_pass() {
        assert_eq!(normalise("").as_deref(), Some("index.html"));
        assert_eq!(normalise("index.html").as_deref(), Some("index.html"));
        assert_eq!(
            normalise("assets/index-Ab12_c.js").as_deref(),
            Some("assets/index-Ab12_c.js")
        );
        assert_eq!(
            normalise("assets/fonts/inter.woff2").as_deref(),
            Some("assets/fonts/inter.woff2")
        );
        for bad in [
            "assets/",
            "assets/../index.html",
            "assets/./a.js",
            "assets//a.js",
            "assets/..",
            "assets/.hidden",
            "assets/a\\b.js",
            "assets/a%2e%2e.js",
            "assets/%2e%2e/secret",
            "assets/a\0.js",
            "assets/C:x.js",
            "assets/a b.js",
            "../index.html",
            "/etc/passwd",
            "other.js",
            "favicon.ico",
        ] {
            assert_eq!(normalise(bad), None, "{bad:?}");
        }
    }
}
