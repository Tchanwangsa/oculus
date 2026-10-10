//! Typing in maths: characters, and the keys that build structure (`^`,
//! `_`, `/`, `{`, `}`).

use core::ops::Range;

use katex::{parser::parse_node::AnyParseNode, symbols::Atom, types::ErrorLocationProvider as _};

use super::{
    Command, compose,
    template::{place, selection_slot},
    text,
};
use crate::{
    field::{Field, Outcome, Selection},
    parse::parse,
    slot::{Bounds, SlotId, SlotKind},
};

/// Typed text. In a text run the whole string goes in at once (an IME's
/// syllable stays whole); in maths each character is a key of its own.
pub fn insert(field: &Field, typed: &str) -> Outcome {
    if field.in_text() {
        return text::insert(field, typed);
    }
    let mut chars = typed.chars();
    let Some(first) = chars.next() else {
        return Outcome::none(field);
    };
    if chars.next().is_none() {
        return key(field, first);
    }
    let mut outcome = Outcome::none(field);
    for c in typed.chars() {
        let next = outcome.field.run(&Command::Insert(c.to_string()));
        outcome = Outcome {
            effect: next.effect,
            ..compose(field, next)
        };
    }
    outcome
}

/// One character typed in maths.
fn key(field: &Field, c: char) -> Outcome {
    match c {
        '\\' => Outcome::moved(field.clone().with_pending(Some(String::new()))),
        '^' => script(field, SlotKind::Sup),
        '_' => script(field, SlotKind::Sub),
        '/' => fraction(field),
        '{' => super::template::template(field, "{#0}"),
        '}' => close_group(field),
        // A typed `~` means "similar to"; bare, TeX draws it as a space.
        '~' => symbol(field, r"\sim"),
        '#' => symbol(field, r"\#"),
        '%' => symbol(field, r"\%"),
        '$' => symbol(field, r"\$"),
        // In an array cell `&` would start a new cell: 3c's (matrix keys).
        '&' => symbol(field, r"\&"),
        // Space is the view's (quick picks); maths ignores it anyway.
        c if c.is_whitespace() || c.is_control() => Outcome::none(field),
        c => symbol(field, c.encode_utf8(&mut [0; 4])),
    }
}

/// `latex` in place of the selection, the caret after it.
fn symbol(field: &Field, latex: &str) -> Outcome {
    let (slot, range) = selection_slot(field);
    place(field, slot, range, latex, "")
}

/// `^` or `_`. After an atom that has that script already (or right
/// before its scripts, at `x|^2`), the caret moves into it; otherwise an
/// empty script goes in at the caret (a base-less one at a slot's start).
/// A selection becomes the base: `{sel}^{}`, braces left off a single
/// atom.
fn script(field: &Field, kind: SlotKind) -> Outcome {
    let op = if kind == SlotKind::Sup { "^" } else { "_" };
    let stops = field.stops();
    let selection = field.selection();
    if !selection.is_caret() {
        let (slot, range) = selection_slot(field);
        let one_atom = stops
            .slot(slot)
            .atoms
            .iter()
            .filter(|atom| range.start <= atom.start && atom.end <= range.end)
            .count()
            == 1;
        let after_scripts = stops
            .atom_after(slot, range.end)
            .is_some_and(|atom| has_scripts(field, slot, atom));
        let template = if one_atom && !after_scripts {
            format!("#0{op}{{#?}}")
        } else {
            format!("{{#0}}{op}{{#?}}")
        };
        let fill = field.source()[range.clone()].to_owned();
        return place(field, slot, range, &template, &fill);
    }
    let head = selection.head;
    let slot = stops.stop(head).slot;
    let offset = stops.offset(head);
    let before = stops.atom_before(slot, offset);
    let after = stops
        .atom_after(slot, offset)
        .filter(|&atom| stops.slot(slot).atoms[atom].start == offset);
    for atom in [before, after].into_iter().flatten() {
        if !has_scripts(field, slot, atom) {
            continue;
        }
        if let Some(script) = stops
            .atom_slots(slot, atom)
            .into_iter()
            .find(|&s| stops.slot(s).kind == kind)
        {
            let caret = stops.last_stop(script);
            return Outcome::moved(field.clone().select(Selection::caret(caret)));
        }
    }
    place(field, slot, offset..offset, &format!("{op}{{#?}}"), "")
}

