//! Which numbers in a `style` object are checked against the theme's
//! `space` and `radius` entries: a literal that is itself a length, the
//! value of a length key or a term added to or subtracted from one
//! (`space.s + 4`). A unitless key (`opacity`, `scale`, `rotate`,
//! `flexGrow`, `fontWeight`, `lineHeight`, `transformOrigin`, ...) is never
//! checked, and neither is a factor (`pop * 4`, `24 * s`), a side of a
//! comparison, an argument of a call (`clamp(x, 0, 1)`), a negative number,
//! or a number inside a nested object (`shadow`, `border`, `fill`).

use super::super::tsx::{Kind, Tok};

/// The style keys whose value is a length in pixels.
const LENGTH_KEYS: [&str; 19] = [
    "width",
    "height",
    "minWidth",
    "minHeight",
    "maxWidth",
    "maxHeight",
    "gap",
    "padding",
    "margin",
    "left",
    "top",
    "right",
    "bottom",
    "radius",
    "blur",
    "translateX",
    "translateY",
    "fontSize",
    "letterSpacing",
];

#[derive(Clone, PartialEq)]
enum Group {
    /// The JSX expression holding a style object: `style={{ ... }}`.
    Container,
    /// A style object, with the key whose value is being read.
    Style { length: bool },
    /// Any other `{`: a nested object, a block, a JSX expression.
    Object,
    /// An array, or parentheses that only group: the value passes through.
    Through,
    /// A call's arguments.
    Call,
}

/// Per token, whether it is a number to check: inside a style object, as
/// the value of a length key, either the whole value, a branch of it (an
/// array element, a ternary's arm, the side of `||`, `??` or `&&`) or a
/// term of a sum or difference.
pub(super) fn style_numbers(toks: &[Tok]) -> Vec<bool> {
    let mut stack: Vec<Group> = Vec::new();
    let mut out = vec![false; toks.len()];
    let kind = |j: Option<usize>| j.and_then(|j| toks.get(j)).map(|t| &t.kind);
    for (i, tok) in toks.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| &toks[j]);
        let after_style = |sep: char| {
            i >= 2
                && toks[i - 1].kind == Kind::Punct(sep)
                && toks[i - 2].kind == Kind::Ident
                && toks[i - 2].text == "style"
        };
        match tok.kind {
            Kind::Punct('{') => {
                // `style={{` opens a container, then its object; `style: {`
                // is an object.
                let in_container = prev.is_some_and(|p| p.kind == Kind::Punct('{'))
                    && stack.last() == Some(&Group::Container);
                stack.push(if after_style('=') {
                    Group::Container
                } else if after_style(':') || in_container {
                    Group::Style { length: false }
                } else {
                    Group::Object
                });
            }
            Kind::Punct('}') => {
                while matches!(stack.last(), Some(Group::Through | Group::Call)) {
                    stack.pop();
                }
                stack.pop();
            }
            Kind::Punct('[') => stack.push(Group::Through),
            Kind::Punct('(') => {
                let call =
                    prev.is_some_and(|p| matches!(p.kind, Kind::Ident | Kind::Punct(')' | ']')));
                stack.push(if call { Group::Call } else { Group::Through });
            }
            Kind::Punct(']' | ')') => {
                if matches!(stack.last(), Some(Group::Through | Group::Call)) {
                    stack.pop();
                }
            }
            Kind::Punct(':') => {
                // A key: a name or string right after the style's `{` or `,`.
                let named = prev.is_some_and(|p| matches!(p.kind, Kind::Ident | Kind::Str));
                let opens = matches!(kind(i.checked_sub(2)), Some(Kind::Punct('{' | ',')));
                if let (true, true, Some(p), Some(Group::Style { length })) =
                    (named, opens, prev, stack.last_mut())
                {
                    *length = LENGTH_KEYS.contains(&p.text.as_str());
                }
            }
            Kind::Punct(',') => {
                if let Some(Group::Style { length }) = stack.last_mut() {
                    *length = false;
                }
            }
            Kind::Number(_) => {
                let length = stack
                    .iter()
                    .rev()
                    .find(|g| **g != Group::Through)
                    .is_some_and(|g| *g == (Group::Style { length: true }));
                // A `-` after a value subtracts; anywhere else it negates.
                let subtracts = i >= 2
                    && matches!(
                        toks[i - 2].kind,
                        Kind::Ident | Kind::Number(_) | Kind::Str | Kind::Punct(')' | ']')
                    );
                let before = match prev.map(|p| &p.kind) {
                    Some(Kind::Punct(':' | ',' | '[' | '(' | '?' | '|' | '&' | '+')) => true,
                    Some(Kind::Punct('-')) => subtracts,
                    _ => false,
                };
                let after = matches!(
                    kind(Some(i + 1)),
                    Some(Kind::Punct(',' | ']' | '}' | ')' | ':' | '+' | '-'))
                );
                out[i] = length && before && after;
            }
            _ => {}
        }
    }
    out
}
