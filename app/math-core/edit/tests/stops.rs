//! Hand-written cases: where the caret stops in each kind of construct,
//! and what the slots around the stops are.
#![allow(
    clippy::non_ascii_literal,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

use core::ops::Range;

use oculus_math_edit::{Affinity, Bounds, SlotId, SlotKind, SlotPath, StopId, Stops, stops};

fn parsed(source: &str, display: bool) -> Stops {
    stops(source, display).unwrap_or_else(|err| panic!("{source:?}: {err}"))
}

/// The source with a `|` at every stop, one per stop (stops that share
/// an offset show as `||`).
fn marked(source: &str, display: bool) -> String {
    let stops = parsed(source, display);
    let mut out = String::new();
    let mut at = 0;
    for stop in stops.stops() {
        out.push_str(&source[at..stop.offset]);
        out.push('|');
        at = stop.offset;
    }
    out.push_str(&source[at..]);
    out
}

/// `(source, source with its stops marked)`, inline maths.
const INLINE: &[(&str, &str)] = &[
    ("ab", "|a|b|"),
    ("12+x", "|1|2|+|x|"),
    (r"\alpha x", r"|\alpha |x|"),
    (r"\sin x", r"|\sin |x|"),
    (r"a\,b", r"|a|\,|b|"),
    (r"\frac{a}{b}", r"|\frac{|a|}{|b|}|"),
    (r"\frac ab", r"|\frac |a||b||"),
    (r"\frac{}{}", r"|\frac{|}{|}|"),
    (r"\dfrac{a}{b}", r"|\dfrac{|a|}{|b|}|"),
    ("x^2", "|x|^|2||"),
    ("x^{2}", "|x|^{|2|}|"),
    ("x^{}", "|x|^{|}|"),
    ("x_1^2", "|x|_|1|^|2||"),
    ("x^2_1", "|x|^|2|_|1||"),
    ("x''^2", "|x|'|'|^|2||"),
    ("x'", "|x|'|"),
    ("x²", "|x|²|"),
    ("^2", "|^|2||"),
    (r"\sum_{i=1}^n i", r"|\sum|_{|i|=|1|}^|n|| i|"),
    (r"\sqrt[3]{x}", r"|\sqrt[|3|]{|x|}|"),
    (r"\sqrt{}", r"|\sqrt{|}|"),
    (r"\sqrt x", r"|\sqrt |x||"),
    (r"\sqrt\frac ab", r"|\sqrt|\frac |a||b|||"),
    (r"\hat{x}", r"|\hat{|x|}|"),
    (r"\hat x", r"|\hat |x||"),
    (r"\overline{ab}", r"|\overline{|a|b|}|"),
    (r"\overline{}", r"|\overline{|}|"),
    (r"\operatorname{sin}", r"|\operatorname{|s|i|n|}|"),
    (r"\operatorname*{lim}_x", r"|\operatorname*{|l|i|m|}|_|x||"),
    (r"\mathbf{x}", r"|\mathbf{|x|}|"),
    (r"\mathbf x", r"|\mathbf |x||"),
    (r"\boldsymbol{x}", r"|\boldsymbol{|x|}|"),
    (r"\left(a+b\right)", r"|\left(|a|+|b|\right)|"),
    (r"\left.\right.", r"|\left.|\right.|"),
    (
        r"\left\langle x\right\rangle",
        r"|\left\langle |x|\right\rangle|",
    ),
    (r"\text{a b}", r"|\text{|a| |b|}|"),
    (r"\text{ไทย}", r"|\text{|ไ|ท|ย|}|"),
    // A combining vowel stays with its consonant.
    (r"\text{สวัสดี x}", r"|\text{|ส|วั|ส|ดี| |x|}|"),
    (r"\text{---}", r"|\text{|-|-|-|}|"),
    (r"\text{}", r"|\text{|}|"),
    (r"\text{a $x$ b}", r"|\text{|a| |$|x|$| |b|}|"),
    (r"\textbf{a}", r"|\textbf{|a|}|"),
    // Macro output is one atom.
    (r"\dots", r"|\dots|"),
    (r"a\iff b", r"|a|\iff |b|"),
    (r"\not=", r"|\not|=|"),
    (r"\argmax", r"|\argmax|"),
    (r"\def\x{ab}\x", r"|\def\x{ab}\x|"),
    // ... unless its argument is pasted into it.
    (r"\bra{x}", r"|\bra{|x|}|"),
    (r"\boxed{x}", r"|\boxed{|x|}|"),
    // Spacing groups are empty slots.
    ("{}^{14}C", "|{|}|^{|1|4|}|C|"),
    ("a={}b", "|a|=|{|}|b|"),
    (r"\color{red}{x}", r"|\color{red}|{|x|}|"),
    (r"\color{red} x y", r"|\color{red} |x| y|"),
    (r"\textcolor{red}{x}", r"|\textcolor{red}{|x|}|"),
    (r"\colorbox{red}{x}", r"|\colorbox{red}{|x|}|"),
    (r"\bf ab", r"|\bf |a|b|"),
    (r"\displaystyle x", r"|\displaystyle |x|"),
    (r"a\over b", r"||a|\over |b||"),
    (r"{\over b}", r"|{||\over |b||}|"),
    (r"{n\choose k}", r"|{||n|\choose |k||}|"),
    (r"\overset{a}{b}", r"|\overset{|a|}{|b|}|"),
    (r"\underbrace{x}_{y}", r"|\underbrace{|x|}|_{|y|}|"),
    (r"\xrightarrow[b]{a}", r"|\xrightarrow[|b|]{|a|}|"),
    (
        r"\mathchoice{a}{b}{c}{d}",
        r"|\mathchoice{|a|}{|b|}{|c|}{|d|}|",
    ),
    (r"\begingroup a\endgroup", r"|\begingroup |a|\endgroup|"),
    (
        r"\begin{pmatrix}a&b\\c&d\end{pmatrix}",
        r"|\begin{pmatrix}|a|&|b|\\|c|&|d|\end{pmatrix}|",
    ),
    (
        r"\begin{matrix}a&&b\end{matrix}",
        r"|\begin{matrix}|a|&|&|b|\end{matrix}|",
    ),
    (
        r"\begin{matrix}\end{matrix}",
        r"|\begin{matrix}|\end{matrix}|",
    ),
    // KaTeX drops a trailing empty row; so do the stops.
    (
        r"\begin{pmatrix}a\\\end{pmatrix}",
        r"|\begin{pmatrix}|a|\\\end{pmatrix}|",
    ),
    // A cell that is one braced group (as parsed files write them).
    (
        r"\begin{array}{c}{a+b}\\{c}\end{array}",
        r"|\begin{array}{c}|{|a|+|b|}|\\|{|c|}|\end{array}|",
    ),
    // KaTeX unwraps a one-atom group argument; its braces stay a slot.
    (r"\hat{{}}", r"|\hat{|{|}|}|"),
    (r"\phantom{\sum\limits x}", r"|\phantom{|\sum\limits |x|}|"),
];

