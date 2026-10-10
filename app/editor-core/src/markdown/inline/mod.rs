//! The inline parser: `@lezer/markdown`'s `InlineContext` with the app's
//! inline extensions, tried at each position in configured order:
//! ParenMath, Escape, Entity, InlineCode, HTMLTag, Emphasis, Strikethrough,
//! HardBreak, Link, Image, Autolink, InlineMath, LinkEnd.
//!
//! Emphasis and strikethrough runs and link openers go on a delimiter list
//! (`parts`) that `resolve_markers` turns into nodes. Lezer never consults
//! link reference definitions: any well-formed `[…]` is a `Link`. Offsets
//! are bytes; where Lezer reads one UTF-16 unit (the chars around a
//! delimiter run), `chars` answers for the char there.

mod autolink;
mod marks;
mod math;

use super::block::{Parsed, parse_link_label, parse_link_title, parse_url, skip_space};
use super::chars::{self, js_punctuation, js_space, word};
use super::html;
use super::tables::NodeType as T;
use super::tree::Elt;
use autolink::autolink;
use math::{dollar_math, paren_math};

pub(crate) use marks::inject_marks;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DelimType {
    EmphasisUnderscore,
    EmphasisAsterisk,
    LinkStart,
    ImageStart,
    Strikethrough,
}

impl DelimType {
    fn resolves(self) -> bool {
        !matches!(self, DelimType::LinkStart | DelimType::ImageStart)
    }

    fn mark(self) -> T {
        if self == DelimType::Strikethrough {
            T::StrikethroughMark
        } else {
            T::EmphasisMark
        }
    }
}

const OPEN: u8 = 1;
const CLOSE: u8 = 2;

#[derive(Debug, Clone)]
struct Delim {
    kind: DelimType,
    from: usize,
    to: usize,
    side: u8,
}

#[derive(Debug, Clone)]
enum Part {
    Elt(Elt),
    Delim(Delim),
}

impl Part {
    fn to(&self) -> usize {
        match self {
            Part::Elt(e) => e.to,
            Part::Delim(d) => d.to,
        }
    }
}

struct InlineContext<'a> {
    text: &'a str,
    offset: usize,
    parts: Vec<Option<Part>>,
}

impl<'a> InlineContext<'a> {
    fn end(&self) -> usize {
        self.offset + self.text.len()
    }

    /// Byte at document position `pos`, or -1 outside the text.
    fn char(&self, pos: usize) -> i32 {
        if pos < self.offset || pos >= self.end() {
            -1
        } else {
            self.text.as_bytes()[pos - self.offset] as i32
        }
    }

    fn rel(&self, pos: usize) -> usize {
        pos - self.offset
    }

    fn append(&mut self, part: Part) -> Option<usize> {
        let to = part.to();
        self.parts.push(Some(part));
        Some(to)
    }

    fn skip_space(&self, from: usize) -> usize {
        skip_space(self.text, from - self.offset) + self.offset
    }

    fn has_open_link(&self) -> bool {
        self.parts.iter().rev().any(|p| {
            matches!(p, Some(Part::Delim(d)) if matches!(d.kind, DelimType::LinkStart | DelimType::ImageStart))
        })
    }

    /// The `(whitespace, punctuation)` class of the one-unit slice before
    /// `pos`; the empty slice at the text's start is whitespace.
    fn class_before(&self, pos: usize) -> (bool, bool) {
        if pos <= self.offset {
            return (true, false);
        }
        let c = chars::char_before(self.text, self.rel(pos)).unwrap();
        (js_space(c), js_punctuation(c))
    }

    /// The same for the one-unit slice at `pos`.
    fn class_at(&self, pos: usize) -> (bool, bool) {
        match chars::char_at(self.text, self.rel(pos)) {
            None => (true, false),
            Some(c) => (js_space(c), js_punctuation(c)),
        }
    }

