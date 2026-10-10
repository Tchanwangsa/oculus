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

use oculus_math_edit::check::{Report, check};
use proptest::{collection::vec, prelude::*, sample::select};

/// `katex/tests/source_locs.rs`'s formulas: one of each construct the
/// source mapping knows.
const FORMULAS: &[&str] = &[
    "ab",
    r"\frac{a}{b}",
    r"\frac ab",
    r"a\over b",
    r"{\over b}",
    r"{a\over}",
    r"a\above 2pt b",
    r"{n\choose k}",
    "x^2_i",
    "^2",
    "x'",
    "x''^2",
    "x²",
    "x₁₂",
    r"\sqrt[3]{x}",
    r"\sqrt x",
    r"\left(\frac{a}{b}\right)",
    r"\left(a\middle|b\right)",
    r"\dots",
    r"a\iff b",
    r"\neq",
    r"\alpha x",
    r"\def\x{ab}\x",
    r"\def\x#1{#1+#1}\x{y}",
    r"\newcommand{\br}[1]{\langle #1|}\br{x}",
    r"\bra{x}",
    r"\text{สวัสดี x}",
    r"\text{a $x$ b}",
    r"\begin{pmatrix}a&b\\c&\end{pmatrix}",
    r"\begin{pmatrix}&\\&\end{pmatrix}",
    r"\begin{cases}a&b\\c&d\end{cases}",
    r"\begin{aligned}a&=b\\&=c\end{aligned}",
    r"\begin{array}{cc}a&b\end{array}",
    r"\color{red}{x}",
    r"\color{red} x y",
    r"\textcolor{red}{x}",
    r"\mathbf{ab}",
    r"\bf ab",
    r"\displaystyle x",
    r"\large x",
    r"\operatorname{sin}x",
    r"\overset{a}{b}",
    r"\verb|x|",
    r"\tag{1} x",
    "{}",
    r"\frac{}{}",
    "x^{}",
    r"\hat{x}",
    "é",
    r"\sum_{i=1}^n i",
    r"\mathrm{d}x",
    r"\char`a",
    r"\kern1em x",
    r"\rule{1em}{2em}",
    r"\hbox{x}",
    r"\boxed{x}",
    r"\xrightarrow[b]{a}",
    r"\overbrace{x}^{y}",
    r"\mathchoice{a}{b}{c}{d}",
    r"\phantom{x}",
    r"\not=",
    r"\big(",
    r"\cancel{x}",
    r"a\\b",
    r"\begin{CD}A @>f>> B\\@VVV @AAA\\C @= D\end{CD}",
    r"\begin{CD}A @<a<< B @>>b> C\\@| @AcAA @VVdV\\D @= E @>>> F\end{CD}",
    r"\begin{align}a&=b\tag{1}\\c&=d\end{align}",
    r"\begin{gather}a\\b\end{gather}",
    r"\begin{smallmatrix}a&b\end{smallmatrix}",
    r"\begin{array}{|c|}\hline a\\\hline\end{array}",
    r"\begin{matrix}a\\[1em]b\end{matrix}",
    r"\begin{matrix}\end{matrix}",
    r"\begin{rcases}a\end{rcases}",
    r"\begin{gathered}a\end{gathered}",
    r"\begin{bmatrix*}[r]a\end{bmatrix*}",
    r"\begin{equation}a\end{equation}",
    r"\left.\right.",
    r"\sqrt[]{}",
    "a_{}^{}",
    "x^{y^{z}}",
    r"\overline{}",
    r"\text{--- ''}",
    r"\textbf{a}",
    r"\boldsymbol{x}",
    r"\stackrel{a}{=}",
    r"\underbrace{x}_{y}",
    r"\xleftarrow{}",
    r"\pmb{x}",
    r"\raisebox{1em}{x}",
    r"\rlap{x}",
    r"\smash{x}",
    r"\vcenter{x}",
    r"\vphantom{x}",
    r"\mathop{x}",
    r"\binom{a}{b}",
    r"\genfrac(]{0pt}{2}{a}{b}",
    r"\cfrac{a}{b}",
    r"\operatorname*{lim}_x",
    r"\lim\limits_{x}",
    r"\Set{x|y}",
    r"\braket{a|b}",
    r"\colorbox{red}{x}",
    r"\fcolorbox{red}{blue}{x}",
    r"\kern-1em x",
    r"\hspace{1em}x",
    r"\mathring{a}",
    r"\ddots\vdots\cdots\dotsb",
    r"\TextOrMath{a}{b}",
    r"\href{http://a}{x}",
    r"\url{http://a}",
    r"\includegraphics{a.png}",
    r"\htmlClass{a}{x}",
    r"\phase{x}",
    r"\angl{n}",
    r"\sqrt{\smash[b]{y}}",
    r"\begingroup a\endgroup",
    r"\def\x#1.{#1}\x a.",
    r"\let\y=a\y",
    r"\gdef\z{q}\z",
    r"\mathchoice{a}{b}{c}{d}",
];

/// More shapes the stops treat specially.
const EDIT_FORMULAS: &[&str] = &[
    r"\begin{matrix}a&&b\end{matrix}",
    r"\begin{pmatrix}a\\\end{pmatrix}",
    r"\text{ไทย}",
    r"\text{สวัสดี x}",
    "a={}b",
    "{}^{14}C",
    r"\sqrt\frac ab",
    r"x^\frac ab",
    r"\frac\alpha  b",
    r"\textcolor{red}{}",
    r"\operatorname{}",
    r"\left\langle \right\rangle",
    r"\Set{x|y}",
    r"\boxed{\frac{a}{b}}",
    r"a\\b\over c",
    r"x\tag*{a}",
    r"\text{a\ b}",
    r"\mathrm{d}x\,\mathrm{d}y",
    "x_{a_{b_c}}",
    r"\left(\begin{matrix}a\end{matrix}\right)^2",
    r"\displaystyle\sum_{n=1}^\infty \frac1{n^2}",
];

fn fixtures() -> Vec<(String, bool)> {
    let json: serde_json::Value =
        serde_json::from_str(include_str!("../../oracle/fixtures.json")).unwrap();
    json.as_array()
        .unwrap()
        .iter()
        .map(|case| {
            (
                case["tex"].as_str().unwrap().to_owned(),
                case["display"].as_bool().unwrap_or(false),
            )
        })
        .collect()
}

/// Every formula, inline and display.
fn corpus() -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = FORMULAS
        .iter()
        .chain(EDIT_FORMULAS)
        .flat_map(|tex| [((*tex).to_owned(), false), ((*tex).to_owned(), true)])
        .collect();
    out.extend(fixtures());
    out
}

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
