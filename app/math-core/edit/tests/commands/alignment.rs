//! Alignment points delete as syntax; only consecutive aligned rows stay wrapped.

use super::harness::{check_all, field, marked};
use oculus_math_edit::{Affinity, Command};

#[test]
fn deleting_points_shrinks_and_splits_the_aligned_runs() {
    check_all(
        &[
            (
                r"\begin{aligned}a&|=b\\c&=d\\e&=f\end{aligned}",
                &["bs"],
                r"a|=b\\\begin{aligned}c&=d\\e&=f\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\c&|=d\\e&=f\end{aligned}",
                &["bs"],
                r"\begin{aligned}a&=b\end{aligned}\\c|=d\\\begin{aligned}e&=f\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\c&=d\\e&|=f\end{aligned}",
                &["bs"],
                r"\begin{aligned}a&=b\\c&=d\end{aligned}\\e|=f",
            ),
            (
                r"\begin{aligned}a&|=b&c\\d&=e&f\end{aligned}",
                &["bs"],
                r"\begin{aligned}a|=b&c\\d&=e&f\end{aligned}",
            ),
            (
                r"\begin{aligned}a|&=b\\c&=d\end{aligned}",
                &["del"],
                r"a|=b\\\begin{aligned}c&=d\end{aligned}",
            ),
        ],
        true,
    );
}

#[test]
fn removing_every_point_leaves_only_plain_rows() {
    let mut f = field(r"\begin{aligned}|a&=b\\c&=d\\e&=f\\g&=h\end{aligned}", true);
    for _ in 0..4 {
        let at = f.source().find('&').unwrap() + 1;
        f = f
            .caret_at(at, Affinity::After)
            .run(&Command::Backspace)
            .field;
    }
    assert_eq!(marked(&f), r"a=b\\c=d\\e=f\\g|=h");
}

#[test]
fn nested_structures_keep_the_grouping_they_need() {
    check_all(
        &[
            (
                r"\frac{\begin{aligned}a&|=b\end{aligned}}{x}",
                &["bs"],
                r"\frac{a|=b}{x}",
            ),
            (
                r"\frac{\begin{aligned}a&|=b\\c=d\end{aligned}}{x}",
                &["bs"],
                r"\frac{\begin{gathered}a|=b\\c=d\end{gathered}}{x}",
            ),
            (r"\begin{aligned}a&|=b\end{aligned}^2", &["bs"], "{a|=b}^2"),
            (
                r"\begin{aligned}a&|=b\\c&=d\end{aligned}+x",
                &["bs"],
                r"\begin{gathered}a|=b\\\begin{aligned}c&=d\end{aligned}\end{gathered}+x",
            ),
            (
                r"\begin{aligned}a&|\begin{matrix}x&y\end{matrix}\end{aligned}",
                &["bs"],
                r"a|\begin{matrix}x&y\end{matrix}",
            ),
        ],
        true,
    );
}

#[test]
fn empty_cells_and_literal_ampersands_do_not_keep_alignment_alive() {
    check_all(
        &[
            (r"\begin{aligned}&|=b\end{aligned}", &["bs"], "|=b"),
            (r"\begin{aligned}a&|\end{aligned}", &["bs"], "a|"),
            (r"\begin{aligned}&|\end{aligned}", &["bs"], "|"),
            (
                r"\begin{aligned}\text{a\&b}&|=c\end{aligned}",
                &["bs"],
                r"\text{a\&b}|=c",
            ),
            (r"\begin{aligned}‹a&›=b\end{aligned}", &["bs"], "|=b"),
        ],
        true,
    );
}

#[test]
fn syntax_spacing_and_local_styles_survive() {
    check_all(
        &[
            (
                r"\begin{aligned}\alpha&|b\end{aligned}",
                &["bs"],
                r"\alpha |b",
            ),
            (
                r"\begin{aligned}[t]a&|=b\\[2pt]c&=d\end{aligned}",
                &["bs"],
                r"[t]a|=b\\[2pt]\begin{aligned}c&=d\end{aligned}",
            ),
            (
                "\\begin{aligned}\na&|=b \\\\\nc&=d\n\\end{aligned}",
                &["bs"],
                "\na|=b \\\\\n\\begin{aligned}c&=d\n\\end{aligned}",
            ),
            (
                r"\begin{aligned}\color{red}a&|=b\end{aligned}",
                &["bs"],
                r"{\color{red}a|=b}",
            ),
            (
                "\\begin{aligned}a% & comment\n&|=b\\end{aligned}",
                &["bs"],
                "a|% & comment\n=b",
            ),
        ],
        true,
    );
}

