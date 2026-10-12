//! Where a colour goes in a component's source. A CSS colour name is a
//! colour literal only there: anywhere else ("purple", "gold") it is a word
//! shown on screen. A string's place is the key it is the value of, read
//! from the scanner's tokens: `key: value` in an object, `key=value` in JSX
//! or an assignment, carried into the arrays, calls and `{...}` expressions
//! inside that value (`stops: [[0, "red"]]`, `fill={on ? "red" : x}`) and
//! ended by the next `,` or `;` of an object or block.

use super::super::tsx::{Kind, Tok};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Place {
    /// Not where a colour goes.
    Other,
    /// A colour key's value: `color`, `fill`, `stroke`, `background`,
    /// `stops`, or any key ending in `Color` (`borderColor`).
    Colour,
    /// A `token("color")` prop's value, where a theme colour's name is the
    /// token and not a literal.
    ColourProp,
}

const COLOUR_KEYS: [&str; 5] = ["color", "fill", "stroke", "background", "stops"];

fn place_of(key: &str, colour_props: &[String]) -> Place {
    if COLOUR_KEYS.contains(&key) || key.ends_with("Color") {
        Place::Colour
    } else if colour_props.iter().any(|p| p == key) {
        Place::ColourProp
    } else {
        Place::Other
    }
}

/// One open bracket's state: the key whose value is being read, whether a
/// comma ends it (an object or block, not a list or call), and whether it
/// came from a JSX attribute (`fill="..."`), which a `>` also ends.
struct Frame {
    key: Option<Place>,
    object: bool,
    attr: bool,
}

/// Each token's place; only strings' places are read. `colour_props` are
/// the names of the component's `token("color")` props.
pub(super) fn places(toks: &[Tok], colour_props: &[String]) -> Vec<Place> {
    let mut stack = vec![Frame {
        key: None,
        object: true,
        attr: false,
    }];
    let mut out = vec![Place::Other; toks.len()];
    let punct = |j: Option<usize>| j.and_then(|j| toks.get(j)).map(|t| &t.kind);
    for (i, tok) in toks.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| &toks[j]);
        let inherited = stack.last().and_then(|f| f.key);
        out[i] = inherited.unwrap_or(Place::Other);
        let Kind::Punct(c) = tok.kind else {
            continue;
        };
        let Some(top) = stack.last_mut() else {
            continue;
        };
        match c {
            ':' => {
                // An object key: a name or string right after `{` or `,`.
                let named = prev.is_some_and(|p| matches!(p.kind, Kind::Ident | Kind::Str));
                let opens = matches!(punct(i.checked_sub(2)), None | Some(Kind::Punct('{' | ',')));
                if let (true, true, Some(p)) = (named, opens, prev) {
                    top.key = Some(place_of(&p.text, colour_props));
                    top.attr = false;
                }
            }
            '=' => {
                // `key=` in JSX or an assignment, not `==`, `=>`, `<=`.
                let named = prev.is_some_and(|p| p.kind == Kind::Ident);
                let next = punct(Some(i + 1));
                if let (true, Some(p)) = (named, prev)
                    && !matches!(next, Some(Kind::Punct('=' | '>')))
                {
                    top.key = Some(place_of(&p.text, colour_props));
                    top.attr = true;
                }
            }
            ',' if top.object => top.key = None,
            ';' => top.key = None,
            '>' if top.attr => {
                top.key = None;
                top.attr = false;
            }
            '{' | '[' | '(' => {
                // A `{` opens a fresh object or block, except the JSX
                // expression of an attribute (`fill={...}`), which is its
                // value.
                let carries = c != '{' || prev.is_some_and(|p| p.kind == Kind::Punct('='));
                stack.push(Frame {
                    key: if carries { inherited } else { None },
                    object: c == '{',
                    attr: false,
                });
            }
            '}' | ']' | ')' if stack.len() > 1 => {
                stack.pop();
            }
            _ => {}
        }
    }
    out
}