/// `(source, source with its stops marked)`, display maths.
const DISPLAY: &[(&str, &str)] = &[
    (
        r"\begin{aligned}a&=b\\&=c\end{aligned}",
        r"|\begin{aligned}|a|&|=|b|\\|&|=|c|\end{aligned}|",
    ),
    // A row starts after the spaces of the break before it.
    (r"a\\ b", r"|a|\\ |b|"),
    // `\over` takes the whole row, so this `\\` is inside the numerator,
    // where a line break is one atom.
    (r"a\\b\over c", r"||a|\\|b|\over |c||"),
    (r"\tag{1} x", r"|\tag{|1|}| x|"),
    (r"x\tag*{a}", r"|x|\tag*{|a|}|"),
    // A CD diagram's cells are arrow syntax too: edited as TeX.
    (
        r"\begin{CD}A @>f>> B\end{CD}",
        r"|\begin{CD}A @>f>> B\end{CD}|",
    ),
];

#[test]
fn stops_in_each_construct() {
    for (cases, display) in [(INLINE, false), (DISPLAY, true)] {
        for (source, expected) in cases {
            assert!(!source.contains('|'), "{source:?}: `|` marks stops");
            assert_eq!(marked(source, display), *expected, "{source:?}");
        }
    }
}

fn slot_of(stops: &Stops, kind: SlotKind) -> SlotId {
    let index = stops
        .slots()
        .iter()
        .position(|slot| slot.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} slot"));
    SlotId(index)
}

