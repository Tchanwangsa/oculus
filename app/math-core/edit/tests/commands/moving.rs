//! ←/→, Shift, Home/End, ↑/↓, select all, Tab, Esc.

use super::harness::{Case, check_all};

#[test]
fn arrows_walk_the_stops_and_leave_past_the_edge() {
    let cases: &[Case] = &[
        ("a|b", &["right"], "ab|"),
        ("a|b", &["left"], "|ab"),
        ("ab|", &["right"], "ab|  => Leave(Right)"),
        ("|ab", &["left"], "|ab  => Leave(Left)"),
        (r"x|\frac{a}{b}", &["right"], r"x\frac{|a}{b}"),
        (r"\frac{a|}{b}", &["right"], r"\frac{a}{|b}"),
        // The stop between a base and its script is kept.
        ("x^{2}|", &["left", "left", "left"], "x|^{2}"),
        // A selection collapses to the side the arrow points to.
        ("‹ab›", &["left"], "|ab"),
        ("‹ab›", &["right"], "ab|"),
        ("›ab‹", &["right"], "ab|"),
    ];
    check_all(cases, false);
}

#[test]
fn shift_arrows_extend_over_whole_structures() {
    let cases: &[Case] = &[
        ("a|bc", &["s-right", "s-right"], "a‹bc›"),
        ("a|bc", &["s-left"], "›a‹bc"),
        (r"x|\frac{a}{b}", &["s-right"], r"x‹\frac{a}{b}›"),
        (r"x|\frac{a}{b}", &["s-right", "s-left"], r"x|\frac{a}{b}"),
        (r"\frac{a|}{b}", &["s-right"], r"‹\frac{a}{b}›"),
        (r"\frac{a|}{b}", &["s-left"], r"\frac{›a‹}{b}"),
        (r"\frac{|a}{b}", &["s-left"], r"›\frac{a}{b}‹"),
        ("x^{2|}", &["s-right"], "x‹^{2}›"),
        ("‹ab›", &["s-right"], "‹ab›"),
    ];
    check_all(cases, false);
}

/// A block's rows select as one run.
#[test]
fn a_selection_across_rows_keeps_each_end_in_its_row() {
    let cases: &[Case] = &[
        (r"a|\\b", &["s-right", "s-right"], r"a‹\\b›"),
        (
            r"\frac{x|}{y}\\b",
            &["s-end", "s-right", "s-right"],
            r"‹\frac{x}{y}\\b›",
        ),
    ];
    check_all(cases, true);
}

#[test]
fn home_and_end_go_to_the_row_ends() {
    check_all(
        &[
            (r"a\frac{b|}{c}", &["home"], r"|a\frac{b}{c}"),
            (r"a\frac{b|}{c}", &["end"], r"a\frac{b}{c}|"),
            ("a|bc", &["s-end"], "a‹bc›"),
            (r"a\frac{b|}{c}", &["s-home"], r"›a\frac{b}{c}‹"),
        ],
        false,
    );
    check_all(
        &[
            (r"a+b\\c|d", &["home"], r"a+b\\|cd"),
            (r"a+b|\\cd", &["end"], r"a+b|\\cd"),
            (r"a|+b\\cd", &["end"], r"a+b|\\cd"),
        ],
        true,
    );
}

#[test]
fn select_all_takes_the_field() {
    check_all(&[(r"a|\frac{b}{c}", &["all"], r"‹a\frac{b}{c}›")], false);
}

#[test]
fn up_and_down_move_between_stacked_slots_by_x() {
    let cases: &[Case] = &[
        (r"\frac{ab|}{cd}", &["down"], r"\frac{ab}{cd|}"),
        (r"\frac{ab}{c|d}", &["up"], r"\frac{a|b}{cd}"),
        (r"\frac{abc|}{d}", &["down"], r"\frac{abc}{d|}"),
        ("x^{2|}_{1}", &["down"], "x^{2}_{1|}"),
        ("x^{2}_{|1}", &["up"], "x^{|2}_{1}"),
        (r"\sqrt[3|]{x}", &["down"], r"\sqrt[3]{x|}"),
        (r"\overset{a|}{b}", &["down"], r"\overset{a}{b|}"),
        // No stacked slot here: the structure around is tried.
        (r"\frac{x^{2|}}{y}", &["down"], r"\frac{x^{2}}{y|}"),
        // None at all: the caret leaves.
        ("a|", &["up"], "a|  => Leave(Up)"),
        (r"\frac{a}{b|}", &["down"], r"\frac{a}{b|}  => Leave(Down)"),
    ];
    check_all(cases, false);
}

#[test]
fn up_and_down_move_between_rows() {
    let cases: &[Case] = &[
        (r"ab|\\c", &["down"], r"ab\\c|"),
        (r"ab\\|c", &["up"], r"|ab\\c"),
        (
            r"\begin{aligned}a&=b|\\c&=d\end{aligned}",
            &["down"],
            r"\begin{aligned}a&=b\\c&=d|\end{aligned}",
        ),
    ];
    check_all(cases, true);
}

