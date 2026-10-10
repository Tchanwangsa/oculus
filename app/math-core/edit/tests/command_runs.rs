//! The command invariants (`oculus_math_edit::check::commands`) over
//! random command sequences on the corpus formulas: every command keeps
//! the source rendering, the stops valid, the selection on them and its
//! change exact, and Backspace undoes a typed character or template.
#![allow(
    clippy::non_ascii_literal,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

mod common;

use std::sync::LazyLock;

use oculus_math_edit::check::commands::{Report, picks, run, seed};
use proptest::{collection::vec, prelude::*};

static CORPUS: LazyLock<Vec<(String, bool)>> = LazyLock::new(common::corpus);

fn failures(source: &str, display: bool, report: &Report) -> String {
    format!(
        "{source:?} (display {display}): {:?}",
        report
            .failures
            .iter()
            .map(|f| (f.step(), f.kind()))
            .collect::<Vec<_>>()
    )
}

/// Each formula, and the empty field, with its own fixed run.
#[test]
fn a_fixed_run_on_every_formula_keeps_the_invariants() {
    let mut restores = 0;
    let empty = [(String::new(), false), (String::new(), true)];
    for (source, display) in CORPUS.iter().chain(&empty) {
        let Ok(report) = run(source, *display, picks(seed(source), 60)) else {
            continue;
        };
        assert!(
            report.failures.is_empty(),
            "{}",
            failures(source, *display, &report)
        );
        restores += report.restores;
    }
    assert!(restores > 500, "only {restores} insertions were undone");
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 10_000,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn random_commands_keep_the_invariants(
        formula in 0..CORPUS.len(),
        picks in vec(any::<u64>(), 1..16),
    ) {
        let (source, display) = &CORPUS[formula];
        if let Ok(report) = run(source, *display, picks) {
            prop_assert!(report.failures.is_empty(), "{}", failures(source, *display, &report));
        }
    }
}
