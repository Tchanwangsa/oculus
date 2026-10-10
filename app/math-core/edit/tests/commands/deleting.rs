//! Backspace, Delete and ⌘Backspace.

use super::harness::{Case, check_all};

#[test]
fn plain_atoms_delete_one_at_a_time() {
    let cases: &[Case] = &[
        ("ab|", &["bs"], "a|"),
        ("|ab", &["del"], "|b"),
        ("a|b", &["bs", "del"], "|"),
        (r"\alpha|+1", &["bs"], "|+1"),
        ("‹ab›c", &["bs"], "|c"),
        ("a‹bc›", &["del"], "a|"),
    ];
    check_all(cases, false);
}

#[test]
fn deleting_keeps_control_words_apart_from_letters() {
    let cases: &[Case] = &[
        // The space a typed letter brought goes with it.
        (r"\alpha x|", &["bs"], r"\alpha|"),
        (r"\alpha|", &["t:x", "bs"], r"\alpha|"),
        (r"\alpha xy|", &["bs"], r"\alpha x|"),
        // A letter that meets the word gets one.
        (r"\alpha+|x", &["bs"], r"\alpha |x"),
        (r"\alpha|+x", &["del"], r"\alpha |x"),
    ];
    check_all(cases, false);
}

#[test]
fn deleting_into_a_structure_selects_it_first() {
    let cases: &[Case] = &[
        (r"\frac{a}{b}|", &["bs"], r"›\frac{a}{b}‹"),
        (r"\frac{a}{b}|", &["bs", "bs"], "|"),
        (r"|\sqrt{x}+1", &["del"], r"‹\sqrt{x}›+1"),
        (r"|\sqrt{x}+1", &["del", "del"], "|+1"),
        ("x^{2}|", &["bs"], "x›^{2}‹"),
        ("x|^{2}", &["del"], "x‹^{2}›"),
        ("x|^{2}", &["bs"], "|^{2}"),
        // At a slot's start with something in the structure: selected.
        (r"\frac{|a}{b}", &["bs"], r"›\frac{a}{b}‹"),
        (r"\frac{a}{b|}", &["del"], r"‹\frac{a}{b}›"),
    ];
    check_all(cases, false);
}

#[test]
fn an_empty_structure_goes_at_once() {
    let cases: &[Case] = &[
        (r"a\frac{|}{}", &["bs"], "a|"),
        (r"a\sqrt{|}b", &["del"], "a|b"),
        ("x^{|}", &["bs"], "x|"),
        ("a{|}", &["bs"], "a|"),
        // What was inserted, deleted: the source comes back.
        ("a|", &[r"tpl:\frac{#0}{#?}", "bs"], "a|"),
        ("a|", &["t:^", "bs"], "a|"),
        ("a|", &["t:{", "bs"], "a|"),
    ];
    check_all(cases, false);
}

/// doc 82-84.
#[test]
fn backspace_in_an_empty_script_drops_it_with_the_caret_after_its_base() {
    let cases: &[Case] = &[
        (r"\cos^{|}", &["bs"], r"\cos|"),
        // The script gone, `\cos` and `x` need a space between them.
        (r"\cos^{|}x", &["bs"], r"\cos |x"),
        ("x_{1}^{|}", &["bs"], "x_{1}|"),
        ("x_{|}^{2}", &["bs"], "x|^{2}"),
    ];
    check_all(cases, false);
}

#[test]
fn an_emptied_bare_argument_keeps_braces() {
    let cases: &[Case] = &[
        ("x^2|", &["bs"], "x^{|}"),
        (r"\frac a|b", &["bs"], r"\frac {|}b"),
        ("x^‹2›", &["del"], "x^{|}"),
    ];
    check_all(cases, false);
}

/// doc 81-82.
#[test]
fn backspace_in_an_empty_field_removes_the_maths() {
    let cases: &[Case] = &[
        ("|", &["bs"], "|  => RemoveMaths"),
        ("| ", &["bs"], "|   => RemoveMaths"),
        ("|", &["cmd-bs"], "|  => RemoveMaths"),
        ("|", &["del"], "|"),
        ("|a", &["bs"], "|a"),
    ];
    check_all(cases, false);
}

#[test]
fn rows_join_at_their_edges() {
    let cases: &[Case] = &[
        (r"a\\|b", &["bs"], "a|b"),
        (r"a|\\b", &["del"], "a|b"),
        (r"a\\b|", &["del"], r"a\\b|"),
    ];
    check_all(cases, true);
}

#[test]
fn cells_step_to_their_neighbour_at_their_edges() {
    let cases: &[Case] = &[
        (
            r"\begin{aligned}a&|=b\end{aligned}",
            &["bs"],
            r"\begin{aligned}a|&=b\end{aligned}",
        ),
        (
            r"\begin{aligned}a|&=b\end{aligned}",
            &["del"],
            r"\begin{aligned}a&|=b\end{aligned}",
        ),
    ];
    check_all(cases, true);
}

#[test]
fn text_deletes_a_character_at_a_time() {
    let cases: &[Case] = &[
        (r"\text{ab|}", &["bs"], r"\text{a|}"),
        (r"\text{a|b}", &["del"], r"\text{a|}"),
        // A Thai vowel stays with its consonant.
        (r"\text{ดี|}", &["bs"], r"\text{|}"),
        (r"\text{a|ดี}", &["del"], r"\text{a|}"),
        (r"\text{a\#|}", &["bs"], r"\text{a|}"),
        (r"\text{a\textasciicircum{}|}", &["bs"], r"\text{a|}"),
        (r"\text{a|\textasciicircum{}}", &["del"], r"\text{a|}"),
        // At an empty run's start the `\text{}` goes.
        (r"x\text{|}", &["bs"], "x|"),
        (r"\text{|a}", &["bs"], r"›\text{a}‹"),
    ];
    check_all(cases, false);
}

/// doc 79-81.
#[test]
fn command_backspace_deletes_the_line_up_to_the_caret() {
    check_all(
        &[
            ("a+b|", &["cmd-bs"], "|"),
            ("a+b|c", &["cmd-bs"], "|c"),
            // Through the structure the caret is in.
            (r"x+\frac{a|}{b}+y", &["cmd-bs"], "|+y"),
        ],
        false,
    );
    check_all(
        &[
            (r"a\\b+c|", &["cmd-bs"], r"a\\|"),
            // At a line's start it is Backspace.
            (r"a\\|b", &["cmd-bs"], "a|b"),
            (
                r"\begin{aligned}a&=b+c|\end{aligned}",
                &["cmd-bs"],
                r"\begin{aligned}a&|\end{aligned}",
            ),
        ],
        true,
    );
}