#[test]
fn bare_arguments_are_marked_bare() {
    let stops = parsed(r"\frac ab", false);
    for kind in [SlotKind::Numer, SlotKind::Denom] {
        assert_eq!(stops.slot(slot_of(&stops, kind)).bounds, Bounds::Bare);
    }
    let stops = parsed(r"\frac{a}{b}", false);
    let numer = stops.slot(slot_of(&stops, SlotKind::Numer));
    assert_eq!(
        (numer.bounds, numer.interior.clone()),
        (Bounds::Delimited, 6..7)
    );
    let stops = parsed("x^2", false);
    assert_eq!(
        stops.slot(slot_of(&stops, SlotKind::Sup)).bounds,
        Bounds::Bare
    );
    let stops = parsed(r"a\over b", false);
    assert_eq!(
        stops.slot(slot_of(&stops, SlotKind::Denom)).bounds,
        Bounds::Open
    );
    // KaTeX unwraps `{x}` to `x`; the braces are still there.
    let stops = parsed(r"\hat{x}", false);
    assert_eq!(
        stops.slot(slot_of(&stops, SlotKind::Body)).bounds,
        Bounds::Delimited
    );
}

#[test]
fn text_slots_are_text() {
    let stops = parsed(r"\text{a $x$ b}", false);
    assert!(stops.slot(slot_of(&stops, SlotKind::Text)).text);
    assert!(!stops.slot(slot_of(&stops, SlotKind::Math)).text);
    assert!(!stops.slot(slot_of(&stops, SlotKind::Row(0))).text);
    // A text-mode argument of a maths command.
    let stops = parsed(r"\colorbox{red}{x}", false);
    assert!(stops.slot(slot_of(&stops, SlotKind::Body)).text);
    let stops = parsed(r"\tag{1} x", true);
    assert!(stops.slot(slot_of(&stops, SlotKind::Tag)).text);
    // Thai typed in maths is a text-mode node, but its slot is maths.
    let stops = parsed("x_{ก}", false);
    assert!(!stops.slot(slot_of(&stops, SlotKind::Sub)).text);
}

#[test]
fn cells_know_their_row_and_column() {
    let stops = parsed(r"\begin{pmatrix}a&b\\c&\end{pmatrix}", false);
    let cells: Vec<(SlotKind, Vec<usize>)> = stops
        .slots()
        .iter()
        .filter(|slot| matches!(slot.kind, SlotKind::Cell { .. }))
        .map(|slot| {
            (
                slot.kind,
                slot.stops.iter().map(|&id| stops.offset(id)).collect(),
            )
        })
        .collect();
    assert_eq!(
        cells,
        [
            (SlotKind::Cell { row: 0, col: 0 }, vec![15, 16]),
            (SlotKind::Cell { row: 0, col: 1 }, vec![17, 18]),
            (SlotKind::Cell { row: 1, col: 0 }, vec![20, 21]),
            // Empty: one stop, just before `\end`.
            (SlotKind::Cell { row: 1, col: 1 }, vec![22]),
        ]
    );
    let stops = parsed(r"\begin{matrix}\end{matrix}", false);
    let cell = stops.slot(slot_of(&stops, SlotKind::Cell { row: 0, col: 0 }));
    assert_eq!(cell.interior, 14..14);
}

#[test]
fn top_level_rows_split_at_line_breaks() {
    let stops = parsed(r"a\\b\\", true);
    let rows: Vec<(SlotKind, Range<usize>)> = stops
        .slots()
        .iter()
        .map(|slot| (slot.kind, slot.interior.clone()))
        .collect();
    assert_eq!(
        rows,
        [
            (SlotKind::Row(0), 0..1),
            (SlotKind::Row(1), 3..4),
            (SlotKind::Row(2), 6..6),
        ]
    );
}

