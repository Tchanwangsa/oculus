//! Typing in maths: characters, structure keys, templates, paste.

use super::harness::{Case, check_all};

#[test]
fn characters_go_in_as_themselves_or_escaped() {
    let cases: &[Case] = &[
        ("|", &["t:x+1"], "x+1|"),
        ("|x", &["t:#%$&"], r"\#\%\$\&|x"),
        // `~` would draw as a space: it means "similar to".
        ("|", &["t:~"], r"\sim|"),
        ("x|", &["t:'"], "x'|"),
        ("x|", &["t:="], "x=|"),
        // Space types nothing in maths (the view's quick picks).
        ("x|", &["space"], "x|"),
        ("‹x›+1", &["t:y"], "y|+1"),
    ];
    check_all(cases, false);
}

#[test]
fn a_letter_after_a_control_word_is_spaced_off() {
    let cases: &[Case] = &[
        (r"\alpha|", &["t:x"], r"\alpha x|"),
        (r"\alpha|", &["t:2"], r"\alpha2|"),
        ("|x", &[r"tpl:\sin"], r"\sin |x"),
    ];
    check_all(cases, false);
}

#[test]
fn typing_at_the_base_script_stop_goes_before_the_script() {
    check_all(&[("x|^2", &["t:y"], "xy|^2")], false);
}

#[test]
fn a_bare_argument_takes_braces_for_a_second_atom() {
    let cases: &[Case] = &[
        ("x^2|", &["t:3"], "x^{23|}"),
        ("x^|2", &["t:3"], "x^{3|2}"),
        ("x^2¦", &["t:3"], "x^23|"),
        (r"\frac a|b", &["t:c"], r"\frac {ac|}b"),
        (r"\hat x|", &["t:y"], r"\hat {xy|}"),
        // One atom for one: it stays bare.
        ("x^‹2›", &["t:3"], "x^3|"),
        ("x^‹2›", &["t:+"], "x^{+|}"),
        (r"\sqrt\frac ab|", &["t:c"], r"\sqrt\frac a{bc|}"),
    ];
    check_all(cases, false);
}

#[test]
fn braces() {
    let cases: &[Case] = &[
        ("a|", &["t:{"], "a{|}"),
        ("‹ab›", &["t:{"], "{ab}|"),
        ("{a|}b", &["t:}"], "{a}|b"),
        (r"\frac{a|}{b}", &["t:}"], r"\frac{a}{|b}"),
        ("a|", &["t:}"], "a|"),
    ];
    check_all(cases, false);
}

#[test]
fn scripts() {
    let cases: &[Case] = &[
        ("x|", &["t:^"], "x^{|}"),
        ("x|", &["t:_2"], "x_{2|}"),
        (r"\cos|", &["t:^2"], r"\cos^{2|}"),
        // An atom with that script already: the caret goes into it.
        ("x^{2}|", &["t:^"], "x^{2|}"),
        ("x|^{2}", &["t:^"], "x^{2|}"),
        ("x|^{2}", &["t:_"], "x_{|}^{2}"),
        ("x_{1}|", &["t:^"], "x_{1}^{|}"),
        // No base: KaTeX takes a base-less script.
        ("|", &["t:^"], "^{|}"),
        // A selection is the base.
        ("‹ab›", &["t:^"], "{ab}^{|}"),
        ("‹x›", &["t:_"], "x_{|}"),
    ];
    check_all(cases, false);
}

#[test]
fn slash_makes_a_fraction_of_the_term_before() {
    let cases: &[Case] = &[
        ("x|", &["t:/"], r"\frac{x}{|}"),
        ("12|", &["t:/"], r"\frac{12}{|}"),
        ("a+b|", &["t:/"], r"a+\frac{b}{|}"),
        ("x^{2}|", &["t:/"], r"\frac{x^{2}}{|}"),
        ("x'|", &["t:/"], r"\frac{x'}{|}"),
        ("2(a+b)|", &["t:/"], r"2\frac{(a+b)}{|}"),
        (r"\sqrt{x}|", &["t:/"], r"\frac{\sqrt{x}}{|}"),
        // After an operator or at a slot's start: an empty numerator.
        ("a+|", &["t:/"], r"a+\frac{|}{}"),
        ("a=|", &["t:/"], r"a=\frac{|}{}"),
        ("|", &["t:/"], r"\frac{|}{}"),
        ("‹a+b›", &["t:/"], r"\frac{a+b}{|}"),
        // Inside a bare argument: braces first.
        ("x^2|", &["t:/"], r"x^{\frac{2}{|}}"),
    ];
    check_all(cases, false);
}

#[test]
fn templates_fill_their_first_slot_with_the_selection() {
    let cases: &[Case] = &[
        ("x|", &[r"tpl:\sqrt{#0}"], r"x\sqrt{|}"),
        ("‹x›", &[r"tpl:\sqrt{#0}"], r"\sqrt{x}|"),
        ("|", &[r"tpl:\frac{#0}{#?}"], r"\frac{|}{}"),
        ("‹a›", &[r"tpl:\frac{#0}{#?}"], r"\frac{a}{|}"),
        ("x|", &["tpl:^{#?}"], "x^{|}"),
        (r"\alpha|", &["tpl:x"], r"\alpha x|"),
        // A text command's slot is text mode.
        ("|", &[r"tpl:\text{#0}"], r"\text{|}"),
        // A template that would not render here does nothing.
        ("x^{2}|", &["tpl:^{#?}"], "x^{2}|"),
    ];
    check_all(cases, false);
}

#[test]
fn paste_goes_in_only_when_it_renders() {
    let cases: &[Case] = &[
        // After it in the row, not in its bare denominator.
        ("a|", &[r"paste:\frac12"], r"a\frac12¦"),
        ("‹a›b", &["paste:x^2"], "x^2¦b"),
        ("a|", &[r"paste:\frac{"], "a|"),
        ("a|", &[r"paste:\undefined"], "a|"),
    ];
    check_all(cases, false);
}
