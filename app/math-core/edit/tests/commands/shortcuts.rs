//! Shortcuts: typed keys that expand (`sin` → `\sin`, `->` → `\to`), the
//! word-boundary rule, longer keys, the two undo steps and Esc's revert.

use oculus_math_edit::{Change, Command, Field, Outcome};

use super::harness::{Case, check_all, field, press};

#[test]
fn letter_keys_expand_when_the_whole_run_is_the_key() {
    let cases: &[Case] = &[
        ("|", &["t:alpha"], r"\alpha|"),
        ("|", &["t:sin"], r"\sin|"),
        ("|", &["t:Delta"], r"\Delta|"),
        ("|", &["t:xx"], r"\times|"),
        ("|", &["t:NN"], r"\mathbb{N}|"),
        ("a+|", &["t:pi"], r"a+\pi|"),
        // Never a suffix of a longer run.
        ("|", &["t:xsin"], "xsin|"),
        ("|", &["t:card"], "card|"),
        ("|", &["t:word"], "word|"),
        ("x|", &["t:pi"], "xpi|"),
        // Letters typed after letters already there make one run.
        ("si|", &["t:n"], r"\sin|"),
        // A digit, a control word or a script ends the run.
        ("|", &["t:2pi"], r"2\pi|"),
        (r"\alpha|", &["t:pi"], r"\alpha\pi|"),
        ("x^{2}|", &["t:pi"], r"x^{2}\pi|"),
        ("(|", &["t:pi"], r"(\pi|"),
        // Only the left side matters; glue spaces a letter after.
        ("|x", &["t:sin"], r"\sin |x"),
        ("|2", &["t:pi"], r"\pi|2"),
    ];
    check_all(cases, false);
}

#[test]
fn a_run_starts_fresh_after_an_expansion() {
    let cases: &[Case] = &[
        ("|", &["t:sintheta"], r"\sin\theta|"),
        ("|", &["t:sinx"], r"\sin x|"),
        ("|", &["t:pipi"], r"\pi\pi|"),
        ("|", &["t:alphabeta"], r"\alpha\beta|"),
    ];
    check_all(cases, false);
}

#[test]
fn powers_take_the_atom_before_as_their_base() {
    let cases: &[Case] = &[
        ("|", &["t:xsr"], "x^2¦"),
        ("|", &["t:xcb"], "x^3¦"),
        ("|", &["t:ainvs"], "a^{-1}|"),
        ("2|", &["t:sr"], "2^2¦"),
        (")|", &["t:sr"], ")^2¦"),
        (r"\alpha|", &["t:sr"], r"\alpha^2¦"),
        // A template: the caret in its slot.
        ("|", &["t:xrd"], "x^{|}"),
        ("|", &["t:xrd", "t:n"], "x^{n|}"),
        // No base, or more than one letter before: letters.
        ("|", &["t:sr"], "sr|"),
        ("a+|", &["t:sr"], "a+sr|"),
        ("|", &["t:card"], "card|"),
        // A base with a superscript already would not render.
        ("x^{2}|", &["t:sr"], "x^{2}sr|"),
    ];
    check_all(cases, false);
}

#[test]
fn symbol_keys_match_the_keys_just_typed() {
    let cases: &[Case] = &[
        ("|", &["t:->"], r"\to|"),
        ("a|", &["t:->b"], r"a\to b|"),
        ("|", &["t:<="], r"\le|"),
        ("|", &["t:>="], r"\ge|"),
        ("|", &["t:+-"], r"\pm|"),
        ("|", &["t:..."], r"\dots|"),
        ("a|", &["t:**"], r"a\cdot|"),
        ("a|", &["t:*"], r"a\cdot|"),
        ("|", &["t:=>"], r"\implies|"),
        ("|", &["t:~~"], r"\approx|"),
        ("|", &["t:@a"], r"\alpha|"),
        ("|", &["t:@ve"], r"\varepsilon|"),
        ("|", &["t:@"], r"\circ|"),
        ("x|", &["t:-lt"], r"x\prec|"),
        ("|", &["t:!exists"], r"\nexists|"),
        // Whatever the keys built on the way: `^^` is not a script in a
        // script, `//` not a fraction in a fraction.
        ("a|", &["t:^^"], r"a\wedge|"),
        ("a|", &["t://"], "a/|"),
        ("a|", &["t:(+)"], r"a\oplus|"),
        ("|", &["t:\u{2264}"], r"\le|"),
    ];
    check_all(cases, false);
}

#[test]
fn not_equal_is_a_factorial_after_an_operand() {
    let cases: &[Case] = &[
        ("|", &["t:!="], r"\ne|"),
        ("a+|", &["t:!="], r"a+\ne|"),
        ("(|", &["t:!="], r"(\ne|"),
        ("|", &["t:n!="], "n!=|"),
        ("|", &["t:2!="], "2!=|"),
        ("(a)|", &["t:!="], "(a)!=|"),
        ("x^{2}|", &["t:!="], "x^{2}!=|"),
    ];
    check_all(cases, false);
}

