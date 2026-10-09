//! Comparing version strings and reading one out of a registry's answer.

use super::fetch::Body;

/// The numeric core of a version: `v` dropped, then everything from the
/// first `-`/`+`/`,` (prerelease, build, a cask's `,sha`) ignored — so
/// `1.2.0-beta` orders as `1.2.0`.
pub(super) fn version_core(v: &str) -> Vec<u64> {
    let v = v.trim();
    let v = v.strip_prefix('v').unwrap_or(v);
    let core = v.split(['-', '+', ',']).next().unwrap_or("");
    core.split('.')
        .map(|part| {
            let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
            digits.parse().unwrap_or(0)
        })
        .collect()
}

/// Whether `latest` is strictly newer than `installed`, missing parts read
/// as zero. A local build ahead of the registry is not an update.
pub fn is_newer(latest: &str, installed: &str) -> bool {
    let (a, b) = (version_core(latest), version_core(installed));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

pub(super) fn read_version(body: &str, shape: Body) -> Result<String, String> {
    let v = match shape {
        Body::Text => body.trim().to_string(),
        Body::JsonVersion => serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|j| {
                j.get("version")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .ok_or("the answer had no version")?,
    };
    if v.trim_start_matches('v')
        .starts_with(|c: char| c.is_ascii_digit())
    {
        Ok(v)
    } else {
        Err(format!("unexpected answer {v:?}"))
    }
}
