//! Finding a library file and reading what a caller typed about it.

use crate::*;

/// Extensions whose bytes are worth reading as text.
pub(crate) const TEXT_EXTS: &[&str] = &["md", "txt", "csv", "json", "html", "htm", "vtt", "srt"];

pub(crate) fn is_text_file(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    TEXT_EXTS.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// Find the one file a caller meant.
///
/// Tiered, not fuzzy: exact path, then exact filename, then fragment; only the
/// best tier that matched counts, and a tie within it is reported, not guessed.
pub(crate) fn resolve_file<'a>(files: &'a [LibFile], target: &str) -> Result<&'a LibFile, String> {
    let needle = target.to_lowercase();
    let tiers: [Box<dyn Fn(&LibFile) -> bool>; 4] = [
        Box::new(|f: &LibFile| f.relative_path == target),
        Box::new(|f: &LibFile| f.filename == target),
        Box::new(|f: &LibFile| f.filename.to_lowercase() == needle),
        Box::new(|f: &LibFile| f.relative_path.to_lowercase().contains(&needle)),
    ];

    for matches in tiers {
        let hits: Vec<&LibFile> = files.iter().filter(|f| matches(f)).collect();
        match hits.len() {
            0 => continue,
            1 => return Ok(hits[0]),
            _ => {
                let mut message = format!("{} matches {} files:\n", target, hits.len());
                for f in hits.iter().take(12) {
                    message.push_str(&format!("       {}\n", f.relative_path));
                }
                if hits.len() > 12 {
                    message.push_str(&format!("       … and {} more\n", hits.len() - 12));
                }
                message.push_str("       Name one of them, or narrow it with --subject.");
                return Err(message);
            }
        }
    }
    Err(format!(
        "no library file matches {target} — `oculus files -m {}` to look",
        shell_quote(target)
    ))
}

/// `12`, `12-15`, `12,14,20-22`, `30-` (to the end), `-4` (from the start).
pub(crate) fn parse_page_spec(spec: &str) -> Result<Vec<(i64, i64)>, String> {
    let mut ranges = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let bad = || format!("not a page range: {part}");
        let (lo, hi) = match part.split_once('-') {
            None => {
                let n: i64 = part.parse().map_err(|_| bad())?;
                (n, n)
            }
            Some((from, to)) => {
                let lo = if from.trim().is_empty() {
                    1
                } else {
                    from.trim().parse().map_err(|_| bad())?
                };
                let hi = if to.trim().is_empty() {
                    i64::MAX
                } else {
                    to.trim().parse().map_err(|_| bad())?
                };
                (lo, hi)
            }
        };
        if lo > hi {
            return Err(format!("empty page range: {part}"));
        }
        ranges.push((lo, hi));
    }
    if ranges.is_empty() {
        return Err("no pages given".to_string());
    }
    Ok(ranges)
}

pub(crate) fn page_wanted(ranges: &[(i64, i64)], page: i64) -> bool {
    ranges.iter().any(|(lo, hi)| page >= *lo && page <= *hi)
}

pub(crate) fn build_regex(
    pattern: &str,
    fixed: bool,
    case_sensitive: bool,
) -> Result<regex::Regex, String> {
    let body = if fixed {
        regex::escape(pattern)
    } else {
        pattern.to_string()
    };
    regex::RegexBuilder::new(&body)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| format!("bad pattern: {e}"))
}

/// A page of markdown flattened to one line of prose, for a result list.
pub(crate) fn snippet(markdown: &str, max: usize) -> String {
    let flat: Vec<&str> = markdown.split_whitespace().collect();
    truncate(&flat.join(" "), max)
}

/// Quote a suggested command argument so a name with spaces pastes intact.
pub(crate) fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || "._-/".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}
