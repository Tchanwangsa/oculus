//! Iterators over a `Text`: raw leaf chunks, CodeMirror-style runs with
//! separate line breaks (`iter`/`iterRange`), and lines (`iterLines`).

use std::borrow::Cow;

use super::tree::{Bias, Cursor, Dim, Node};

/// The stored chunks over a byte range, forwards or backwards.
#[derive(Clone)]
pub struct Chunks<'a> {
    cursor: Cursor<'a>,
    from: usize,
    to: usize,
    forward: bool,
    done: bool,
}

impl<'a> Chunks<'a> {
    pub(super) fn new(root: &'a Node, from: usize, to: usize, forward: bool) -> Chunks<'a> {
        let cursor = if forward {
            Cursor::seek(root, Dim::Bytes, from, Bias::Right)
        } else {
            Cursor::seek(root, Dim::Bytes, to, Bias::Left)
        };
        Chunks {
            cursor,
            from,
            to,
            forward,
            done: from >= to,
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        while !self.done {
            let text = self.cursor.text();
            let start = self.cursor.start.bytes;
            let end = start + text.len();
            let (lo, hi) = (start.max(self.from), end.min(self.to));
            self.done = if self.forward {
                end >= self.to || !self.cursor.next()
            } else {
                start <= self.from || !self.cursor.prev()
            };
            if lo < hi {
                return Some(&text[lo - start..hi - start]);
            }
        }
        None
    }
}

/// Runs of text that never contain `\n`, with each line break yielded alone as
/// `"\n"`. Backwards, runs come last-first but each reads forwards.
#[derive(Clone)]
pub struct Iter<'a> {
    chunks: Chunks<'a>,
    rest: &'a str,
}

impl<'a> Iter<'a> {
    pub(super) fn new(chunks: Chunks<'a>) -> Iter<'a> {
        Iter { chunks, rest: "" }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            self.rest = self.chunks.next()?;
        }
        let rest = self.rest;
        let (piece, left) = if self.chunks.forward {
            let cut = match rest.find('\n') {
                Some(0) => 1,
                Some(i) => i,
                None => rest.len(),
            };
            (&rest[..cut], &rest[cut..])
        } else {
            let cut = match rest.rfind('\n') {
                Some(i) if i + 1 == rest.len() => i,
                Some(i) => i + 1,
                None => 0,
            };
            (&rest[cut..], &rest[..cut])
        };
        self.rest = left;
        Some(piece)
    }
}

/// The lines of a range: its text split on `\n`, so `n` breaks give `n + 1`
/// lines and an empty range gives one empty line. A line that spans leaves is
/// joined into an owned string; otherwise it borrows.
#[derive(Clone)]
pub struct Lines<'a> {
    inner: Iter<'a>,
    done: bool,
}

impl<'a> Lines<'a> {
    pub(super) fn new(inner: Iter<'a>) -> Lines<'a> {
        Lines { inner, done: false }
    }
}

impl<'a> Iterator for Lines<'a> {
    type Item = Cow<'a, str>;

    fn next(&mut self) -> Option<Cow<'a, str>> {
        if self.done {
            return None;
        }
        let mut line: Cow<'a, str> = Cow::Borrowed("");
        loop {
            match self.inner.next() {
                Some("\n") => return Some(line),
                Some(run) if line.is_empty() => line = Cow::Borrowed(run),
                Some(run) => line.to_mut().push_str(run),
                None => {
                    self.done = true;
                    return Some(line);
                }
            }
        }
    }
}
