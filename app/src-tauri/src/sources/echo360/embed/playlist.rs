//! HLS as Echo360 packages an embed: a master lists variants by bandwidth,
//! and a variant is byte ranges over one fMP4 that `#EXT-X-MAP` names. That
//! file, fetched whole, is the stream, so a download needs no segment
//! stitching. Any other packaging is refused rather than half-supported.

use url::Url;

pub const UNSUPPORTED: &str = "This video uses a packaging Oculus doesn't support yet";

/// The highest-`BANDWIDTH` variant URI of a master playlist, as written (often
/// relative). `None` when the text is already a media playlist.
pub(super) fn best_variant(master: &str) -> Result<Option<String>, String> {
    let mut best: Option<(u64, String)> = None;
    let mut pending: Option<u64> = None;
    for line in master.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            let attrs = parse_attrs(attrs);
            let bandwidth = ["BANDWIDTH", "AVERAGE-BANDWIDTH"]
                .iter()
                .find_map(|k| attr(&attrs, k).and_then(|v| v.parse().ok()))
                .unwrap_or(0);
            pending = Some(bandwidth);
        } else if !line.starts_with('#') {
            if let Some(bandwidth) = pending.take() {
                if best.as_ref().is_none_or(|(b, _)| bandwidth > *b) {
                    best = Some((bandwidth, line.to_string()));
                }
            }
        }
    }
    match best {
        Some((_, uri)) => Ok(Some(uri)),
        None if master.contains("#EXTINF") => Ok(None),
        None => Err(format!("{UNSUPPORTED} (a playlist with no variants).")),
    }
}

/// The one file a variant's init map and every segment name, resolved
/// against `variant_url` and without a query (it is signed afresh).
pub(super) fn single_file(variant: &str, variant_url: &Url) -> Result<Url, String> {
    let resolve = |uri: &str| -> Result<Url, String> {
        let mut url = variant_url
            .join(uri)
            .map_err(|e| format!("A playlist entry is not a URL: {e}"))?;
        url.set_query(None);
        url.set_fragment(None);
        Ok(url)
    };
    let map = variant
        .lines()
        .find_map(|l| l.trim().strip_prefix("#EXT-X-MAP:"))
        .and_then(|attrs| attr(&parse_attrs(attrs), "URI").map(str::to_string))
        .ok_or_else(|| format!("{UNSUPPORTED} (no single-file map)."))?;
    let file = resolve(&map)?;
    let mut segments = 0;
    for line in variant.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if resolve(line)? != file {
            return Err(format!("{UNSUPPORTED} (segments span several files)."));
        }
        segments += 1;
    }
    if segments == 0 {
        return Err(format!("{UNSUPPORTED} (a playlist with no segments)."));
    }
    Ok(file)
}

/// An attribute list (`A=1,B="x,y"`) as name/value pairs, quotes removed.
fn parse_attrs(list: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = list.trim();
    while !rest.is_empty() {
        let Some((name, after)) = rest.split_once('=') else {
            break;
        };
        let (value, tail) = if let Some(quoted) = after.strip_prefix('"') {
            match quoted.split_once('"') {
                Some((v, t)) => (v, t),
                None => (quoted, ""),
            }
        } else {
            after.split_once(',').map_or((after, ""), |(v, t)| (v, t))
        };
        out.push((name.trim().to_string(), value.to_string()));
        rest = tail.trim_start_matches(',').trim_start();
    }
    out
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}