#[test]
fn a_longer_key_expands_again_from_before_the_first() {
    let cases: &[Case] = &[
        ("|", &["t:sinh"], r"\sinh|"),
        ("|", &["t:cosh"], r"\cosh|"),
        ("|", &["t:<=>"], r"\iff|"),
        ("|", &["t:->>"], r"\twoheadrightarrow|"),
        ("|", &["t:liminf"], r"\liminf_{|}"),
        ("|", &["t:nnn"], r"\bigcap|"),
        ("a|", &["t:***"], r"a\ast|"),
        ("|", &["t:->..."], r"\to\cdots|"),
        ("|", &["t:argmin"], r"\operatorname*{arg~min}_{|}"),
        // From before its first key even when a shorter key inside it
        // expanded (`*` is `\cdot`).
        ("a|", &["t:(*)"], r"a\otimes|"),
        // Primes stay primes.
        ("x|", &["t:'''"], "x'''|"),
        // A move in between ends the run.
        ("|", &["t:sin", "left", "right", "t:h"], r"\sin h|"),
        ("|", &["t:<=", "left", "right", "t:>"], r"\le>|"),
    ];
    check_all(cases, false);
}

#[test]
fn templates_leave_the_caret_in_their_first_slot() {
    let cases: &[Case] = &[
        ("|", &["t:sqrt"], r"\sqrt{|}"),
        ("|", &["t:sqrt", "t:2"], r"\sqrt{2|}"),
        ("|", &["t:sum"], r"\sum_{|}^{}"),
        ("|", &["t:int"], r"\int_{|}^{}"),
        ("|", &["t:prod"], r"\prod_{|}^{}"),
        ("|", &["t:log"], r"\log_{|}"),
        ("|", &["t:nthroot"], r"\sqrt[|]{}"),
        // A placeholder inside a slot: the caret goes there.
        ("|", &["t:lim"], r"\lim_{|\to}"),
        ("|", &["t:lim", "t:n"], r"\lim_{n|\to}"),
    ];
    check_all(cases, false);
}

#[test]
fn shortcuts_expand_in_any_maths_slot() {
    let cases: &[Case] = &[
        (r"\frac{|}{b}", &["t:pi"], r"\frac{\pi|}{b}"),
        ("x^{|}", &["t:sin"], r"x^{\sin|}"),
        // A bare script takes braces for the second letter.
        ("x^s|", &["t:in"], r"x^{\sin|}"),
        (r"\sqrt{|}", &["t:->"], r"\sqrt{\to|}"),
        (
            r"\begin{matrix}a & |\end{matrix}",
            &["t:pi"],
            r"\begin{matrix}a & \pi|\end{matrix}",
        ),
    ];
    check_all(cases, false);
}

#[test]
fn shortcuts_never_expand_outside_maths_typing() {
    let cases: &[Case] = &[
        (r"\text{|}", &["t:sin"], r"\text{sin|}"),
        (r"\text{|}", &["t:->"], r"\text{->|}"),
        // In a pending command the letters are its name.
        ("|", &[r"t:\alpha", "space"], r"\alpha|"),
        ("|", &[r"t:\sin", "space"], r"\sin|"),
        // An IME's string, a template, a paste go in as they are.
        ("|", &["ime:alpha"], "alpha|"),
        ("|", &["ime:->"], "->|"),
        ("|", &["tpl:pi"], "pi|"),
        ("|", &["paste:alpha"], "alpha|"),
        // A name is not maths.
        (r"\operatorname{|}", &["t:sinc"], r"\operatorname{sinc|}"),
        (r"\mathbb{|}", &["t:RR"], r"\mathbb{RR|}"),
        (r"\mathrm{|}", &["t:d->"], r"\mathrm{d->|}"),
    ];
    check_all(cases, false);
}

#[test]
fn a_selection_is_replaced_then_the_run_rule_applies() {
    let cases: &[Case] = &[
        ("‹x›", &["t:pi"], r"\pi|"),
        ("si‹x›", &["t:n"], r"\sin|"),
        ("a‹x›", &["t:->"], r"a\to|"),
    ];
    check_all(cases, false);
}

#[test]
fn esc_right_after_an_expansion_puts_the_keys_back() {
    let cases: &[Case] = &[
        ("|", &["t:sin", "esc"], "sin|"),
        ("|", &["t:alpha", "esc"], "alpha|"),
        ("|", &["t:<=", "esc"], "<=|"),
        ("|", &["t:->", "esc"], "->|"),
        ("|", &["t:sinh", "esc"], "sinh|"),
        ("|", &["t:<=>", "esc"], "<=>|"),
        ("|", &["t:sqrt", "esc"], "sqrt|"),
        ("|", &["t:xsr", "esc"], "xsr|"),
        ("|", &["t:@a", "esc"], "@a|"),
        ("|", &["t:sintheta", "esc"], r"\sin theta|"),
        ("si|", &["t:n", "esc"], "sin|"),
        ("a|", &["t:^^", "esc"], "a^{^{|}}"),
        // A second Esc leaves the field as usual.
        ("|", &["t:sin", "esc", "esc"], "sin|  => Leave(Right)"),
    ];
    check_all(cases, false);
}