    /// `resolveMarkers`: matches closing delimiters from `from` with their
    /// openers, and returns the elements left.
    fn resolve_markers(&mut self, from: usize) -> Vec<Elt> {
        let mut i = from;
        while i < self.parts.len() {
            let close = match &self.parts[i] {
                Some(Part::Delim(d)) if d.kind.resolves() && d.side & CLOSE != 0 => d.clone(),
                _ => {
                    i += 1;
                    continue;
                }
            };
            let emp = matches!(
                close.kind,
                DelimType::EmphasisUnderscore | DelimType::EmphasisAsterisk
            );
            let close_size = close.to - close.from;
            let mut open = None;
            let mut j = i;
            while j > from {
                j -= 1;
                if let Some(Part::Delim(part)) = &self.parts[j] {
                    let part_size = part.to - part.from;
                    if part.side & OPEN != 0
                        && part.kind == close.kind
                        && !(emp
                            && (close.side & OPEN != 0 || part.side & CLOSE != 0)
                            && (part_size + close_size) % 3 == 0
                            && (part_size % 3 != 0 || close_size % 3 != 0))
                    {
                        open = Some(part.clone());
                        break;
                    }
                }
            }
            let Some(open) = open else {
                i += 1;
                continue;
            };
            let mut kind = if close.kind == DelimType::Strikethrough {
                T::Strikethrough
            } else {
                T::Emphasis
            };
            let (mut start, mut end) = (open.from, close.to);
            if emp {
                let size = 2.min(open.to - open.from).min(close_size);
                start = open.to - size;
                end = close.from + size;
                kind = if size == 1 {
                    T::Emphasis
                } else {
                    T::StrongEmphasis
                };
            }
            let mut content = vec![Elt::new(open.kind.mark(), start, open.to)];
            for k in j + 1..i {
                if let Some(Part::Elt(e)) = self.parts[k].take() {
                    content.push(e);
                }
            }
            content.push(Elt::new(close.kind.mark(), close.from, end));
            let element = Elt::with(kind, start, end, content);
            self.parts[j] = (emp && open.from != start).then(|| {
                Part::Delim(Delim {
                    to: start,
                    ..open.clone()
                })
            });
            let keep = (emp && close.to != end).then(|| {
                Part::Delim(Delim {
                    from: end,
                    ..close.clone()
                })
            });
            if keep.is_some() {
                self.parts[i] = keep;
                self.parts.insert(i, Some(Part::Elt(element)));
            } else {
                self.parts[i] = Some(Part::Elt(element));
            }
            i += 1;
        }
        // Callers drop `parts[from..]` afterwards, so the elements move out.
        let from = from.min(self.parts.len());
        self.parts
            .drain(from..)
            .filter_map(|p| match p {
                Some(Part::Elt(e)) => Some(e),
                _ => None,
            })
            .collect()
    }

    fn take_content(&mut self, start: usize) -> Vec<Elt> {
        let content = self.resolve_markers(start);
        self.parts.truncate(start);
        content
    }
}

/// `MarkdownParser.parseInline`: the inline elements of `text`, which starts
/// at document position `offset`.
pub(crate) fn parse_inline(text: &str, offset: usize) -> Vec<Elt> {
    let mut cx = InlineContext {
        text,
        offset,
        parts: Vec::new(),
    };
    let mut pos = offset;
    'outer: while pos < cx.end() {
        let next = cx.char(pos);
        for parser in INLINE_PARSERS {
            if let Some(to) = parser(&mut cx, next, pos) {
                pos = to;
                continue 'outer;
            }
        }
        // To the next char; no parser can start inside one.
        pos += chars::char_at(text, pos - offset).map_or(1, char::len_utf8);
    }
    cx.resolve_markers(0)
}

type InlineParser = fn(&mut InlineContext, i32, usize) -> Option<usize>;

const INLINE_PARSERS: [InlineParser; 13] = [
    paren_math,
    escape,
    entity,
    inline_code,
    html_tag,
    emphasis,
    strikethrough,
    hard_break,
    link,
    image,
    autolink,
    dollar_math,
    link_end,
];

const ESCAPABLE: &[u8] = b"!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~";

fn escape(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b'\\' as i32 || start == cx.end() - 1 {
        return None;
    }
    let escaped = cx.char(start + 1);
    if escaped >= 0 && ESCAPABLE.contains(&(escaped as u8)) {
        return cx.append(Part::Elt(Elt::new(T::Escape, start, start + 2)));
    }
    None
}

