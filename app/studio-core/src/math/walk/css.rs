//! KaTeX's inline style values. KaTeX writes lengths as `Nem` (or a bare
//! `0`) and the occasional percentage; nothing else reaches its inline
//! styles.

use katex::types::{CssProperty, CssStyle};

/// A CSS length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Len {
    /// In the element's own em.
    Em(f64),
    /// A fraction (0.5 for `50%`).
    Pct(f64),
}

pub fn parse(v: &str) -> Option<Len> {
    let v = v.trim();
    if let Some(n) = v.strip_suffix("em") {
        return n.trim().parse().ok().map(Len::Em);
    }
    if let Some(n) = v.strip_suffix('%') {
        return n.trim().parse::<f64>().ok().map(|p| Len::Pct(p / 100.0));
    }
    v.parse::<f64>()
        .ok()
        .filter(|n| *n == 0.0)
        .map(|_| Len::Em(0.0))
}

/// `prop` in em of the root size, given the element's `size`; a
/// percentage resolves against `of` (root em), and is 0 without it.
pub fn get(style: &CssStyle, prop: CssProperty, size: f64, of: Option<f64>) -> Option<f64> {
    match parse(style.get(prop)?)? {
        Len::Em(n) => Some(n * size),
        Len::Pct(p) => Some(p * of.unwrap_or(0.0)),
    }
}

/// Like `get`, 0 when the property is absent.
pub fn or_zero(style: &CssStyle, prop: CssProperty, size: f64) -> f64 {
    get(style, prop, size, None).unwrap_or(0.0)
}
