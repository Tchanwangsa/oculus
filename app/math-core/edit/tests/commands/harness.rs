//! The marker notation the behaviour tables are written in, and the key
//! names they use.
//!
//! A formula is written with its selection marked: `|` is a caret, `¦` a
//! caret at the same offset as another stop but in the slot that starts
//! there (`Affinity::After`: `x^2¦` is after the script, `x^2|` inside
//! it), and `‹`/`›` the anchor and head of a selection. None of them is
//! LaTeX, though `|` is: formulas in the tables write `\vert` instead.
//! The result adds `  ⟨\name⟩` for a pending command and `  => Effect`
//! for an effect.

use core::fmt::Write as _;

use oculus_math_edit::{Affinity, Command, Field, Outcome, Selection, StopId};

const MARKS: [char; 4] = ['|', '¦', '‹', '›'];

/// The field `marked` describes.
pub fn field(marked: &str, display: bool) -> Field {
    let mut source = String::new();
    let mut marks = Vec::new();
    for c in marked.chars() {
        if MARKS.contains(&c) {
            marks.push((c, source.len()));
        } else {
            source.push(c);
        }
    }
    let field = Field::new(&source, display).unwrap_or_else(|e| panic!("{marked:?}: {e}"));
    let stop = |c: char| {
        marks.iter().find(|(m, _)| *m == c).map(|&(m, offset)| {
            let affinity = if m == '¦' {
                Affinity::After
            } else {
                Affinity::Before
            };
            field.stops().stop_at(offset, affinity)
        })
    };
    let selection = match (stop('|').or_else(|| stop('¦')), stop('‹'), stop('›')) {
        (Some(caret), None, None) => Selection::caret(caret),
        (None, Some(anchor), Some(head)) => Selection { anchor, head },
        _ => panic!("{marked:?}: mark a caret, or an anchor and a head"),
    };
    field.select(selection)
}

/// `field` in the notation.
pub fn marked(field: &Field) -> String {
    let stops = field.stops();
    let selection = field.selection();
    let mark = |id: StopId, plain: char| {
        let offset = stops.offset(id);
        if plain == '|' && stops.stop_at(offset, Affinity::Before) != id {
            '¦'
        } else {
            plain
        }
    };
    let mut marks: Vec<(usize, char)> = if selection.is_caret() {
        vec![(stops.offset(selection.head), mark(selection.head, '|'))]
    } else {
        vec![
            (stops.offset(selection.anchor), '‹'),
            (stops.offset(selection.head), '›'),
        ]
    };
    marks.sort_by_key(|&(offset, c)| (offset, c != '‹'));
    let source = field.source();
    let mut out = String::new();
    let mut at = 0;
    for (offset, c) in marks {
        out.push_str(&source[at..offset]);
        out.push(c);
        at = offset;
    }
    out.push_str(&source[at..]);
    if let Some(name) = field.pending() {
        let _ = write!(out, "  ⟨\\{name}⟩");
    }
    out
}

/// The command a key name stands for: `t:abc` types each character as
/// a key of its own, `ime:abc` inserts it as one string, `tpl:` and
/// `paste:` insert a template or LaTeX; the rest name keys (`s-` is
/// Shift, `cmd-bs` ⌘Backspace). ↑/↓ measure each stop's x as its index
/// in its slot, as if every slot were left-aligned.
pub fn commands(field: &Field, key: &str) -> Vec<Command> {
    if let Some(text) = key.strip_prefix("t:") {
        return text
            .chars()
            .map(|c| Command::Insert(c.to_string()))
            .collect();
    }
    let xs: Vec<f64> = field
        .stops()
        .stops()
        .iter()
        .map(|s| s.index as f64)
        .collect();
    vec![match key {
        _ if key.starts_with("ime:") => Command::Insert(key[4..].to_owned()),
        _ if key.starts_with("tpl:") => Command::Template(key[4..].to_owned()),
        _ if key.starts_with("paste:") => Command::Paste(key[6..].to_owned()),
        "space" => Command::Insert(" ".to_owned()),
        "bs" => Command::Backspace,
        "del" => Command::Delete,
        "cmd-bs" => Command::DeleteLine,
        "left" => Command::Left { extend: false },
        "right" => Command::Right { extend: false },
        "s-left" => Command::Left { extend: true },
        "s-right" => Command::Right { extend: true },
        "home" => Command::Home { extend: false },
        "end" => Command::End { extend: false },
        "s-home" => Command::Home { extend: true },
        "s-end" => Command::End { extend: true },
        "up" => Command::Up { xs },
        "down" => Command::Down { xs },
        "all" => Command::SelectAll,
        "tab" => Command::Tab,
        "s-tab" => Command::ShiftTab,
        "enter" => Command::Enter,
        "esc" => Command::Escape,
        _ => panic!("unknown key {key:?}"),
    }]
}

/// The keys pressed in turn on `marked`, the result in the notation. An
/// effect ends the run.
pub fn press(marked_in: &str, display: bool, keys: &[&str]) -> String {
    let mut current = field(marked_in, display);
    for key in keys {
        for command in commands(&current, key) {
            let outcome: Outcome = current.run(&command);
            check_change(&current, &outcome);
            if let Some(effect) = outcome.effect {
                return format!("{}  => {effect:?}", marked(&outcome.field));
            }
            current = outcome.field;
        }
    }
    marked(&current)
}

/// The outcome's change turns the old source into the new one.
fn check_change(before: &Field, outcome: &Outcome) {
    let mut source = before.source().to_owned();
    for change in outcome.changes.iter().rev() {
        source.replace_range(change.from..change.to, &change.insert);
    }
    assert_eq!(
        source,
        outcome.field.source(),
        "change from {:?}",
        before.source()
    );
}

/// Each case: `(marked input, keys, marked output)`.
pub type Case<'a> = (&'a str, &'a [&'a str], &'a str);

pub fn check_all(cases: &[Case], display: bool) {
    let mut failed = Vec::new();
    for (input, keys, expected) in cases {
        let got = press(input, display, keys);
        if got != *expected {
            failed.push(format!(
                "{input:?} {keys:?}\n   expected {expected:?}\n        got {got:?}"
            ));
        }
    }
    assert!(
        failed.is_empty(),
        "{} failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
}
