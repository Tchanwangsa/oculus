//! Link reference definitions, and the URL, title and label parsers the
//! inline parser reuses for links.

use super::{BlockContext, LeafBlock, skip_space};
use crate::markdown::chars;
use crate::markdown::chars::space;
use crate::markdown::tables::NodeType as T;
use crate::markdown::tree::Elt;

/// A `parseURL`/`parseLinkTitle`/`parseLinkLabel` result: Lezer's element,
/// `null` (ran off the end) or `false` (failed).
pub(crate) enum Parsed {
    Elt(Elt),
    End,
    Fail,
}

/// `LinkReferenceParser`: a state machine over the leaf's growing content.
pub(crate) struct LinkReference {
    /// -1 failed, 0 start, 1 label, 2 link, 3 title.
    stage: i32,
    elts: Vec<Elt>,
    pos: usize,
    start: usize,
}

impl LinkReference {
    pub(super) fn new(leaf: &LeafBlock) -> Self {
        let mut r = LinkReference {
            stage: 0,
            elts: Vec::new(),
            pos: 0,
            start: leaf.start,
        };
        r.advance(&leaf.content);
        r
    }

    pub(super) fn next_line(&mut self, cx: &mut BlockContext, leaf: &LeafBlock) -> bool {
        if self.stage == -1 {
            return false;
        }
        let content = format!("{}\n{}", leaf.content, cx.line.scrub());
        let finish = self.advance(&content);
        if finish > -1 && (finish as usize) < content.len() {
            return self.complete(cx, leaf, finish as usize);
        }
        false
    }

    pub(super) fn finish(mut self, cx: &mut BlockContext, leaf: &LeafBlock) -> bool {
        if (self.stage == 2 || self.stage == 3)
            && skip_space(&leaf.content, self.pos) == leaf.content.len()
        {
            let len = leaf.content.len();
            return self.complete(cx, leaf, len);
        }
        false
    }

    fn complete(&mut self, cx: &mut BlockContext, leaf: &LeafBlock, len: usize) -> bool {
        let elts = std::mem::take(&mut self.elts);
        cx.add_leaf_element(
            leaf,
            Elt::with(T::LinkReference, self.start, self.start + len, elts),
        );
        true
    }

    fn next_stage(&mut self, elt: Parsed) -> bool {
        match elt {
            Parsed::Elt(e) => {
                self.pos = e.to - self.start;
                self.elts.push(e);
                self.stage += 1;
                true
            }
            Parsed::Fail => {
                self.stage = -1;
                false
            }
            Parsed::End => false,
        }
    }

    fn advance(&mut self, content: &str) -> i64 {
        loop {
            match self.stage {
                -1 => return -1,
                0 => {
                    if !self.next_stage(parse_link_label(content, self.pos, self.start, true)) {
                        return -1;
                    }
                    if content.as_bytes().get(self.pos) != Some(&b':') {
                        self.stage = -1;
                        return -1;
                    }
                    let at = self.pos + self.start;
                    self.elts.push(Elt::new(T::LinkMark, at, at + 1));
                    self.pos += 1;
                }
                1 => {
                    if !self.next_stage(parse_url(
                        content,
                        skip_space(content, self.pos),
                        self.start,
                    )) {
                        return -1;
                    }
                }
                2 => {
                    let skip = skip_space(content, self.pos);
                    let mut end = 0;
                    if skip > self.pos
                        && let Parsed::Elt(title) = parse_link_title(content, skip, self.start)
                    {
                        let title_end = line_end(content, title.to - self.start);
                        if title_end > 0 {
                            self.next_stage(Parsed::Elt(title));
                            end = title_end;
                        }
                    }
                    if end == 0 {
                        end = line_end(content, self.pos);
                    }
                    return if end > 0 && (end as usize) < content.len() {
                        end
                    } else {
                        -1
                    };
                }
                _ => return line_end(content, self.pos),
            }
        }
    }
}

