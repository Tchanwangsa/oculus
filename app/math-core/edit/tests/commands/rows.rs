//! Enter.

use super::harness::{Case, check_all};

/// doc 71-72.
#[test]
fn enter_in_inline_maths_leaves() {
    check_all(&[("a|b", &["enter"], "a|b  => Leave(Right)")], false);
}

/// doc 73-78.
#[test]
fn enter_in_display_maths_adds_a_row_never_a_second_empty_one() {
    let cases: &[Case] = &[
        ("a|b", &["enter"], r"a\\|b"),
        ("ab|", &["enter"], r"ab\\|"),
        // Inside a structure the row splits after it.
        (r"\frac{a|}{b}c", &["enter"], r"\frac{a}{b}\\|c"),
        // On an empty row nothing happens.
        ("|", &["enter"], "|"),
        ("ab|", &["enter", "enter"], r"ab\\|"),
        (r"a|\\", &["enter"], r"a\\|"),
        (r"a\\|", &["enter"], r"a\\|"),
        (r"\\|a", &["enter"], r"\\|a"),
    ];
    check_all(cases, true);
}

/// doc 73-78: in an environment's rows, a new row of empty cells.
#[test]
fn enter_in_an_array_adds_an_array_row() {
    let cases: &[Case] = &[
        (
            r"\begin{aligned}a&=b|\end{aligned}",
            &["enter"],
            r"\begin{aligned}a&=b\\|&\end{aligned}",
        ),
        (
            r"\begin{aligned}a|&=b\\c&=d\end{aligned}",
            &["enter"],
            r"\begin{aligned}a&=b\\|&\\c&=d\end{aligned}",
        ),
        // Beside an array that is the whole row: the row joins it.
        (
            r"\begin{aligned}a&=b\end{aligned}|",
            &["enter"],
            r"\begin{aligned}a&=b\\|&\end{aligned}",
        ),
        // An empty array row: nothing; one after: the caret goes there.
        (
            r"\begin{aligned}a&=b\\|&\end{aligned}",
            &["enter"],
            r"\begin{aligned}a&=b\\|&\end{aligned}",
        ),
        (
            r"\begin{aligned}a&=b|\\&\end{aligned}",
            &["enter"],
            r"\begin{aligned}a&=b\\|&\end{aligned}",
        ),
        // KaTeX drops a last row of one empty cell: no row to add.
        (
            r"\begin{gathered}a|\end{gathered}",
            &["enter"],
            r"\begin{gathered}a|\end{gathered}",
        ),
        (
            r"\begin{cases}a&b|\end{cases}",
            &["enter"],
            r"\begin{cases}a&b\\|&\end{cases}",
        ),
    ];
    check_all(cases, true);
}