/// /^(?:#\d+|#x[a-f\d]+|\w+);/i within the 30 units after `&`; the match is
/// ASCII, so 30 bytes.
fn entity_len(s: &[u8]) -> Option<usize> {
    let s = &s[..s.len().min(30)];
    let run = |from: usize, ok: fn(u8) -> bool| {
        let mut i = from;
        while i < s.len() && ok(s[i]) {
            i += 1;
        }
        i
    };
    let ends = |i: usize, from: usize| (i > from && s.get(i) == Some(&b';')).then_some(i + 1);
    if s.first() == Some(&b'#') {
        let d = run(1, |b| b.is_ascii_digit());
        if let Some(len) = ends(d, 1) {
            return Some(len);
        }
        if matches!(s.get(1), Some(b'x' | b'X')) {
            let h = run(2, |b| b.is_ascii_hexdigit());
            return ends(h, 2);
        }
        return None;
    }
    ends(run(0, word), 0)
}

fn entity(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b'&' as i32 {
        return None;
    }
    let rest = &cx.text.as_bytes()[cx.rel(start) + 1..];
    let len = entity_len(rest)?;
    cx.append(Part::Elt(Elt::new(T::Entity, start, start + 1 + len)))
}

fn inline_code(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b'`' as i32 || (start > 0 && cx.char(start - 1) == b'`' as i32) {
        return None;
    }
    let mut pos = start + 1;
    while pos < cx.end() && cx.char(pos) == b'`' as i32 {
        pos += 1;
    }
    let size = pos - start;
    let mut cur = 0;
    while pos < cx.end() {
        if cx.char(pos) == b'`' as i32 {
            cur += 1;
            if cur == size && cx.char(pos + 1) != b'`' as i32 {
                return cx.append(Part::Elt(Elt::with(
                    T::InlineCode,
                    start,
                    pos + 1,
                    vec![
                        Elt::new(T::CodeMark, start, start + size),
                        Elt::new(T::CodeMark, pos + 1 - size, pos + 1),
                    ],
                )));
            }
        } else {
            cur = 0;
        }
        pos += 1;
    }
    None
}

fn html_tag(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b'<' as i32 || start == cx.end() - 1 {
        return None;
    }
    let after = &cx.text[cx.rel(start) + 1..];
    if let Some(len) = html::inline_autolink(after) {
        return cx.append(Part::Elt(Elt::with(
            T::Autolink,
            start,
            start + 1 + len,
            vec![
                Elt::new(T::LinkMark, start, start + 1),
                Elt::new(T::URL, start + 1, start + len),
                Elt::new(T::LinkMark, start + len, start + 1 + len),
            ],
        )));
    }
    if let Some(len) = html::inline_comment(after) {
        return cx.append(Part::Elt(Elt::new(T::Comment, start, start + 1 + len)));
    }
    if let Some(len) = html::inline_processing(after) {
        return cx.append(Part::Elt(Elt::new(
            T::ProcessingInstruction,
            start,
            start + 1 + len,
        )));
    }
    let len = html::inline_tag(after)?;
    cx.append(Part::Elt(Elt::new(T::HTMLTag, start, start + 1 + len)))
}

fn emphasis(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b'_' as i32 && next != b'*' as i32 {
        return None;
    }
    let mut pos = start + 1;
    while cx.char(pos) == next {
        pos += 1;
    }
    let (s_before, p_before) = cx.class_before(start);
    let (s_after, p_after) = cx.class_at(pos);
    let left = !s_after && (!p_after || s_before || p_before);
    let right = !s_before && (!p_before || s_after || p_after);
    let star = next == b'*' as i32;
    let can_open = left && (star || !right || p_before);
    let can_close = right && (star || !left || p_after);
    let kind = if star {
        DelimType::EmphasisAsterisk
    } else {
        DelimType::EmphasisUnderscore
    };
    let side = if can_open { OPEN } else { 0 } | if can_close { CLOSE } else { 0 };
    cx.append(Part::Delim(Delim {
        kind,
        from: start,
        to: pos,
        side,
    }))
}

fn strikethrough(cx: &mut InlineContext, next: i32, pos: usize) -> Option<usize> {
    if next != b'~' as i32 || cx.char(pos + 1) != b'~' as i32 || cx.char(pos + 2) == b'~' as i32 {
        return None;
    }
    let (s_before, p_before) = cx.class_before(pos);
    let (s_after, p_after) = cx.class_at(pos + 2);
    let open = !s_after && (!p_after || s_before || p_before);
    let close = !s_before && (!p_before || s_after || p_after);
    let side = if open { OPEN } else { 0 } | if close { CLOSE } else { 0 };
    cx.append(Part::Delim(Delim {
        kind: DelimType::Strikethrough,
        from: pos,
        to: pos + 2,
        side,
    }))
}

