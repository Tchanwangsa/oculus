//! Reading a login's output: stripping colour codes and finding the URL.

/// Strip ANSI escape sequences — Codex colours its URL.
pub(super) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            // CSI: parameters, then a byte in @…~ ends it.
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if matches!(c, '@'..='~') {
                        break;
                    }
                }
            }
            // OSC: ended by BEL, or by ESC \ — the ESC of which is eaten on
            // the next turn of the outer loop.
            Some(']') => {
                chars.next();
                for c in chars.by_ref() {
                    if c == '\u{7}' || c == '\u{1b}' {
                        break;
                    }
                }
            }
            // A two-character escape; drop both.
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }
    out
}

/// The first `https://` run in a line, minus trailing punctuation. Only
/// https: Codex prints its `http://localhost:1455` listener just before the
/// authorize URL.
pub(super) fn extract_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let url = rest[..end].trim_end_matches(['.', ',', ')', '>']);
    (url.len() > "https://".len()).then(|| url.to_string())
}
