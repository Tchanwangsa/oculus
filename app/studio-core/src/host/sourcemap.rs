//! Generated-JS position -> TSX position, from oxc's source map tokens.

/// Tokens as (js line, js col, tsx line, tsx col), all 0-based, sorted by
/// JS position.
#[derive(Clone, Debug, Default)]
pub struct LineMap {
    tokens: Vec<(u32, u32, u32, u32)>,
}

impl LineMap {
    pub fn new(tokens: impl Iterator<Item = (u32, u32, u32, u32)>) -> LineMap {
        let mut tokens: Vec<_> = tokens.collect();
        tokens.sort_unstable();
        LineMap { tokens }
    }

    /// The TSX (line, col), 1-based, for a 1-based JS (line, col): the last
    /// token at or before it.
    pub fn lookup(&self, js_line: u32, js_col: u32) -> Option<(u32, u32)> {
        let key = (js_line.saturating_sub(1), js_col.saturating_sub(1));
        let i = self
            .tokens
            .partition_point(|t| (t.0, t.1) <= key)
            .checked_sub(1)?;
        let t = self.tokens.get(i)?;
        Some((t.2 + 1, t.3 + 1))
    }
}
