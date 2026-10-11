//! A scanner over a component's TSX text, for the lint rules that read
//! source (`tokens`). It is not a parser: comments are dropped, strings and
//! template literals become one `Str` token each, and everything else is an
//! identifier, a number or a single punctuation character. JSX text is read
//! as ordinary tokens.

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Ident,
    Number(f64),
    /// A quoted string or template literal; `text` is what is between the
    /// quotes, with escapes left as written.
    Str,
    Punct(char),
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub kind: Kind,
    pub text: String,
    /// Byte offset just past the token.
    pub end: usize,
    /// 1-based line and column (in characters).
    pub line: u32,
    pub col: u32,
}

pub fn scan(src: &str) -> Vec<Tok> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let (mut line, mut line_start) = (1u32, 0usize);
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            line_start = i;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                if bytes[i] == b'\n' {
                    line += 1;
                    line_start = i + 1;
                }
                i += 1;
            }
            i = (i + 2).min(bytes.len());
            continue;
        }
        let start = i;
        let (kind, end) = if c == b'"' || c == b'\'' || c == b'`' {
            // Strings end at their quote or at the line's end, so an
            // apostrophe in JSX text cannot swallow the rest of the file.
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] != c && bytes[j] != b'\n' {
                j += if bytes[j] == b'\\' { 2 } else { 1 };
            }
            let j = j.min(bytes.len());
            let closed = bytes.get(j) == Some(&c);
            let text_end = j;
            let end = if closed { j + 1 } else { j };
            (Kind::Str, (start, text_end, end))
        } else if c.is_ascii_alphabetic() || c == b'_' || c == b'$' {
            let mut j = i;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric() || matches!(bytes[j], b'_' | b'$'))
            {
                j += 1;
            }
            (Kind::Ident, (start, j, j))
        } else if c.is_ascii_digit() {
            let mut j = i;
            while j < bytes.len()
                && (bytes[j].is_ascii_digit()
                    || (bytes[j] == b'.' && bytes.get(j + 1).is_some_and(u8::is_ascii_digit)))
            {
                j += 1;
            }
            let value = src[start..j].parse::<f64>().unwrap_or(f64::NAN);
            (Kind::Number(value), (start, j, j))
        } else {
            let ch = src[i..].chars().next().unwrap_or('\0');
            let len = ch.len_utf8();
            (Kind::Punct(ch), (start, start + len, start + len))
        };
        let (tstart, tend, end) = end;
        // `get`, not indexing: an escape can end inside a multi-byte character.
        let text = match kind {
            Kind::Str => src.get(tstart + 1..tend).unwrap_or_default().to_owned(),
            _ => src.get(tstart..tend).unwrap_or_default().to_owned(),
        };
        let col = src[line_start..start].chars().count() as u32 + 1;
        out.push(Tok {
            kind,
            text,
            end,
            line,
            col,
        });
        i = end.max(start + 1);
    }
    out
}

/// The text of line `line` (1-based), without its newline.
pub fn line_text(src: &str, line: u32) -> &str {
    src.lines()
        .nth(line.saturating_sub(1) as usize)
        .unwrap_or("")
}