#[test]
fn esc_after_anything_else_leaves_the_field() {
    let cases: &[Case] = &[
        ("|", &["t:sin", "left", "esc"], r"|\sin  => Leave(Right)"),
        (
            "|",
            &["t:sin", "left", "right", "esc"],
            r"\sin|  => Leave(Right)",
        ),
        ("|", &["t:sin", "bs", "esc"], "|  => Leave(Right)"),
        (
            "|",
            &["t:sqrt", "t:2", "esc"],
            r"\sqrt{2|}  => Leave(Right)",
        ),
        (
            "|",
            &["t:lim", "t:i", "esc"],
            r"\lim_{i|\to}  => Leave(Right)",
        ),
        ("|", &["t:sin", "tpl:x", "esc"], r"\sin x|  => Leave(Right)"),
        // A pending command: Esc cancels it first.
        ("|", &["t:sin", r"t:\al", "esc"], r"\sin|"),
    ];
    check_all(cases, false);
}

#[test]
fn after_a_revert_the_keys_that_lead_on_stay_as_typed() {
    let cases: &[Case] = &[
        ("|", &["t:sin", "esc", "t:h"], "sinh|"),
        ("|", &["t:<=", "esc", "t:>"], "<=>|"),
        ("|", &["t:->", "esc", "t:>"], "->>|"),
        ("|", &["t:nn", "esc", "t:n"], "nnn|"),
        // The first key that leads nowhere ends it, and may expand.
        ("|", &["t:sin", "esc", "t:@"], r"sin\circ|"),
        ("|", &["t:->", "esc", "t:>..."], r"->>\dots|"),
        // Letters still make one run with the reverted ones.
        ("|", &["t:sin", "esc", "t:theta"], "sintheta|"),
        ("|", &["t:pi", "esc", "t:x"], "pix|"),
        // Any other command ends it, and the run of letters is a key.
        ("|", &["t:sin", "esc", "left", "right", "t:h"], r"\sinh|"),
        ("|", &["t:nn", "esc", "left", "right", "t:n"], r"\bigcap|"),
    ];
    check_all(cases, false);
}

/// The key lands as an edit of the typing run, then the expansion is an
/// undo step of its own; Esc's revert is one too.
#[test]
fn an_expansion_is_a_second_undo_step() {
    let mut f = field("|", false);
    for key in ["s", "i"] {
        f = f.run(&Command::Insert(key.to_owned())).field;
    }
    // Each step is the smallest change.
    let typed = f.run(&Command::Insert("n".to_owned()));
    assert_eq!(typed.changes, vec![change(2, 2, "n")]);
    assert!(!typed.isolate);
    assert_eq!(typed.rewrite, Some(vec![change(0, 0, "\\")]));
    assert_eq!(typed.field.source(), r"\sin");

    let extended = typed.field.run(&Command::Insert("h".to_owned()));
    assert_eq!(extended.changes, vec![change(4, 4, " h")]);
    assert_eq!(extended.rewrite, Some(vec![change(4, 5, "")]));
    assert_eq!(extended.field.source(), r"\sinh");

    let reverted = extended.field.run(&Command::Escape);
    assert_eq!(reverted.changes, vec![change(0, 1, "")]);
    assert_eq!(reverted.field.source(), "sinh");
    assert!(reverted.isolate);
    assert_eq!(reverted.rewrite, None);
    assert_eq!(reverted.effect, None);

    let arrow = field("a|", false)
        .run(&Command::Insert("-".to_owned()))
        .field
        .run(&Command::Insert(">".to_owned()));
    assert_eq!(arrow.changes, vec![change(2, 2, ">")]);
    assert_eq!(arrow.rewrite, Some(vec![change(1, 3, r"\to")]));

    // No shortcut: no rewrite.
    let plain: Outcome = field("|", false).run(&Command::Insert("x".to_owned()));
    assert_eq!(plain.rewrite, None);
}

/// Every value goes in where a shortcut puts it, and its keys typed into
/// an empty field (after `x` for a power) expand to it.
#[test]
fn every_shortcut_renders_and_expands_from_its_keys() {
    let mut failed = Vec::new();
    for &(keys, value) in oculus_math_edit::SHORTCUTS {
        let filled = value.replace("#0", "").replace("#?", "");
        if oculus_math_edit::renders(&filled, false).is_err() {
            failed.push(format!("{keys:?}: {value:?} does not render"));
            continue;
        }
        let base = if ["sr", "cb", "rd", "invs"].contains(&keys) {
            "x"
        } else {
            ""
        };
        let mut f = Field::new(base, false).unwrap();
        for c in keys.chars() {
            f = f.run(&Command::Insert(c.to_string())).field;
        }
        let expected = format!("{base}{filled}");
        if f.source() != expected {
            failed.push(format!(
                "{keys:?}: typed {:?}, want {expected:?}",
                f.source()
            ));
        }
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

fn change(from: usize, to: usize, insert: &str) -> Change {
    Change {
        from,
        to,
        insert: insert.to_owned(),
    }
}

#[test]
fn display_maths_too() {
    assert_eq!(press("a|", true, &["t:->"]), r"a\to|");
}
