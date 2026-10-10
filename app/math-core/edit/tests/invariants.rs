//! The stop invariants (`oculus_math_edit::check`) over every formula the
//! fork's source-location tests use, the oracle's synthetic fixtures,
//! every parseable prefix of those (what typing passes through), and
//! random token soup.
#![allow(
    clippy::non_ascii_literal,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use oculus_math_edit::check::{Report, check};
use proptest::{collection::vec, prelude::*, sample::select};

use common::corpus;

fn assert_holds(source: &str, display: bool) -> Option<Report> {
    let report = check(source, display).ok()?;
    assert!(
        report.failures.is_empty(),
        "{source:?} (display {display}): {:?}",
        report.failures
    );
    Some(report)
}

#[test]
fn corpus_formulas_keep_the_invariants() {
    let corpus = corpus();
    let parsed = corpus
        .iter()
        .filter_map(|(source, display)| assert_holds(source, *display))
        .count();
    // The AMS environments and `\tag` parse in display only.
    assert!(
        parsed * 10 >= corpus.len() * 9,
        "{parsed} of {}",
        corpus.len()
    );
}

#[test]
fn every_parseable_prefix_keeps_the_invariants() {
    for (source, display) in corpus() {
        for (end, _) in source.char_indices().skip(1) {
            assert_holds(&source[..end], display);
        }
    }
}

/// Pieces a random formula is glued from: atoms, structures with their
/// braces, scripts, and the delimiters that only parse when balanced.
/// Macros that take an argument come closed: the fork accepts some
/// unclosed ones after `\dots` (`\dots\dots\bra{`), where KaTeX JS
/// rejects them, and that source is not the field's to fix.
const PIECES: &[&str] = &[
    "a",
    "1",
    "+",
    "=",
    " ",
    "'",
    r"\alpha",
    r"\alpha ",
    r"\,",
    r"\dots",
    r"\iff",
    "{",
    "}",
    "{}",
    "^",
    "_",
    "^{",
    "_{",
    r"\frac",
    r"\frac{",
    "}{",
    r"\sqrt",
    r"\sqrt[",
    "]",
    r"\hat",
    r"\text{",
    "ไ",
    "ี",
    "x y",
    r"\left(",
    r"\right)",
    r"\over",
    r"\color{red}",
    r"\begin{matrix}",
    r"\end{matrix}",
    "&",
    r"\\",
    r"\mathbf",
    r"\operatorname{",
    "$",
    r"\bra{x}",
    r"\boxed{a}",
    r"\overset{",
    r"\tag{1}",
];

/// Well-formed formulas: atoms, and structures nested in each other.
fn formula() -> impl Strategy<Value = String> {
    let atom = select(
        &[
            // Control words keep a space: concatenation would extend them.
            "a",
            "12",
            "+",
            "=",
            "x'",
            "x²",
            r"\alpha ",
            r"\,",
            r"\dots ",
            r"\iff ",
            r"\sin ",
            "{}",
            r"\text{a b}",
            r"\text{สวัสดี}",
            r"\text{}",
            r"\lim\limits ",
            r"\not=",
        ][..],
    )
    .prop_map(str::to_owned);
    atom.prop_recursive(4, 48, 3, |inner| {
        let two = (inner.clone(), inner.clone());
        prop_oneof![
            vec(inner.clone(), 1..4).prop_map(|parts| parts.concat()),
            inner.clone().prop_map(|a| format!("{{{a}}}")),
            two.clone()
                .prop_map(|(a, b)| format!(r"\frac{{{a}}}{{{b}}}")),
            two.clone().prop_map(|(a, b)| format!("{{{a}}}^{{{b}}}")),
            two.clone().prop_map(|(a, b)| format!("{{{a}}}_{{{b}}}")),
            two.clone().prop_map(|(a, b)| format!(r"{{{a}\over {b}}}")),
            // KaTeX reads an index that starts with a group as that group,
            // and the first `]` ends it unless braces hide it.
            two.clone()
                .prop_map(|(a, b)| format!(r"\sqrt[x{{{a}}}]{{{b}}}")),
            inner.clone().prop_map(|a| format!(r"\sqrt{{{a}}}")),
            inner.clone().prop_map(|a| format!(r"\hat{{{a}}}")),
            inner.clone().prop_map(|a| format!(r"\left({a}\right)")),
            inner.clone().prop_map(|a| format!(r"\text{{a ${a}$}}")),
            inner.clone().prop_map(|a| format!(r"\color{{red}}{a}")),
            inner.clone().prop_map(|a| format!(r"\boxed{{{a}}}")),
            inner.prop_map(|a| format!(r"\operatorname{{{a}}}")),
            two.clone()
                .prop_map(|(a, b)| format!(r"\overset{{{a}}}{{{b}}}")),
            two.prop_map(|(a, b)| format!(r"\begin{{pmatrix}}{a}&{b}\\&\end{{pmatrix}}")),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 4000,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn random_formulas_keep_the_invariants(
        pieces in vec(select(PIECES), 1..14),
        display: bool,
    ) {
        let source = pieces.concat();
        if let Ok(report) = check(&source, display) {
            prop_assert!(report.failures.is_empty(), "{source:?}: {:?}", report.failures);
        }
    }

    #[test]
    fn nested_structures_keep_the_invariants(source in formula(), display: bool) {
        let report = check(&source, display);
        prop_assert!(report.is_ok(), "{source:?} does not render: {report:?}");
        if let Ok(report) = report {
            prop_assert!(report.failures.is_empty(), "{source:?}: {:?}", report.failures);
        }
    }
}