#[test]
fn shared_offsets_resolve_by_affinity() {
    // Offset 7 ends the numerator and starts the denominator.
    let stops = parsed(r"\frac ab", false);
    let before = stops.stop_at(7, Affinity::Before);
    let after = stops.stop_at(7, Affinity::After);
    assert_eq!(stops.slot(stops.stop(before).slot).kind, SlotKind::Numer);
    assert_eq!(stops.slot(stops.stop(after).slot).kind, SlotKind::Denom);
    assert_eq!(stops.next(before), Some(after));
    assert_eq!(stops.affinity(before), Some(Affinity::Before));
    assert_eq!(stops.affinity(after), Some(Affinity::After));

    // Three stops at the end: the denominator's, the radicand's, the
    // row's. The middle one is named by its slot.
    let stops = parsed(r"\sqrt\frac ab", false);
    let radicand = slot_of(&stops, SlotKind::Radicand);
    let middle = stops.stop_in(radicand, 13).unwrap();
    assert_eq!(stops.affinity(middle), None);
    assert_eq!(
        stops
            .slot(stops.stop(stops.stop_at(13, Affinity::Before)).slot)
            .kind,
        SlotKind::Denom
    );
    assert_eq!(
        stops
            .slot(stops.stop(stops.stop_at(13, Affinity::After)).slot)
            .kind,
        SlotKind::Row(0)
    );
}

#[test]
fn offsets_inside_an_atom_go_to_a_neighbour() {
    let stops = parsed(r"\alpha x", false);
    assert_eq!(stops.offset(stops.stop_at(3, Affinity::Before)), 0);
    assert_eq!(stops.offset(stops.stop_at(3, Affinity::After)), 7);
    assert_eq!(stops.offset(stops.stop_at(99, Affinity::After)), 8);
}

#[test]
fn moves_and_slot_lookups() {
    let stops = parsed(r"\frac{}{}+x", false);
    let first = StopId(0);
    assert_eq!(stops.prev(first), None);
    let numer = slot_of(&stops, SlotKind::Numer);
    let denom = slot_of(&stops, SlotKind::Denom);
    let row = slot_of(&stops, SlotKind::Row(0));
    assert_eq!(stops.parent(numer), Some(row));
    assert_eq!(stops.parent(row), None);
    assert_eq!(stops.first_stop(numer), stops.last_stop(numer));
    // Tab from the start reaches the numerator, then the denominator.
    let tab = stops.next_empty(first).unwrap();
    assert_eq!(stops.stop(tab).slot, numer);
    let tab = stops.next_empty(tab).unwrap();
    assert_eq!(stops.stop(tab).slot, denom);
    assert_eq!(stops.next_empty(tab), None);
    assert_eq!(
        stops.prev_empty(tab).map(|id| stops.stop(id).slot),
        Some(numer)
    );
    let end = StopId(stops.stops().len() - 1);
    assert_eq!(stops.next(end), None);
    assert_eq!(stops.offset(end), 11);
}

#[test]
fn paths_name_slots_by_atom_and_argument() {
    let stops = parsed(r"a\frac{b}{c^2}", false);
    let paths: Vec<(SlotKind, SlotPath)> = stops
        .slots()
        .iter()
        .map(|slot| (slot.kind, slot.path.clone()))
        .collect();
    let path = |steps: &[(usize, usize)]| SlotPath {
        row: 0,
        steps: steps.to_vec(),
    };
    assert_eq!(
        paths,
        [
            (SlotKind::Row(0), path(&[])),
            (SlotKind::Numer, path(&[(1, 0)])),
            (SlotKind::Denom, path(&[(1, 1)])),
            (SlotKind::Sup, path(&[(1, 1), (1, 0)])),
        ]
    );
}

#[test]
fn unparseable_source_has_no_stops() {
    for source in [r"\frac{a}{", "x^", "a}", r"\left(", r"\undefinedcommand"] {
        assert!(stops(source, false).is_err(), "{source:?}");
    }
}
