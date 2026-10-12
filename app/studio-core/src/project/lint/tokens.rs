//! Token use: a component's source names theme tokens, never literals.
//! Three rules, each an error at the literal's line and column:
//!
//! - a colour literal outside the argument of a `token(...)` default: `#rgb`,
//!   `#rrggbb[aa]`, `rgb(`, `hsl(`, `oklch(` and friends anywhere, and a
//!   CSS colour name as a whole string only where a colour goes (`places`);
//! - a font family string (a bundled family's name, or any string after
//!   `fontFamily:`);
//! - a number that is a length in a `style` object (`style` says which)
//!   and equals a theme `space` or `radius` entry. Other numbers stay
//!   allowed.
//!
//! The rules read the source through `tsx::scan`, so a literal inside a
//! template literal's `${...}` is not checked.
//!
//! `mono_note` is a note, not an error: monospace belongs to a component
//! named for code.

mod names;
mod places;
mod style;

use super::tsx::{Kind, Tok, line_text, scan};
use crate::error::{StudioError, TsxLoc};
use crate::theme::Theme;
use names::CSS_COLOUR_NAMES;
use places::{Place, places};
use serde_json::Value;
use std::collections::HashSet;
use style::style_numbers;

/// Every error the token rules find in `src`, in source order.
/// `colour_props` names the component's `token("color")` props.
pub fn check(
    component: &str,
    src: &str,
    theme: &Theme,
    colour_props: &[String],
) -> Vec<StudioError> {
    let palette = Palette::of(theme);
    let toks = scan(src);
    let exempt = exempt_flags(&toks);
    let place = places(&toks, colour_props);
    let checked = style_numbers(&toks);
    let mut out = Vec::new();
    let mut fonts_seen: HashSet<usize> = HashSet::new();
    // The position is the token's, moved right by `within` characters for a
    // literal inside a string.
    let err = |i: usize, within: usize, message: String| {
        let tok = &toks[i];
        StudioError::Component {
            component: component.to_owned(),
            message,
            loc: Some(TsxLoc {
                line: tok.line,
                col: tok.col + within as u32,
                snippet: line_text(src, tok.line).to_owned(),
            }),
        }
    };
    for (i, tok) in toks.iter().enumerate() {
        match &tok.kind {
            Kind::Str => {
                let family_key = i >= 2
                    && toks[i - 1].kind == Kind::Punct(':')
                    && toks[i - 2].kind == Kind::Ident
                    && toks[i - 2].text == "fontFamily";
                if family_key || palette.is_family(&tok.text) {
                    if fonts_seen.insert(i) {
                        out.push(err(i, 0, palette.font_message(&tok.text)));
                    }
                    continue;
                }
                if !exempt[i] {
                    let named = match place[i] {
                        Place::Colour => true,
                        Place::ColourProp => !palette.is_colour_token(&tok.text),
                        Place::Other => false,
                    };
                    for (at, lit) in colour_literals(&tok.text, named) {
                        let within = tok.text[..at].chars().count() + 1;
                        out.push(err(i, within, palette.colour_message(&lit)));
                    }
                }
            }
            Kind::Punct('#') if !exempt[i] => {
                // A hex colour in JSX text: the `#` and the hex digits after
                // it, read from the source (the scanner splits `5e6ad2`).
                let run = src[tok.end..]
                    .bytes()
                    .take_while(u8::is_ascii_hexdigit)
                    .count();
                let after = src.as_bytes().get(tok.end + run);
                let hex = &src[tok.end..tok.end + run];
                if is_hex(hex) && after.is_none_or(|b| !b.is_ascii_alphanumeric()) {
                    let lit = format!("#{hex}");
                    out.push(err(i, 0, palette.colour_message(&lit)));
                }
            }
            Kind::Number(v) if checked[i] => {
                if let Some(name) = palette.spacing_name(*v) {
                    out.push(err(
                        i,
                        0,
                        format!(
                            "style value {} equals theme.{name}; use theme.{name}",
                            tok.text
                        ),
                    ));
                }
            }
            _ => {}
        }
    }
    out
}

