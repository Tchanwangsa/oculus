//! Signing a CDN URL the way the player's `applySourceQueryStringsToUri`
//! does: the query string of the pattern that matches the URL *replaces* the
//! URL's own query. Appending to it 403s, as do the bare URL and a Bearer.

use super::properties::QueryString;

/// `url` with its query replaced by its pattern's. A `uriPattern` is assumed
/// to be a CloudFront resource: literal text where `*` matches any run of
/// characters, `/` included, matched against the URL without its query. When
/// several match, the longest pattern (the most specific) wins.
pub fn sign(url: &str, patterns: &[QueryString]) -> Result<String, String> {
    let bare = url.split(['?', '#']).next().unwrap_or(url);
    let pattern = patterns
        .iter()
        .filter(|p| glob(&p.uri_pattern, bare))
        .max_by_key(|p| p.uri_pattern.len())
        .ok_or_else(|| {
            format!(
                "None of Echo360's {} signing patterns covers {bare}",
                patterns.len()
            )
        })?;
    let query = pattern.query_string.trim_start_matches('?');
    Ok(if query.is_empty() {
        bare.to_string()
    } else {
        format!("{bare}?{query}")
    })
}

/// `*` matches any run of characters; everything else is literal.
pub(super) fn glob(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    let (mut pi, mut ti) = (0, 0);
    // The last `*` seen and the text position it was tried against.
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && p[pi] == b'*' {
            star = Some((pi, ti));
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&b| b == b'*')
}
