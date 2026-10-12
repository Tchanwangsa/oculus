//! Link targets and link text in the lesson's files.

/// `rel` (course-relative) as a link from the folder `dir`, each segment
/// percent-encoded, since a space or `(` would end a Markdown link.
pub(super) fn local_link(rel: &str, dir: &str) -> String {
    let within = rel.strip_prefix(dir).unwrap_or(rel);
    let mut out = String::new();
    for (i, segment) in within.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        for b in segment.bytes() {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// [`local_link`] led by `./`, so a file beside the slide reads as a local
/// path to the viewer, which resolves `<segment>/…` below the linking file.
pub(super) fn sibling_link(rel: &str, dir: &str) -> String {
    format!("./{}", local_link(rel, dir))
}

pub(super) fn link_text(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            let esc = matches!(c, '[' | ']' | '\\' | '*' | '_' | '`');
            esc.then_some('\\').into_iter().chain(std::iter::once(c))
        })
        .collect()
}