/// The note for a component that is not named for code but reads
/// `font.mono` or `type.code`: the first such place in `src`, or none.
pub fn mono_note(component: &str, src: &str) -> Option<StudioError> {
    if component.starts_with("Code") || component.ends_with("Code") {
        return None;
    }
    let toks = scan(src);
    let (i, token) = toks.iter().enumerate().find_map(|(i, tok)| {
        let owner = toks.get(i.checked_sub(2)?)?;
        let dotted = toks.get(i - 1)?.kind == Kind::Punct('.');
        let token = match (owner.text.as_str(), tok.text.as_str()) {
            ("font", "mono") => "font.mono",
            ("type", "code") => "type.code",
            _ => return None,
        };
        (tok.kind == Kind::Ident && owner.kind == Kind::Ident && dotted).then_some((i, token))
    })?;
    let tok = &toks[i];
    Some(StudioError::Component {
        component: component.to_owned(),
        message: format!(
            "theme.{token} is used outside a Code component; monospace is for code, and numbers take fontVariantNumeric: \"tabular-nums\""
        ),
        loc: Some(TsxLoc {
            line: tok.line,
            col: tok.col,
            snippet: line_text(src, tok.line).to_owned(),
        }),
    })
}

/// Which tokens sit inside the argument of a `token(...)` default: a
/// `token(...)` call, or the `.default(...)` chained on one.
fn exempt_flags(toks: &[Tok]) -> Vec<bool> {
    // Per open paren: (is a token(...) call, counts as exempt).
    let mut parens: Vec<(bool, bool)> = Vec::new();
    let mut last_closed_was_token = false;
    let mut exempt_depth = 0usize;
    let mut out = vec![false; toks.len()];
    for (i, tok) in toks.iter().enumerate() {
        out[i] = exempt_depth > 0;
        match tok.kind {
            Kind::Punct('(') => {
                let callee = match i.checked_sub(1).map(|j| &toks[j]) {
                    Some(t) if t.kind == Kind::Ident => t.text.as_str(),
                    _ => "",
                };
                let is_token = callee == "token";
                let chained = callee == "default"
                    && i >= 3
                    && toks[i - 2].kind == Kind::Punct('.')
                    && toks[i - 3].kind == Kind::Punct(')')
                    && last_closed_was_token;
                let exempt = is_token || chained;
                if exempt {
                    exempt_depth += 1;
                }
                parens.push((is_token, exempt));
            }
            Kind::Punct(')') => {
                if let Some((is_token, exempt)) = parens.pop() {
                    if exempt {
                        exempt_depth -= 1;
                    }
                    last_closed_was_token = is_token;
                } else {
                    last_closed_was_token = false;
                }
            }
            _ => {}
        }
    }
    out
}

/// The colour literals in one string, each with its byte offset in it: hex,
/// functional notation, or (when `named`) the whole string being a CSS
/// colour name.
fn colour_literals(s: &str, named: bool) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'#' {
            continue;
        }
        let run = bytes[i + 1..]
            .iter()
            .take_while(|c| c.is_ascii_hexdigit())
            .count();
        let boundary = bytes
            .get(i + 1 + run)
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        if matches!(run, 3 | 4 | 6 | 8) && boundary {
            out.push((i, s[i..i + 1 + run].to_owned()));
        }
    }
    let lower = s.to_ascii_lowercase();
    for func in [
        "rgb(", "rgba(", "hsl(", "hsla(", "hwb(", "lab(", "lch(", "oklab(", "oklch(",
    ] {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(func) {
            let at = from + pos;
            let boundary = at == 0 || !lower.as_bytes()[at - 1].is_ascii_alphanumeric();
            if boundary {
                let end = s[at..].find(')').map_or(s.len(), |e| at + e + 1);
                out.push((at, s[at..end].to_owned()));
            }
            from = at + func.len();
        }
    }
    let whole = s.trim().to_ascii_lowercase();
    if named && CSS_COLOUR_NAMES.contains(&whole.as_str()) {
        out.push((s.len() - s.trim_start().len(), s.trim().to_owned()));
    }
    out
}