/// Lezer's `lineEnd`: the newline after only spaces from `pos`, or the
/// end; -1 if anything else comes first.
fn line_end(text: &str, mut pos: usize) -> i64 {
    let b = text.as_bytes();
    while pos < b.len() {
        let next = b[pos];
        if next == b'\n' {
            break;
        }
        if !space(next as i32) {
            return -1;
        }
        pos += 1;
    }
    pos as i64
}

/// `parseURL` on `text` from `start`; `offset` is `text`'s document position.
pub(crate) fn parse_url(text: &str, start: usize, offset: usize) -> Parsed {
    let b = text.as_bytes();
    if b.get(start) == Some(&b'<') {
        for (pos, &ch) in b.iter().enumerate().skip(start + 1) {
            match ch {
                b'>' => return Parsed::Elt(Elt::new(T::URL, start + offset, pos + 1 + offset)),
                b'<' | b'\n' => return Parsed::Fail,
                _ => {}
            }
        }
        return Parsed::End;
    }
    let (mut depth, mut pos, mut escaped) = (0, start, false);
    while pos < b.len() {
        let ch = b[pos];
        if space(ch as i32) {
            break;
        } else if escaped {
            escaped = false;
        } else if ch == b'(' {
            depth += 1;
        } else if ch == b')' {
            if depth == 0 {
                break;
            }
            depth -= 1;
        } else if ch == b'\\' {
            escaped = true;
        }
        pos += 1;
    }
    if pos > start {
        Parsed::Elt(Elt::new(T::URL, start + offset, pos + offset))
    } else if pos >= b.len() {
        Parsed::End
    } else {
        Parsed::Fail
    }
}

pub(crate) fn parse_link_title(text: &str, start: usize, offset: usize) -> Parsed {
    let b = text.as_bytes();
    let next = b.get(start).copied();
    let end = match next {
        Some(b'\'') => b'\'',
        Some(b'"') => b'"',
        Some(b'(') => b')',
        _ => return Parsed::Fail,
    };
    let mut escaped = false;
    for (pos, &ch) in b.iter().enumerate().skip(start + 1) {
        if escaped {
            escaped = false;
        } else if ch == end {
            return Parsed::Elt(Elt::new(T::LinkTitle, start + offset, pos + 1 + offset));
        } else if ch == b'\\' {
            escaped = true;
        }
    }
    Parsed::End
}

/// `parseLinkLabel`: scans at most 999 UTF-16 units after the `[` at `start`.
pub(crate) fn parse_link_label(
    text: &str,
    start: usize,
    offset: usize,
    mut require_non_ws: bool,
) -> Parsed {
    let b = text.as_bytes();
    let (mut escaped, mut pos, mut units) = (false, start + 1, 0);
    while pos < b.len() && units < 999 {
        let ch = b[pos];
        if escaped {
            escaped = false;
        } else if ch == b']' {
            return if require_non_ws {
                Parsed::Fail
            } else {
                Parsed::Elt(Elt::new(T::LinkLabel, start + offset, pos + 1 + offset))
            };
        } else {
            if require_non_ws && !space(ch as i32) {
                require_non_ws = false;
            }
            if ch == b'[' {
                return Parsed::Fail;
            } else if ch == b'\\' {
                escaped = true;
            }
        }
        units += chars::units(ch);
        pos += 1;
    }
    Parsed::End
}

/// The byte offset of UTF-16 offset `units` in `s`. Inside a surrogate pair
/// it is a continuation byte of that char, which every caller treats as JS
/// treats the low surrogate: an ordinary non-space char.
pub(super) fn byte_at_units(s: &str, units: usize) -> usize {
    let mut seen = 0;
    for (i, c) in s.char_indices() {
        if seen >= units {
            return if seen == units { i } else { i - 1 };
        }
        seen += c.len_utf16();
    }
    if seen >= units {
        s.len()
    } else {
        s.len() + units - seen
    }
}
