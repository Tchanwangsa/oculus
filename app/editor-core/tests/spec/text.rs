//! Text helpers for the renderer: HTML escaping, backslash and entity
//! decoding, URL encoding and link-label normalisation.

use std::collections::HashMap;

pub(super) fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

const ESCAPABLE: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

/// Backslash escapes and character references decoded, as in link
/// destinations, titles and info strings.
pub(super) fn unescape(s: &str, entities: &HashMap<String, String>) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '\\'
            && rest[1..]
                .chars()
                .next()
                .is_some_and(|n| ESCAPABLE.contains(n))
        {
            out.push(rest[1..].chars().next().unwrap());
            rest = &rest[2..];
        } else if c == '&'
            && let Some((decoded, len)) = decode_entity(rest, entities)
        {
            out.push_str(&decoded);
            rest = &rest[len..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// The character reference at the start of `s` (which starts with `&`).
pub(super) fn decode_entity(
    s: &str,
    entities: &HashMap<String, String>,
) -> Option<(String, usize)> {
    let end = s.find(';')?;
    let body = &s[1..end];
    let decoded = if let Some(num) = body.strip_prefix('#') {
        let (digits, radix) = match num.strip_prefix(['x', 'X']) {
            Some(hex)
                if (1..=6).contains(&hex.len()) && hex.chars().all(|c| c.is_ascii_hexdigit()) =>
            {
                (hex, 16)
            }
            None if (1..=7).contains(&num.len()) && num.chars().all(|c| c.is_ascii_digit()) => {
                (num, 10)
            }
            _ => return None,
        };
        let code = u32::from_str_radix(digits, radix).ok()?;
        let c = if code == 0 {
            '\u{fffd}'
        } else {
            char::from_u32(code).unwrap_or('\u{fffd}')
        };
        c.to_string()
    } else {
        entities.get(body)?.clone()
    };
    Some((decoded, end + 1))
}

/// The reference renderer's URL normalisation: percent-encode what is not
/// safe in a URL, keeping existing `%XX` escapes.
pub(super) fn encode_url(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'%'
            && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
            && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit)
        {
            out.push_str(&s[i..i + 3]);
            i += 3;
            continue;
        }
        if c.is_ascii_alphanumeric() || b";/?:@&=+$,-_.!~*'()#".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
        i += 1;
    }
    out
}

/// Link label matching: Unicode case fold (approximated by lowercasing and
/// `ß`→`ss`) and collapsed whitespace.
pub(super) fn normalize_label(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .replace('ß', "ss")
}