/// Whether atom `atom` of `slot` is a base's scripts.
fn has_scripts(field: &Field, slot: SlotId, atom: usize) -> bool {
    let stops = field.stops();
    stops
        .atom_slots(slot, atom)
        .iter()
        .any(|&s| matches!(stops.slot(s).kind, SlotKind::Sup | SlotKind::Sub))
}

/// `/`: a fraction whose numerator is the selection, or else the term
/// before the caret: the atom before it with its scripts and primes, a
/// whole number, or a bracketed run back to its opening bracket. After an
/// operator, a relation, an opening bracket or punctuation (or at a
/// slot's start) the numerator is empty and takes the caret; otherwise
/// the caret goes to the denominator.
fn fraction(field: &Field) -> Outcome {
    let template = r"\frac{#0}{#?}";
    if !field.selection().is_caret() {
        let (slot, range) = selection_slot(field);
        let fill = field.source()[range.clone()].to_owned();
        return place(field, slot, range, template, &fill);
    }
    let stops = field.stops();
    let head = field.selection().head;
    let slot = stops.stop(head).slot;
    let offset = stops.offset(head);
    let start = numerator_start(field, slot, offset).unwrap_or(offset);
    let fill = field.source()[start..offset].to_owned();
    place(field, slot, start..offset, template, &fill)
}

/// Where the term before `offset` in `slot` starts; `None` when there is
/// none to take.
fn numerator_start(field: &Field, slot: SlotId, offset: usize) -> Option<usize> {
    let src = field.source();
    let stops = field.stops();
    let atoms = &stops.slot(slot).atoms;
    let text = |k: usize| src[atoms[k].clone()].trim();
    let adjacent = |k: usize| src[atoms[k - 1].end..atoms[k].start].trim().is_empty();
    let mut k = stops.atom_before(slot, offset)?;
    // Scripts and primes go with their base.
    while k > 0 && text(k).starts_with(['^', '_', '\'']) && adjacent(k) {
        k -= 1;
    }
    let digits =
        |k: usize| !text(k).is_empty() && text(k).chars().all(|c| c.is_ascii_digit() || c == '.');
    if digits(k) {
        while k > 0 && digits(k - 1) && adjacent(k) {
            k -= 1;
        }
    } else if let Some(open) = match text(k) {
        ")" => Some(("(", ")")),
        "]" => Some(("[", "]")),
        _ => None,
    } {
        let mut depth = 0usize;
        for j in (0..=k).rev() {
            if text(j) == open.1 {
                depth += 1;
            } else if text(j) == open.0 {
                depth -= 1;
                if depth == 0 {
                    k = j;
                    break;
                }
            }
        }
    } else if matches!(
        family(field, atoms[k].clone()),
        Some(Atom::Bin | Atom::Rel | Atom::Open | Atom::Punct)
    ) {
        return None;
    }
    Some(atoms[k].start)
}

/// The class of the symbol at exactly `range`, when it is a plain symbol
/// (`+`, `=`, `(`, `\le`).
fn family(field: &Field, range: Range<usize>) -> Option<Atom> {
    let nodes = parse(field.source(), field.display()).ok()?;
    let mut stack: Vec<&AnyParseNode> = nodes.iter().collect();
    while let Some(node) = stack.pop() {
        let at = node.loc().map(|loc| loc.start..loc.end);
        if let (AnyParseNode::Atom(atom), Some(at)) = (node, &at)
            && *at == range
        {
            return Some(atom.family);
        }
        if at.is_none_or(|at| at.start <= range.start && range.end <= at.end) {
            stack.extend(node.children());
        }
    }
    None
}

/// `}` at the end of a braced slot steps out of it, as → would;
/// elsewhere it does nothing (braces only come in pairs).
fn close_group(field: &Field) -> Outcome {
    let stops = field.stops();
    let head = field.selection().head;
    let slot = stops.slot(stops.stop(head).slot);
    let closes = field.selection().is_caret()
        && slot.bounds == Bounds::Delimited
        && stops.at_slot_end(head)
        && field.source()[slot.interior.end..].starts_with('}');
    match stops.next(head) {
        Some(next) if closes => Outcome::moved(field.clone().select(Selection::caret(next))),
        _ => Outcome::none(field),
    }
}