fn hard_break(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next == b'\\' as i32 && cx.char(start + 1) == b'\n' as i32 {
        return cx.append(Part::Elt(Elt::new(T::HardBreak, start, start + 2)));
    }
    if next == b' ' as i32 {
        let mut pos = start + 1;
        while cx.char(pos) == b' ' as i32 {
            pos += 1;
        }
        if cx.char(pos) == b'\n' as i32 && pos >= start + 2 {
            return cx.append(Part::Elt(Elt::new(T::HardBreak, start, pos + 1)));
        }
    }
    None
}

fn link(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b'[' as i32 {
        return None;
    }
    cx.append(Part::Delim(Delim {
        kind: DelimType::LinkStart,
        from: start,
        to: start + 1,
        side: OPEN,
    }))
}

fn image(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b'!' as i32 || cx.char(start + 1) != b'[' as i32 {
        return None;
    }
    cx.append(Part::Delim(Delim {
        kind: DelimType::ImageStart,
        from: start,
        to: start + 2,
        side: OPEN,
    }))
}

fn link_end(cx: &mut InlineContext, next: i32, start: usize) -> Option<usize> {
    if next != b']' as i32 {
        return None;
    }
    for i in (0..cx.parts.len()).rev() {
        let Some(Part::Delim(part)) = &cx.parts[i] else {
            continue;
        };
        if !matches!(part.kind, DelimType::LinkStart | DelimType::ImageStart) {
            continue;
        }
        let part = part.clone();
        let after = cx.char(start + 1);
        if part.side == 0
            || (cx.skip_space(part.to) == start && after != b'(' as i32 && after != b'[' as i32)
        {
            cx.parts[i] = None;
            return None;
        }
        let content = cx.take_content(i);
        let kind = if part.kind == DelimType::LinkStart {
            T::Link
        } else {
            T::Image
        };
        let link = finish_link(cx, content, kind, part.from, start + 1);
        let to = link.to;
        cx.parts.push(Some(Part::Elt(link)));
        if part.kind == DelimType::LinkStart {
            for p in cx.parts[..i].iter_mut() {
                if let Some(Part::Delim(d)) = p
                    && d.kind == DelimType::LinkStart
                {
                    d.side = 0;
                }
            }
        }
        return Some(to);
    }
    None
}

fn finish_link(
    cx: &InlineContext,
    mut content: Vec<Elt>,
    kind: T,
    start: usize,
    start_pos: usize,
) -> Elt {
    let next = cx.char(start_pos);
    let mut end_pos = start_pos;
    let open_len = if kind == T::Image { 2 } else { 1 };
    content.insert(0, Elt::new(T::LinkMark, start, start + open_len));
    content.push(Elt::new(T::LinkMark, start_pos - 1, start_pos));
    if next == b'(' as i32 {
        let mut pos = cx.skip_space(start_pos + 1);
        let dest = match parse_url(cx.text, cx.rel(pos), cx.offset) {
            Parsed::Elt(e) => Some(e),
            _ => None,
        };
        let mut title = None;
        if let Some(dest) = &dest {
            pos = cx.skip_space(dest.to);
            if pos != dest.to
                && let Parsed::Elt(t) = parse_link_title(cx.text, cx.rel(pos), cx.offset)
            {
                pos = cx.skip_space(t.to);
                title = Some(t);
            }
        }
        if cx.char(pos) == b')' as i32 {
            content.push(Elt::new(T::LinkMark, start_pos, start_pos + 1));
            end_pos = pos + 1;
            content.extend(dest);
            content.extend(title);
            content.push(Elt::new(T::LinkMark, pos, end_pos));
        }
    } else if next == b'[' as i32
        && let Parsed::Elt(label) = parse_link_label(cx.text, cx.rel(start_pos), cx.offset, false)
    {
        end_pos = label.to;
        content.push(label);
    }
    Elt::with(kind, start, end_pos, content)
}
