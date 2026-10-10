//! The HTML regexes of `@lezer/markdown`, as hand-written matchers. Each one
//! names the regex it replaces; where a regex could backtrack, the comment
//! says why the greedy path is the only one that can succeed. `\s` is JS's
//! Unicode whitespace; `\w` and the `i` flag are ASCII-only (no `u` flag).

use super::chars::{char_at, js_space, skip_js_space, word};

fn byte(s: &str, i: usize) -> Option<u8> {
    s.as_bytes().get(i).copied()
}

fn starts_with_ci(s: &str, i: usize, lit: &str) -> bool {
    s.as_bytes()
        .get(i..i + lit.len())
        .is_some_and(|b| b.eq_ignore_ascii_case(lit.as_bytes()))
}

/// `\s` at `i`.
fn space_at(s: &str, i: usize) -> bool {
    char_at(s, i).is_some_and(js_space)
}

fn run(s: &str, mut i: usize, ok: impl Fn(u8) -> bool) -> usize {
    while byte(s, i).is_some_and(&ok) {
        i += 1;
    }
    i
}

const BLOCK_NAMES: &[&str] = &[
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "section",
    "source",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// Which `HTMLBlockStyle` start regex matches `rest` (the line from its first
/// non-space char, which is `<`); the last style is skipped when `breaking`.
pub(crate) fn block_start(rest: &str, breaking: bool) -> Option<usize> {
    let styles = if breaking { 6 } else { 7 };
    (0..styles).find(|&style| block_start_style(rest, style))
}

fn block_start_style(rest: &str, style: usize) -> bool {
    match style {
        // /^<(?:script|pre|style)(?:\s|>|$)/i
        0 => {
            byte(rest, 0) == Some(b'<')
                && ["script", "pre", "style"].iter().any(|name| {
                    starts_with_ci(rest, 1, name) && {
                        let after = 1 + name.len();
                        after == rest.len()
                            || byte(rest, after) == Some(b'>')
                            || space_at(rest, after)
                    }
                })
        }
        // /^\s*<!--/
        1 => rest[skip_js_space(rest, 0)..].starts_with("<!--"),
        // /^\s*<\?/
        2 => rest[skip_js_space(rest, 0)..].starts_with("<?"),
        // /^\s*<![A-Z]/
        3 => {
            let i = skip_js_space(rest, 0);
            rest[i..].starts_with("<!") && byte(rest, i + 2).is_some_and(|b| b.is_ascii_uppercase())
        }
        // /^\s*<!\[CDATA\[/
        4 => rest[skip_js_space(rest, 0)..].starts_with("<![CDATA["),
        // /^\s*<\/?(?:address|…|ul)(?:\s|\/?>|$)/i — any listed name followed
        // by the terminator matches, so every name is tried.
        5 => {
            let mut i = skip_js_space(rest, 0);
            if byte(rest, i) != Some(b'<') {
                return false;
            }
            i += 1;
            if byte(rest, i) == Some(b'/') {
                i += 1;
            }
            BLOCK_NAMES.iter().any(|name| {
                starts_with_ci(rest, i, name) && {
                    let j = i + name.len();
                    j == rest.len()
                        || space_at(rest, j)
                        || byte(rest, j) == Some(b'>')
                        || rest[j..].starts_with("/>")
                }
            })
        }
        // /^\s*(?:<\/[a-z][\w-]*\s*>|<[a-z][\w-]*(\s+[a-z:_][\w-.]*(?:\s*=\s*(?:[^\s"'=<>`]+|'[^']*'|"[^"]*"))?)*\s*>)\s*$/i
        _ => {
            let i = skip_js_space(rest, 0);
            if byte(rest, i) != Some(b'<') {
                return false;
            }
            let end = if byte(rest, i + 1) == Some(b'/') {
                close_tag_name(rest, i + 2).and_then(|j| {
                    let j = skip_js_space(rest, j);
                    (byte(rest, j) == Some(b'>')).then_some(j + 1)
                })
            } else {
                tag_name(rest, i + 1).and_then(|j| {
                    let j = attributes(rest, j, false);
                    let j = skip_js_space(rest, j);
                    (byte(rest, j) == Some(b'>')).then_some(j + 1)
                })
            };
            end.is_some_and(|j| skip_js_space(rest, j) == rest.len())
        }
    }
}

/// `[a-z][\w-]*` (any case): the name's end. The run is maximal, since what
/// follows a name can never be a name char.
fn tag_name(s: &str, i: usize) -> Option<usize> {
    byte(s, i)
        .is_some_and(|b| b.is_ascii_alphabetic())
        .then(|| run(s, i + 1, |b| word(b) || b == b'-'))
}

fn close_tag_name(s: &str, i: usize) -> Option<usize> {
    tag_name(s, i)
}

/// `(\s+[a-z:_][\w-.]*(?:\s*=\s*(?:[^\s"'=<>`]+|'[^']*'|"[^"]*"))?)*` from
/// `i`; `colon` adds `:` to the name's tail (the inline regex). An attribute
/// is taken whenever whitespace is followed by a name start, and a value
/// whenever one parses: neither choice can make a later part fail.
fn attributes(s: &str, mut i: usize, colon: bool) -> usize {
    loop {
        let j = skip_js_space(s, i);
        if j == i || !byte(s, j).is_some_and(|b| b.is_ascii_alphabetic() || b == b':' || b == b'_')
        {
            return i;
        }
        i = run(s, j + 1, |b| {
            word(b) || b == b'-' || b == b'.' || (colon && b == b':')
        });
        let k = skip_js_space(s, i);
        if byte(s, k) == Some(b'=') {
            let v = skip_js_space(s, k + 1);
            if let Some(end) = attribute_value(s, v) {
                i = end;
            }
        }
    }
}

fn attribute_value(s: &str, i: usize) -> Option<usize> {
    let first = char_at(s, i)?;
    match first {
        '\'' | '"' => s[i + 1..].find(first).map(|k| i + 1 + k + 1),
        '=' | '<' | '>' | '`' => None,
        c if js_space(c) => None,
        _ => {
            let mut j = i;
            while let Some(c) = char_at(s, j) {
                if js_space(c) || matches!(c, '"' | '\'' | '=' | '<' | '>' | '`') {
                    break;
                }
                j += c.len_utf8();
            }
            Some(j)
        }
    }
}

/// Whether line `text` ends an HTML block of `style` (`HTMLBlockStyle[style][1]`).
pub(crate) fn block_end(style: usize, text: &str) -> bool {
    match style {
        // /<\/(?:script|pre|style)>/i
        0 => text.match_indices("</").any(|(i, _)| {
            ["script", "pre", "style"].iter().any(|name| {
                starts_with_ci(text, i + 2, name) && byte(text, i + 2 + name.len()) == Some(b'>')
            })
        }),
        1 => text.contains("-->"),
        2 => text.contains("?>"),
        3 => text.contains('>'),
        4 => text.contains("]]>"),
        // /^[ \t]*$/
        _ => text.bytes().all(|b| b == b' ' || b == b'\t'),
    }
}

/// The inline `<…>` autolink regex on `after` (the text after `<`): the
/// match length, closing `>` included.
pub(crate) fn inline_autolink(after: &str) -> Option<usize> {
    // [a-z][-\w+.]+:[^\s>]+> — the scheme run ends where `:` must be, and
    // the address run ends where `>` must be.
    if byte(after, 0).is_some_and(|b| b.is_ascii_alphabetic()) {
        let j = run(after, 1, |b| word(b) || b == b'-' || b == b'+' || b == b'.');
        if j > 1 && byte(after, j) == Some(b':') {
            let mut k = j + 1;
            while let Some(c) = char_at(after, k) {
                if c == '>' || js_space(c) {
                    break;
                }
                k += c.len_utf8();
            }
            if k > j + 1 && byte(after, k) == Some(b'>') {
                return Some(k + 1);
            }
        }
    }
    // [a-z\d.!#$%&'*+/=?^_`{|}~-]+@ label (\. label)* > with
    // label = [a-z\d](?:[a-z\d-]{0,61}[a-z\d])?: each label must take its
    // whole [a-z\d-] run, since only `.` or `>` may follow it.
    let local = run(after, 0, |b| {
        b.is_ascii_alphanumeric() || b"!#$%&'*+/=?^_`{|}~.-".contains(&b)
    });
    if local == 0 || byte(after, local) != Some(b'@') {
        return None;
    }
    let mut i = local + 1;
    loop {
        let end = run(after, i, |b| b.is_ascii_alphanumeric() || b == b'-');
        let label = &after.as_bytes()[i..end];
        let ok = !label.is_empty()
            && label.len() <= 63
            && label[0].is_ascii_alphanumeric()
            && label[label.len() - 1].is_ascii_alphanumeric();
        if !ok {
            return None;
        }
        match byte(after, end) {
            Some(b'>') => return Some(end + 1),
            Some(b'.') => i = end + 1,
            _ => return None,
        }
    }
}

/// /^!--[^>](?:-[^-]|[^-])*?-->/i on `after`: the match length.
pub(crate) fn inline_comment(after: &str) -> Option<usize> {
    if !after.starts_with("!--") {
        return None;
    }
    let first = char_at(after, 3)?;
    if first == '>' {
        return None;
    }
    let mut i = 3 + first.len_utf8();
    loop {
        if after[i..].starts_with("-->") {
            return Some(i + 3);
        }
        let c = char_at(after, i)?;
        if c == '-' {
            let next = char_at(after, i + 1)?;
            if next == '-' {
                return None;
            }
            i += 1 + next.len_utf8();
        } else {
            i += c.len_utf8();
        }
    }
}

/// /^\?[^]*?\?>/ on `after`.
pub(crate) fn inline_processing(after: &str) -> Option<usize> {
    if !after.starts_with('?') {
        return None;
    }
    after[1..].find("?>").map(|k| 1 + k + 2)
}

/// The inline HTML tag regex on `after`:
/// /^(?:![A-Z][^]*?>|!\[CDATA\[[^]*?\]\]>|\/\s*[a-zA-Z][\w-]*\s*>|\s*[a-zA-Z][\w-]*(attrs)*\s*(\/\s*)?>)/
pub(crate) fn inline_tag(after: &str) -> Option<usize> {
    if after.starts_with('!') {
        if byte(after, 1).is_some_and(|b| b.is_ascii_uppercase()) {
            return after[2..].find('>').map(|k| 2 + k + 1);
        }
        if let Some(rest) = after.strip_prefix("![CDATA[") {
            return rest.find("]]>").map(|k| 8 + k + 3);
        }
        return None;
    }
    if after.starts_with('/') {
        let i = skip_js_space(after, 1);
        let j = tag_name(after, i)?;
        let j = skip_js_space(after, j);
        return (byte(after, j) == Some(b'>')).then_some(j + 1);
    }
    let i = skip_js_space(after, 0);
    let j = tag_name(after, i)?;
    let j = attributes(after, j, true);
    let mut j = skip_js_space(after, j);
    if byte(after, j) == Some(b'/') {
        j = skip_js_space(after, j + 1);
    }
    (byte(after, j) == Some(b'>')).then_some(j + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_tags() {
        assert_eq!(inline_tag("a href=\"x\">rest"), Some(11));
        assert_eq!(inline_tag(" a/>"), Some(4));
        assert_eq!(inline_tag("a b= >"), None);
        assert_eq!(inline_tag("/ a >"), Some(5));
        assert_eq!(inline_tag("!DOCTYPE html>"), Some(14));
        assert_eq!(inline_comment("!-- x -->"), Some(9));
        assert_eq!(inline_comment("!---->"), None);
        assert_eq!(inline_autolink("http://a.b>"), Some(11));
        assert_eq!(inline_autolink("me@ex-ample.com>"), Some(16));
        assert_eq!(inline_autolink("me@-x.com>"), None);
    }

    #[test]
    fn block_starts() {
        assert_eq!(block_start("<div>", false), Some(5));
        assert_eq!(block_start("<DIV class=x>", true), Some(5));
        assert_eq!(block_start("<span a='1'>  ", false), Some(6));
        assert_eq!(block_start("<span a='1'> x", false), None);
        assert_eq!(block_start("<!-- c", false), Some(1));
        assert_eq!(block_start("<Script>", false), Some(0));
    }
}