#[test]
fn unrelated_edits_and_other_environments_keep_their_source() {
    check_all(
        &[
            (
                r"\begin{aligned}a|=b\end{aligned}",
                &["t:x"],
                r"\begin{aligned}ax|=b\end{aligned}",
            ),
            (
                r"\begin{aligned}\text{a\&|b}\end{aligned}",
                &["bs"],
                r"\begin{aligned}\text{a|b}\end{aligned}",
            ),
            (
                r"\begin{cases}a&|b\end{cases}",
                &["bs"],
                r"\begin{cases}a|&b\end{cases}",
            ),
            (
                r"\begin{matrix}a&|b\end{matrix}",
                &["bs"],
                r"\begin{matrix}a|&b\end{matrix}",
            ),
        ],
        true,
    );
}

#[test]
fn empty_middle_rows_and_replaced_points_are_normalized() {
    check_all(
        &[(
            r"\begin{aligned}a&=b\\&|\\c&=d\end{aligned}",
            &["bs"],
            r"\begin{aligned}a&=b\end{aligned}\\|\\\begin{aligned}c&=d\end{aligned}",
        )],
        true,
    );
    let f = field(r"\begin{aligned}‹a&=b\\c=›d\end{aligned}", true);
    let next = f.run(&Command::Paste(r"a=b\\c&=".to_owned()));
    assert_eq!(
        next.field.source(),
        r"a=b\\\begin{aligned}c&=d\end{aligned}"
    );
}

#[test]
fn an_ampersand_starts_and_restores_display_alignment() {
    check_all(
        &[
            ("a|", &["t:&=b"], r"\begin{aligned}a&=b|\end{aligned}"),
            ("a|=b", &["t:&"], r"\begin{aligned}a&|=b\end{aligned}"),
            ("|", &["t:&="], r"\begin{aligned}&=|\end{aligned}"),
            (
                r"a=b\\c|=d\\e=f",
                &["t:&"],
                r"a=b\\\begin{aligned}c&|=d\end{aligned}\\e=f",
            ),
            (
                r"\begin{aligned}a&|=b\end{aligned}",
                &["bs", "t:&"],
                r"\begin{aligned}a&|=b\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\\c&|=d\\e&=f\end{aligned}",
                &["bs", "t:&"],
                r"\begin{aligned}a&=b\\c&|=d\\e&=f\end{aligned}",
            ),
        ],
        true,
    );
}

#[test]
fn adding_alignment_keeps_literal_and_nested_contexts_distinct() {
    check_all(
        &[
            ("a|", &["t:&="], r"a\&=|"),
            (r"\text{a|}", &["t:&="], r"\text{a\&=|}"),
            (r"\frac{a|}{b}", &["t:&="], r"\frac{a\&=|}{b}"),
        ],
        false,
    );
    check_all(
        &[
            (r"\text{a|}", &["t:&="], r"\text{a\&=|}"),
            (r"\frac{a|}{b}", &["t:&="], r"\frac{a\&=|}{b}"),
            ("a|", &[r"t:\&"], r"a\&|"),
            (
                r"\frac{\begin{gathered}a|=b\\c=d\end{gathered}}{x}",
                &["t:&"],
                r"\frac{\begin{gathered}\begin{aligned}a&|=b\end{aligned}\\c=d\end{gathered}}{x}",
            ),
            (
                r"\begin{aligned}\color{red}a&=b\end{aligned}\\c|=d",
                &["t:&"],
                r"\begin{aligned}\color{red}a&=b\end{aligned}\\\begin{aligned}c&|=d\end{aligned}",
            ),
            (
                r"\begin{aligned}a&=b\end{aligned}\\\\c|=d",
                &["t:&"],
                r"\begin{aligned}a&=b\end{aligned}\\\\\begin{aligned}c&|=d\end{aligned}",
            ),
        ],
        true,
    );
}
