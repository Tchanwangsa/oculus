//! Literals that bypass the theme or garble maths.
//!
//! - `colour`: a string or template text holding `#rgb`, `#rgba`,
//!   `#rrggbb` or `#rrggbbaa` (not after `&`, a letter or a digit) or a
//!   CSS colour function (`rgb(`, `rgba(`, `hsl(`, `hsla(`, `hwb(`, `lab(`,
//!   `lch(`, `oklab(`, `oklch(`); a number written `0x` with six hex digits
//!   (three.js's colour form).
//! - `font`: an object key or JSX attribute named `fontFamily` or
//!   `font-family`, or `font-family:` inside a string or template text.
//! - `tex`: a JSX attribute string holding `\\`, which JSX keeps as two
//!   backslashes (`tex="\\theta"` is TeX `\\theta`).

use super::Finding;
use oxc::ast::ast::{
    JSXAttribute, JSXAttributeName, JSXAttributeValue, NumericLiteral, ObjectProperty, Program,
    StringLiteral, TemplateElement,
};
use oxc::ast_visit::{walk, Visit};

const COLOUR_FUNCTIONS: &[&str] = &[
    "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch",
];
const FONT_KEYS: &[&str] = &["fontFamily", "font-family"];

pub(super) fn check(program: &Program, src: &str) -> Vec<Finding> {
    let mut rule = Literals {
        src,
        out: Vec::new(),
    };
    rule.visit_program(program);
    rule.out
}

struct Literals<'s> {
    src: &'s str,
    out: Vec<Finding>,
}

impl Literals<'_> {
    fn text(&mut self, text: &str, offset: u32) {
        if let Some(colour) = colour_in(text) {
            self.push(offset, "colour", colour_message(&colour));
        }
        if declares_font(text) {
            self.push(offset, "font", FONT_MESSAGE.into());
        }
    }

    fn push(&mut self, offset: u32, rule: &'static str, message: String) {
        self.out.push(Finding::at(self.src, offset, rule, message));
    }
}

const FONT_MESSAGE: &str =
    "a font family of its own: the frame already uses the app's fonts, so leave the family unset";

fn colour_message(colour: &str) -> String {
    format!(
        "`{colour}` is a fixed colour, wrong in one of the themes: name a theme token \
         (`color=\"brand\"` on a kit part, `var(--color-brand)` in a style)"
    )
}

impl<'a> Visit<'a> for Literals<'_> {
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        self.text(it.value.as_str(), it.span.start);
    }

    fn visit_template_element(&mut self, it: &TemplateElement<'a>) {
        let text = it.value.cooked.as_ref().unwrap_or(&it.value.raw);
        self.text(text.as_str(), it.span.start);
    }

    fn visit_numeric_literal(&mut self, it: &NumericLiteral<'a>) {
        let raw = &self.src[it.span.start as usize..it.span.end as usize];
        let hex = raw
            .strip_prefix("0x")
            .or_else(|| raw.strip_prefix("0X"))
            .unwrap_or("");
        if hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            self.push(it.span.start, "colour", colour_message(raw));
        }
    }

    fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
        if !it.computed
            && it
                .key
                .static_name()
                .is_some_and(|k| FONT_KEYS.contains(&&*k))
        {
            self.push(it.span.start, "font", FONT_MESSAGE.into());
        }
        walk::walk_object_property(self, it);
    }

    fn visit_jsx_attribute(&mut self, it: &JSXAttribute<'a>) {
        if let JSXAttributeName::Identifier(name) = &it.name {
            if FONT_KEYS.contains(&name.name.as_str()) {
                self.push(it.span.start, "font", FONT_MESSAGE.into());
            }
        }
        if let Some(JSXAttributeValue::StringLiteral(s)) = &it.value {
            let raw = &self.src[s.span.start as usize..s.span.end as usize];
            if raw.contains("\\\\") {
                let name = match &it.name {
                    JSXAttributeName::Identifier(n) => n.name.as_str(),
                    JSXAttributeName::NamespacedName(n) => n.name.name.as_str(),
                };
                self.push(
                    s.span.start,
                    "tex",
                    format!(
                        "a JSX attribute string keeps `\\\\` as two backslashes: write \
                         {name}={{\"\\\\theta\"}} (a JavaScript string), not {name}=\"\\\\theta\""
                    ),
                );
            }
        }
        walk::walk_jsx_attribute(self, it);
    }
}

/// The first colour literal in `text`, as written.
fn colour_in(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'#' {
            continue;
        }
        let before = i.checked_sub(1).map(|j| bytes[j]);
        if before.is_some_and(|c| c == b'&' || c.is_ascii_alphanumeric()) {
            continue;
        }
        let digits = bytes[i + 1..]
            .iter()
            .take_while(|c| c.is_ascii_hexdigit())
            .count();
        let after = bytes.get(i + 1 + digits);
        if matches!(digits, 3 | 4 | 6 | 8)
            && !after.is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            return Some(text[i..i + 1 + digits].to_string());
        }
    }
    let lower = text.to_ascii_lowercase();
    for name in COLOUR_FUNCTIONS {
        let call = format!("{name}(");
        for (i, _) in lower.match_indices(&call) {
            let before = lower[..i].bytes().next_back();
            if !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
                return Some(call);
            }
        }
    }
    None
}

/// `font-family` followed by a colon, as CSS text writes it.
fn declares_font(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower
        .match_indices("font-family")
        .any(|(i, m)| lower[i + m.len()..].trim_start().starts_with(':'))
}

#[cfg(test)]
mod tests {
    use super::{colour_in, declares_font};

    #[test]
    fn colour_literals_are_found_and_lookalikes_are_not() {
        for (text, found) in [
            ("#fff", Some("#fff")),
            ("1px solid #ff000080", Some("#ff000080")),
            ("color: RGB(0, 0, 0)", Some("rgb(")),
            ("oklch(0.7 0.1 200)", Some("oklch(")),
            ("hsla(1,2%,3%,.5)", Some("hsla(")),
            ("&#123;", None),
            ("#12", None),
            ("#12345", None),
            ("step #1234567", None),
            ("a#fff", None),
            ("var(--color-brand)", None),
            ("--my-rgb(1)", None),
            ("Collab(1)", None),
        ] {
            assert_eq!(colour_in(text).as_deref(), found, "{text}");
        }
    }

    #[test]
    fn font_family_counts_only_as_css() {
        assert!(declares_font("p { font-family : serif }"));
        assert!(!declares_font("the font-family is set by the app"));
    }
}
