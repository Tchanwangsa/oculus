//! GFM bare-URL autolinks.

use super::{InlineContext, Part};
use crate::markdown::chars::{self, js_space, word};
use crate::markdown::tables::NodeType as T;
use crate::markdown::tree::Elt;

fn run(s: &[u8], mut i: usize, ok: impl Fn(u8) -> bool) -> usize {
    while i < s.len() && ok(s[i]) {
        i += 1;
    }
    i
}

fn url_char(b: u8) -> bool {
    word(b) || b == b'-'
}

/// `autolinkURLEnd`: where a `www.`/`http://` URL from `from` ends, or None.
fn autolink_url_end(text: &str, from: usize) -> Option<usize> {
    let s = text.as_bytes();
    // urlRE: [\w-]+(\.[\w-]+)+(:\d+)?(\/[^\s<]*)? — each run is maximal,
    // because nothing after it could match a shorter one.
    let mut i = run(s, from, url_char);
    if i == from {
        return None;
    }
    // The last two host labels, which lastTwoDomainWords finds.
    let (mut prev, mut last) = (None, (from, i));
    while s.get(i) == Some(&b'.') {
        let j = run(s, i + 1, url_char);
        if j == i + 1 {
            break;
        }
        prev = Some(last);
        last = (i + 1, j);
        i = j;
    }
    let prev = prev?;
    if s.get(i) == Some(&b':') {
        let j = run(s, i + 1, |b| b.is_ascii_digit());
        if j > i + 1 {
            i = j;
        }
    }
    if s.get(i) == Some(&b'/') {
        i += 1;
        while let Some(c) = chars::char_at(text, i) {
            if c == '<' || js_space(c) {
                break;
            }
            i += c.len_utf8();
        }
    }
    // lastTwoDomainWords matches the last two labels of the host.
    if s[prev.0..prev.1].contains(&b'_') || s[last.0..last.1].contains(&b'_') {
        return None;
    }
    let mut end = i;
    loop {
        let last = s[end - 1];
        if b"?!.,:*_~".contains(&last)
            || (last == b')'
                && s[from..end].iter().filter(|&&b| b == b')').count()
                    > s[from..end].iter().filter(|&&b| b == b'(').count())
        {
            end -= 1;
        } else if last == b';' {
            match trailing_entity(&s[from..end]) {
                Some(at) => end = from + at,
                None => break,
            }
        } else {
            break;
        }
    }
    Some(end)
}

/// `/&(?:#\d+|#x[a-f\d]+|\w+);$/` (no `i` flag): where the match starts.
/// Its body cannot hold `&`, so only the last `&` can start it.
fn trailing_entity(s: &[u8]) -> Option<usize> {
    if s.last() != Some(&b';') {
        return None;
    }
    let amp = s.iter().rposition(|&b| b == b'&')?;
    let body = &s[amp + 1..s.len() - 1];
    let ok = if let Some(rest) = body.strip_prefix(b"#") {
        (!rest.is_empty() && rest.iter().all(|b| b.is_ascii_digit()))
            || rest.strip_prefix(b"x").is_some_and(|h| {
                !h.is_empty()
                    && h.iter()
                        .all(|&b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
    } else {
        !body.is_empty() && body.iter().all(|&b| word(b))
    };
    ok.then_some(amp)
}

/// `autolinkEmailEnd`: [\w.+-]+@[\w-]+(\.[\w.-]+)+ from `from`, minus a
/// trailing `.`; None if it ends in `_` or `-`.
fn autolink_email_end(s: &[u8], from: usize) -> Option<usize> {
    let local = run(s, from, |b| word(b) || b == b'.' || b == b'+' || b == b'-');
    if local == from || s.get(local) != Some(&b'@') {
        return None;
    }
    let host = run(s, local + 1, url_char);
    if host == local + 1 || s.get(host) != Some(&b'.') {
        return None;
    }
    let end = run(s, host + 1, |b| word(b) || b == b'.' || b == b'-');
    if end == host + 1 {
        return None;
    }
    match s[end - 1] {
        b'_' | b'-' => None,
        b'.' => Some(end - 1),
        _ => Some(end),
    }
}

pub(super) fn autolink(cx: &mut InlineContext, _next: i32, abs_pos: usize) -> Option<usize> {
    let s = cx.text.as_bytes();
    let pos = cx.rel(abs_pos);
    if pos > 0 && word(s[pos - 1]) {
        return None;
    }
    let rest = &s[pos..];
    let end = if rest.starts_with(b"www.")
        || rest.starts_with(b"http://")
        || rest.starts_with(b"https://")
    {
        let skip = if rest.starts_with(b"www.") {
            4
        } else if rest.starts_with(b"http://") {
            7
        } else {
            8
        };
        let mut end = autolink_url_end(cx.text, pos + skip)?;
        if cx.has_open_link() {
            end = pos + no_bracket_len(&s[pos..end]);
        }
        end
    } else {
        // [\w.+-]{1,100}@ — a longer run cannot be followed by `@` there.
        let limit = &s[..s.len().min(pos + 101)];
        let local = run(limit, pos, |b| {
            word(b) || b == b'.' || b == b'+' || b == b'-'
        });
        if local > pos && local - pos <= 100 && s.get(local) == Some(&b'@') {
            autolink_email_end(s, pos)?
        } else if rest.starts_with(b"mailto:") || rest.starts_with(b"xmpp:") {
            let xmpp = rest.starts_with(b"xmpp:");
            let mut end = autolink_email_end(s, pos + if xmpp { 5 } else { 7 })?;
            if xmpp && s.get(end) == Some(&b'/') {
                let r = run(s, end + 1, |b| {
                    b.is_ascii_alphanumeric() || b == b'@' || b == b'.'
                });
                if r > end + 1 {
                    end = r;
                }
            }
            end
        } else {
            return None;
        }
    };
    let elt = Elt::new(T::URL, abs_pos, end + cx.offset);
    cx.append(Part::Elt(elt));
    Some(end + cx.offset)
}

/// /([^\[\]]|\[[^\]]*\])*/ at the start of `s`: the greedy path is the
/// first match.
fn no_bracket_len(s: &[u8]) -> usize {
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b']' => break,
            b'[' => match s[i + 1..].iter().position(|&b| b == b']') {
                Some(k) => i += k + 2,
                None => break,
            },
            _ => i += 1,
        }
    }
    i
}
