//! Token use: a component's source names theme tokens, never literals.
//! Three rules, each an error at the literal's line and column:
//!
//! - a colour literal (`#rgb`, `#rrggbb[aa]`, `rgb(`, `hsl(`, `oklch(` and
//!   friends, or a CSS colour name as a whole string) outside the argument
//!   of a `token(...)` default;
//! - a font family string (a bundled family's name, or any string after
//!   `fontFamily:`);
//! - a number in a `style` value equal to a theme `space` or `radius`
//!   entry. Other numbers stay allowed.
//!
//! The rules read the source through `tsx::scan`, so a literal inside a
//! template literal's `${...}` is not checked.

use super::tsx::{Kind, Tok, line_text, scan};
use crate::error::{StudioError, TsxLoc};
use crate::theme::Theme;
use serde_json::Value;
use std::collections::HashSet;

/// Every error the token rules find in `src`, in source order.
pub fn check(component: &str, src: &str, theme: &Theme) -> Vec<StudioError> {
    let palette = Palette::of(theme);
    let toks = scan(src);
    let exempt = exempt_flags(&toks);
    let in_style = style_flags(&toks);
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
                    for (at, lit) in colour_literals(&tok.text) {
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
            Kind::Number(v) if in_style[i] && !(i > 0 && toks[i - 1].kind == Kind::Punct('-')) => {
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

/// Which tokens are a direct value of a `style` object: the object after
/// `style=` or `style:`, and the arrays in it (`padding: [0, 16]`). A
/// nested object (`shadow: { y: 40 }`, `border`, `fill`) is not a style
/// value: its numbers are offsets, widths and stops.
fn style_flags(toks: &[Tok]) -> Vec<bool> {
    #[derive(Clone, Copy, PartialEq)]
    enum Brace {
        /// The JSX expression holding a style object: `style={{ ... }}`.
        Container,
        /// A style object, or one of its nested objects.
        Style,
        Other,
    }
    let mut braces: Vec<Brace> = Vec::new();
    let mut out = vec![false; toks.len()];
    for (i, tok) in toks.iter().enumerate() {
        match tok.kind {
            Kind::Punct('{') => {
                let after_style = |sep: char| {
                    i >= 2
                        && toks[i - 1].kind == Kind::Punct(sep)
                        && toks[i - 2].kind == Kind::Ident
                        && toks[i - 2].text == "style"
                };
                // `style={{` opens a container, then its object; `style: {`
                // is an object.
                let in_container = i >= 1
                    && toks[i - 1].kind == Kind::Punct('{')
                    && braces.last() == Some(&Brace::Container);
                let kind = if after_style('=') {
                    Brace::Container
                } else if after_style(':') || in_container {
                    Brace::Style
                } else {
                    Brace::Other
                };
                braces.push(kind);
                out[i] = kind == Brace::Style;
            }
            Kind::Punct('}') => {
                out[i] = braces.last() == Some(&Brace::Style);
                braces.pop();
            }
            _ => out[i] = braces.last() == Some(&Brace::Style),
        }
    }
    out
}

/// The colour literals in one string, each with its byte offset in it: hex,
/// functional notation, or the whole string being a CSS colour name.
fn colour_literals(s: &str) -> Vec<(usize, String)> {
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
    if CSS_COLOUR_NAMES.contains(&whole.as_str()) {
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
            families: crate::bundled_families(),
        }
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
        format!(
            "font family `{s}` is a literal; use theme.font.body or theme.font.heading (no system fonts)"
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

/// The CSS named colours, lowercase. A string literal equal to one of these
/// is a colour literal.
const CSS_COLOUR_NAMES: &[&str] = &[
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "cadetblue",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "green",
    "greenyellow",
    "grey",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "magenta",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "turquoise",
    "violet",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

#[cfg(test)]
mod tests;