fn is_hex(s: &str) -> bool {
    matches!(s.len(), 3 | 4 | 6 | 8) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The theme's names the messages point at.
struct Palette {
    colours: Vec<(String, Option<[f64; 3]>)>,
    spacing: Vec<(String, &'static str, f64)>,
    heading: String,
    body: String,
    mono: String,
    families: Vec<String>,
}

impl Palette {
    fn of(theme: &Theme) -> Palette {
        let section = |kind: &str| theme.section(kind).cloned().unwrap_or_default();
        let colours = section("color")
            .into_iter()
            .filter_map(|(name, v)| {
                let text = v.as_str()?;
                Some((name, rgb_of(text)))
            })
            .collect();
        let numbers = |kind: &str, group: &'static str| {
            section(kind)
                .into_iter()
                .filter_map(move |(name, v)| Some((name, group, v.as_f64()?)))
                .collect::<Vec<_>>()
        };
        let mut spacing = numbers("space", "space");
        spacing.extend(numbers("radius", "radius"));
        let font = |key: &str| {
            theme
                .tokens
                .pointer(&format!("/font/{key}"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Palette {
            colours,
            spacing,
            heading: font("heading"),
            body: font("body"),
            mono: font("mono"),
            families: crate::bundled_families(),
        }
    }

    fn is_colour_token(&self, s: &str) -> bool {
        self.colours.iter().any(|(n, _)| n == s)
    }

    fn is_family(&self, s: &str) -> bool {
        self.families.iter().any(|f| f == s)
    }

    fn font_message(&self, s: &str) -> String {
        if !self.heading.is_empty() && s == self.heading {
            return format!("font family `{s}` is a literal; use theme.font.heading");
        }
        if !self.body.is_empty() && s == self.body {
            return format!("font family `{s}` is a literal; use theme.font.body");
        }
        if !self.mono.is_empty() && s == self.mono {
            return format!("font family `{s}` is a literal; use theme.font.mono");
        }
        format!(
            "font family `{s}` is a literal; use theme.font.body, theme.font.heading or theme.font.mono (no system fonts)"
        )
    }

    /// Names the nearest colour tokens (by RGB distance for hex and rgb(),
    /// all of them for other notations).
    fn colour_message(&self, lit: &str) -> String {
        let target = rgb_of(lit);
        let names = match target {
            Some(t) => {
                let mut ranked: Vec<(f64, &str)> = self
                    .colours
                    .iter()
                    .filter_map(|(n, c)| c.map(|c| (dist(t, c), n.as_str())))
                    .collect();
                ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(b.1)));
                ranked.iter().take(3).map(|(_, n)| *n).collect::<Vec<_>>()
            }
            None => {
                let mut all: Vec<&str> = self.colours.iter().map(|(n, _)| n.as_str()).collect();
                all.sort_unstable();
                all
            }
        };
        let Some(best) = names.first() else {
            return format!("colour `{lit}` is a literal; the theme has no colour tokens to use");
        };
        format!(
            "colour `{lit}` is a literal; use theme.color.{best} (nearest: {})",
            names.join(", ")
        )
    }

    fn spacing_name(&self, v: f64) -> Option<String> {
        self.spacing
            .iter()
            .find(|(_, _, t)| (t - v).abs() < 1e-9)
            .map(|(name, group, _)| format!("{group}.{name}"))
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
}

/// sRGB channels of `#rgb`, `#rrggbb[aa]` and `rgb(r, g, b[, a])`.
fn rgb_of(text: &str) -> Option<[f64; 3]> {
    let t = text.trim();
    if let Some(hex) = t.strip_prefix('#') {
        if !is_hex(hex) {
            return None;
        }
        let digits: Vec<u8> = if hex.len() <= 4 {
            hex.bytes().map(|b| hex_digit(b) * 17).collect()
        } else {
            hex.as_bytes()
                .chunks(2)
                .take(3)
                .map(|c| hex_digit(c[0]) * 16 + hex_digit(c[1]))
                .collect()
        };
        return Some([
            f64::from(digits[0]),
            f64::from(digits[1]),
            f64::from(digits[2]),
        ]);
    }
    let lower = t.to_ascii_lowercase();
    let inner = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))?;
    let parts: Vec<f64> = inner
        .trim_end_matches(')')
        .split(',')
        .take(3)
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
}

fn hex_digit(b: u8) -> u8 {
    (b as char).to_digit(16).unwrap_or(0) as u8
}

#[cfg(test)]
mod tests;