#[test]
fn tab_goes_to_the_next_empty_slot_else_types_a_qquad() {
    let cases: &[Case] = &[
        (r"|\frac{}{}", &["tab"], r"\frac{|}{}"),
        (r"\frac{|}{}", &["tab"], r"\frac{}{|}"),
        (r"\frac{}{|}", &["tab"], r"\frac{}{\qquad|}"),
        ("a|", &["tab"], r"a\qquad|"),
        (r"\frac{}{|}", &["s-tab"], r"\frac{|}{}"),
        ("a|", &["s-tab"], "a|"),
    ];
    check_all(cases, false);
}

#[test]
fn tab_or_right_at_the_end_of_text_goes_back_to_maths() {
    let cases: &[Case] = &[
        (r"\text{ab|}x", &["right"], r"\text{ab}|x"),
        (r"\text{a|b}x", &["tab"], r"\text{ab}|x"),
        (r"\text{a|b}x", &["right"], r"\text{ab|}x"),
    ];
    check_all(cases, false);
}

#[test]
fn escape_leaves() {
    check_all(&[("a|", &["esc"], "a|  => Leave(Right)")], false);
}

#[test]
fn aligned_carets_stay_in_rows_and_command_arrows_cross_columns() {
    check_all(
        &[
            (
                r"|\begin{aligned}a&=b\\P&=\sin^{2}(\theta)\end{aligned}",
                &["t:x"],
                r"\begin{aligned}x|a&=b\\P&=\sin^{2}(\theta)\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\P&=\sin^{2}(\theta|)\end{aligned}",
                &["home"],
                r"\begin{aligned}a&=b\\|P&=\sin^{2}(\theta)\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\P&=\sin^{2|}(\theta)\end{aligned}",
                &["end"],
                r"\begin{aligned}a&=b\\P&=\sin^{2}(\theta)|\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\P&=x|\end{aligned}",
                &["s-home"],
                r"\begin{aligned}a&=b\\›P&=x‹\end{aligned}",
            ),
            (
                r"\begin{aligned}|a&=b\\P&=x\end{aligned}",
                &["left"],
                r"\begin{aligned}|a&=b\\P&=x\end{aligned}  => Leave(Left)",
            ),
            (
                r"\begin{aligned}a&=b\\P&=x|\end{aligned}",
                &["right"],
                r"\begin{aligned}a&=b\\P&=x|\end{aligned}  => Leave(Right)",
            ),
            (
                r"\begin{aligned}a&=b|\\P&=x\end{aligned}",
                &["right"],
                r"\begin{aligned}a&=b\\|P&=x\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\|P&=x\end{aligned}",
                &["left"],
                r"\begin{aligned}a&=b|\\P&=x\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\P&=x|\end{aligned}",
                &["all"],
                r"‹\begin{aligned}a&=b\\P&=x\end{aligned}›",
            ),
        ],
        true,
    );
}

#[test]
fn option_arrows_jump_runs_structures_and_text_words() {
    check_all(
        &[(r"\text{สวัสดี| ไทย}", &["opt-left"], r"\text{|สวัสดี ไทย}")],
        false,
    );
    check_all(
        &[
            (r"\frac{a{bc|}d}{e}", &["opt-left"], r"\frac{a{|bc}d}{e}"),
            (r"\frac{a{|bc}d}{e}", &["opt-left"], r"\frac{a|{bc}d}{e}"),
            (
                r"\frac{a|{bc}d}{e}",
                &["opt-left", "opt-left"],
                r"|\frac{a{bc}d}{e}",
            ),
            (r"\frac{a{b|c}d}{e}", &["opt-right"], r"\frac{a{bc|}d}{e}"),
            (r"\frac{a{bc|}d}{e}", &["opt-right"], r"\frac{a{bc}|d}{e}"),
            (r"\frac{a{bc}d|}{e}", &["opt-right"], r"\frac{a{bc}d}{e}|"),
            (r"x|\frac{a}{b}+cd", &["opt-right"], r"x\frac{a}{b}|+cd"),
            ("ab+cd|", &["opt-left"], "ab+|cd"),
            ("ab+|cd", &["opt-left"], "ab|+cd"),
            (
                r"\text{hello big wor|ld}",
                &["opt-left"],
                r"\text{hello big |world}",
            ),
            (
                r"\text{hello big |world}",
                &["opt-left"],
                r"\text{hello |big world}",
            ),
            (
                r"\text{hel|lo big world}",
                &["opt-right"],
                r"\text{hello| big world}",
            ),
            (
                r"\text{hello| big world}",
                &["opt-right"],
                r"\text{hello big| world}",
            ),
            (r"\text{สวั|สดี ไทย}", &["opt-left"], r"\text{|สวัสดี ไทย}"),
            (
                r"\text{hello wor|ld}",
                &["s-opt-left"],
                r"\text{hello ›wor‹ld}",
            ),
            (r"x|\frac{ab}{c}", &["s-opt-right"], r"x‹\frac{ab}{c}›"),
            ("ab|", &["opt-right"], "ab|  => Leave(Right)"),
        ],
        false,
    );
}
