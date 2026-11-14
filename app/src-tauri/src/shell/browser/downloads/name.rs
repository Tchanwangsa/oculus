//! The name a download is staged under, from the path WebKit or WebView2
//! proposes. The server picks that name, so only its last component is kept,
//! and nothing in it can leave the download's own staging folder.

use std::path::Path;

/// Room for a `-2` from the uploads folder and the `.pdf` an Office file
/// gains, under the 255-byte name limit of APFS and NTFS.
const MAX_BYTES: usize = 200;
/// An extension longer than this is not kept whole when the name is cut.
const MAX_EXT_BYTES: usize = 16;
const FALLBACK: &str = "download";

/// The safe file name for a download proposed at `proposed`. `downloads`
/// is the folder the engine proposed it in (`~/Downloads`), where it may have
/// counted past a taken name.
pub(super) fn staged_name(proposed: &Path, downloads: Option<&Path>) -> String {
    // The text after the last separator, not `file_name()`, which would
    // read `~/Downloads/.` or a bare `~/Downloads/` as "Downloads".
    let raw = proposed.to_string_lossy();
    let last = raw.rsplit(['/', '\\']).next().unwrap_or_default();
    let last = match downloads {
        Some(dir) => uncounted(last, |candidate| dir.join(candidate).exists()),
        None => last.to_string(),
    };
    safe(&last)
}

/// wry and WebView2 turn `notes.pdf` into `notes (1).pdf` when `~/Downloads`
/// holds one already; that counter means nothing in staging. Dropped only
/// when the name it stepped around is really there.
pub(super) fn uncounted(name: &str, exists: impl Fn(&str) -> bool) -> String {
    let Some(open) = name.rfind(" (") else {
        return name.to_string();
    };
    let rest = &name[open + 2..];
    let Some(close) = rest.find(')') else {
        return name.to_string();
    };
    let (digits, after) = (&rest[..close], &rest[close + 1..]);
    let counted = !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (after.is_empty() || after.starts_with('.'));
    let candidate = format!("{}{after}", &name[..open]);
    if counted && !candidate.is_empty() && exists(&candidate) {
        candidate
    } else {
        name.to_string()
    }
}

/// One path component on macOS and Windows alike: no separators, no
/// characters either system refuses, no leading dot (a hidden file), no
/// trailing dot or space, no device name, never empty, and short enough.
pub(super) fn safe(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        return FALLBACK.to_string();
    }
    let shortened = shorten(trimmed);
    let stem = shortened
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| {
            stem.strip_prefix(p)
                .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
        });
    if device {
        format!("_{shortened}")
    } else {
        shortened
    }
}

/// Cuts `name` to `MAX_BYTES`, keeping a short extension whole.
fn shorten(name: &str) -> String {
    if name.len() <= MAX_BYTES {
        return name.to_string();
    }
    let ext = name
        .rfind('.')
        .map(|at| &name[at..])
        .filter(|ext| ext.len() <= MAX_EXT_BYTES)
        .unwrap_or("");
    let mut cut = MAX_BYTES - ext.len();
    while !name.is_char_boundary(cut) {
        cut -= 1;
    }
    let stem = name[..cut].trim_end_matches(|c: char| c == '.' || c.is_whitespace());
    format!("{stem}{ext}")
}
