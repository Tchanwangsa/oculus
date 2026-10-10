//! Reading the formula's text around a node: the parse tree records where
//! nodes are, not where their braces, script operators or delimiters are.

use core::ops::Range;

use katex::{
    parser::parse_node::AnyParseNode,
    types::{ErrorLocationProvider as _, SourceLocation},
};

/// The formula being laid out.
#[derive(Clone, Copy)]
pub struct Source<'a>(pub &'a str);

impl<'a> Source<'a> {
    pub fn text(self, range: Range<usize>) -> &'a str {
        &self.0[range]
    }

    /// The node's byte range, when it has one in this formula.
    pub fn range(self, node: &AnyParseNode) -> Option<Range<usize>> {
        let loc: &SourceLocation = node.loc()?;
        (&*loc.input == self.0 && loc.start <= loc.end && loc.end <= self.0.len())
            .then_some(loc.start..loc.end)
    }

    /// The control word at `at` (`\frac`, without its trailing spaces).
    pub fn command_at(self, at: usize) -> Option<&'a str> {
        let rest = self.0.get(at..)?;
        let name = rest.strip_prefix('\\')?;
        let len = name
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(name.len());
        (len > 0).then(|| &rest[..=len])
    }

    /// The end of the token at `at`: a control word with the spaces that
    /// end it (as the lexer reads it), a control symbol, or one character.
    pub fn token_end(self, at: usize) -> usize {
        let Some(rest) = self.0.get(at..) else {
            return self.0.len();
        };
        if let Some(word) = self.command_at(at) {
            let after = at + word.len();
            return after + self.0[after..].len() - self.0[after..].trim_start().len();
        }
        let mut chars = rest.chars();
        match chars.next() {
            Some('\\') => at + 1 + chars.next().map_or(0, char::len_utf8),
            Some(c) => at + c.len_utf8(),
            None => at,
        }
    }

    /// The interiors of the top-level `{…}` groups in `range`, in order.
    /// Escaped braces and `%` comments are skipped; an unclosed group ends
    /// the scan.
    pub fn groups(self, range: Range<usize>) -> Vec<Range<usize>> {
        let mut out = Vec::new();
        let mut depth = 0usize;
        let mut open = 0;
        let mut chars = self.0[range.clone()].char_indices();
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' => {
                    chars.next();
                }
                '%' => {
                    for (_, c) in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                '{' => {
                    if depth == 0 {
                        open = range.start + i + 1;
                    }
                    depth += 1;
                }
                '}' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        out.push(open..range.start + i);
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The interior of the `{…}` group that ends `range` (spaces after it
    /// allowed): a command's last argument.
    pub fn last_group(self, range: Range<usize>) -> Option<Range<usize>> {
        let end = range.start + self.0[range.clone()].trim_end().len();
        self.groups(range)
            .pop()
            .filter(|group| group.end + 1 == end)
    }

    /// The `^` or `_` written before `at`, spaces between allowed, at or
    /// after `floor`: where a script's argument starts.
    pub fn script_op(self, at: usize, floor: usize) -> Option<(usize, char)> {
        let before = self.0.get(floor..at)?.trim_end();
        let op = before.chars().next_back()?;
        matches!(op, '^' | '_').then(|| (floor + before.len() - 1, op))
    }

    /// Whether `range` is written as `open…close` around its content.
    pub fn wrapped(self, range: &Range<usize>, open: char, close: char) -> bool {
        let text = self.text(range.clone());
        text.len() >= 2 && text.starts_with(open) && text.ends_with(close)
    }

    /// Whether `range` has text other than spaces outside every range in
    /// `covered` (sorted, disjoint).
    pub fn uncovered(self, range: Range<usize>, covered: &[Range<usize>]) -> bool {
        let mut at = range.start;
        for part in covered.iter().chain([&(range.end..range.end)]) {
            if part.start > at && !self.0[at..part.start.min(range.end)].trim().is_empty() {
                return true;
            }
            at = at.max(part.end);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::Source;

    #[test]
    fn groups_skip_escaped_braces() {
        let src = Source(r"\textcolor{red}{a\{b\}}");
        assert_eq!(src.groups(0..src.0.len()), [11..14, 16..22]);
        assert_eq!(src.last_group(0..src.0.len()), Some(16..22));
    }

    #[test]
    fn control_words_take_their_spaces() {
        let src = Source(r"\left\langle  x");
        assert_eq!(src.token_end(5), 14);
        assert_eq!(src.token_end(0), 5);
        assert_eq!(Source(r"\left(x").token_end(5), 6);
        assert_eq!(Source(r"\,x").token_end(0), 2);
    }

    #[test]
    fn script_operators() {
        let src = Source("x^ {2}");
        assert_eq!(src.script_op(3, 0), Some((1, '^')));
        assert_eq!(Source("x'").script_op(1, 0), None);
    }
}
