//! A sheet's formulas as one line per copied-down or -across pattern.

use std::collections::{HashMap, HashSet};

use calamine::Range;

/// Lines listed under one sheet's table before the rest are counted.
const FORMULA_LINES: usize = 200;
/// Blocks named on one line before the rest are counted.
const LINE_BLOCKS: usize = 20;

/// A formula as written, without ods's `of:` namespace or a leading `=`;
/// `None` for none, and for the placeholder calamine writes for an xls
/// formula it cannot decode.
pub(super) fn formula_body(raw: &str) -> Option<&str> {
    let body = raw.trim();
    let body = body.strip_prefix("of:").unwrap_or(body);
    let body = body.strip_prefix('=').unwrap_or(body).trim();
    (!body.is_empty() && !body.starts_with("Unrecognised formula")).then_some(body)
}

/// The sheet's formulas as a bullet list, one line per pattern in order of
/// its first cell: cells whose formulas match once references are made
/// relative ([`relative`]) — a run copied down or across — share a line
/// naming the blocks they cover and the first cell's formula. `None` when the
/// sheet has none.
pub(super) fn formula_list(formulas: &Range<String>) -> Option<String> {
    let origin = formulas.start()?;
    let mut patterns: Vec<(&str, Vec<(u32, u32)>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    // Row-major, so a pattern's first cell decides its place.
    for (row, col, raw) in formulas.used_cells() {
        let Some(body) = formula_body(raw) else {
            continue;
        };
        let pos = (origin.0 + row as u32, origin.1 + col as u32);
        let at = *index.entry(relative(body, pos)).or_insert_with(|| {
            patterns.push((body, Vec::new()));
            patterns.len() - 1
        });
        patterns[at].1.push(pos);
    }
    if patterns.is_empty() {
        return None;
    }
    let mut lines: Vec<String> = patterns
        .iter()
        .take(FORMULA_LINES)
        .map(|(body, cells)| {
            let blocks = blocks(cells);
            let mut names: Vec<String> = blocks
                .iter()
                .take(LINE_BLOCKS)
                .map(|&(a, b)| block_name(a, b))
                .collect();
            if blocks.len() > LINE_BLOCKS {
                names.push(format!("… and {} more", blocks.len() - LINE_BLOCKS));
            }
            let body = body.replace("\r\n", " ").replace(['\n', '\r'], " ");
            format!("- {}: ={body}", names.join(", "))
        })
        .collect();
    if patterns.len() > FORMULA_LINES {
        lines.push(format!("- … and {} more", patterns.len() - FORMULA_LINES));
    }
    Some(lines.join("\n"))
}

/// Cover `cells` (row-major) with rectangles, each the larger of the one
/// reached by running down from its first cell and then widening, or by
/// running across and then deepening.
pub(super) fn blocks(cells: &[(u32, u32)]) -> Vec<((u32, u32), (u32, u32))> {
    let mut left: HashSet<(u32, u32)> = cells.iter().copied().collect();
    let mut out = Vec::new();
    for &(row, col) in cells {
        if !left.contains(&(row, col)) {
            continue;
        }
        let down_first = {
            let mut bottom = row;
            while left.contains(&(bottom + 1, col)) {
                bottom += 1;
            }
            let mut right = col;
            while (row..=bottom).all(|r| left.contains(&(r, right + 1))) {
                right += 1;
            }
            (bottom, right)
        };
        let across_first = {
            let mut right = col;
            while left.contains(&(row, right + 1)) {
                right += 1;
            }
            let mut bottom = row;
            while (col..=right).all(|c| left.contains(&(bottom + 1, c))) {
                bottom += 1;
            }
            (bottom, right)
        };
        let area =
            |(bottom, right): (u32, u32)| (bottom - row + 1) as u64 * (right - col + 1) as u64;
        let end = if area(across_first) > area(down_first) {
            across_first
        } else {
            down_first
        };
        for r in row..=end.0 {
            for c in col..=end.1 {
                left.remove(&(r, c));
            }
        }
        out.push(((row, col), end));
    }
    out
}

/// `D9`, or `D9:E14` for a block.
pub(super) fn block_name(start: (u32, u32), end: (u32, u32)) -> String {
    if start == end {
        cell_name(start)
    } else {
        format!("{}:{}", cell_name(start), cell_name(end))
    }
}

/// A zero-based (row, column) as an A1 name.
fn cell_name((row, col): (u32, u32)) -> String {
    format!("{}{}", column_name(col), row + 1)
}

/// A zero-based column as letters: 0 is `A`, 26 is `AA`.
pub(super) fn column_name(col: u32) -> String {
    let mut n = col + 1;
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    letters.iter().rev().map(|&b| b as char).collect()
}

/// `formula` with each A1 reference rewritten relative to `at`, the cell it
/// sits in — `G9` in D9 reads `R[0]C[3]`, `$G$9` reads `R9C7` — so copies of
/// one formula down or across compare equal. "Strings", 'quoted' sheet names,
/// function names and `Sheet!` prefixes are kept as written.
pub(super) fn relative(formula: &str, at: (u32, u32)) -> String {
    let bytes = formula.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$' || !b.is_ascii();
    let mut out = String::with_capacity(formula.len() + 16);
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let end = if b == b'"' || b == b'\'' {
            // A doubled quote inside is an escaped one and closes nothing.
            let mut j = i + 1;
            while j < bytes.len() {
                if bytes[j] == b && bytes.get(j + 1) == Some(&b) {
                    j += 2;
                } else if bytes[j] == b {
                    j += 1;
                    break;
                } else {
                    j += 1;
                }
            }
            out.push_str(&formula[i..j]);
            j
        } else if word(b) {
            let mut j = i;
            while j < bytes.len() && word(bytes[j]) {
                j += 1;
            }
            let token = &formula[i..j];
            // `LOG10(` is a function and `A1!` a sheet name, not references.
            let reference = match bytes.get(j) {
                Some(b'(' | b'!') => None,
                _ => reference(token),
            };
            match reference {
                Some((row_abs, row, col_abs, col)) => {
                    let part = |abs: bool, n: u32, here: u32, axis: char| {
                        if abs {
                            format!("{axis}{}", n + 1)
                        } else {
                            format!("{axis}[{}]", n as i64 - here as i64)
                        }
                    };
                    out.push_str(&part(row_abs, row, at.0, 'R'));
                    out.push_str(&part(col_abs, col, at.1, 'C'));
                }
                None => out.push_str(token),
            }
            j
        } else {
            out.push(b as char);
            i + 1
        };
        i = end;
    }
    out
}

/// An A1 reference such as `$G9` as (row absolute, zero-based row, column
/// absolute, zero-based column); `None` for any other word or one past the
/// sheet's bounds.
fn reference(token: &str) -> Option<(bool, u32, bool, u32)> {
    let col_abs = token.starts_with('$');
    let rest = &token[col_abs as usize..];
    let letters = rest.bytes().take_while(u8::is_ascii_uppercase).count();
    if !(1..=3).contains(&letters) {
        return None;
    }
    let col = rest[..letters]
        .bytes()
        .fold(0u32, |n, b| n * 26 + (b - b'A' + 1) as u32)
        - 1;
    let rest = &rest[letters..];
    let row_abs = rest.starts_with('$');
    let digits = &rest[row_abs as usize..];
    if digits.is_empty() || digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let row: u32 = digits.parse().ok()?;
    (col < 16_384 && row <= 1_048_576).then_some((row_abs, row - 1, col_abs, col))
}
