//! Subject and category filtering shared by the read commands and indexing.

use crate::*;

/// `MULT20015` matches `MULT20015_2026_SM2`.
pub(crate) fn matches_code(code: &str, wanted: &str) -> bool {
    let (code, wanted) = (code.to_uppercase(), wanted.to_uppercase());
    code == wanted || code.starts_with(&format!("{wanted}_"))
}

/// Narrow a file list to the categories asked for, and refuse a word that is
/// not a category: a silent empty result from a typo reads as "the library does
/// not cover that". Validity is `paths::CATEGORIES`, not the rows' own
/// categories, so a subject with no quizzes answers `--category quiz` with
/// nothing rather than an error. Shared by `grep` and `files`.
pub(crate) fn filter_categories(
    files: Vec<LibFile>,
    wanted: &[String],
) -> Result<Vec<LibFile>, String> {
    if wanted.is_empty() {
        return Ok(files);
    }
    for c in wanted {
        if !paths::CATEGORIES.iter().any(|k| k.eq_ignore_ascii_case(c)) {
            return Err(format!(
                "no category {c:?} — the categories are: {}",
                paths::CATEGORIES.join(", ")
            ));
        }
    }
    Ok(files
        .into_iter()
        .filter(|f| {
            f.category
                .as_deref()
                .is_some_and(|k| wanted.iter().any(|c| k.eq_ignore_ascii_case(c)))
        })
        .collect())
}

/// The `--category` help, built from the list the flag validates against.
pub(crate) fn category_help() -> String {
    format!(
        "Only these categories ({}). Repeatable",
        paths::CATEGORIES.join(", ")
    )
}

pub(crate) fn filter_subjects(
    subjects: &[store::SubjectRow],
    codes: &[String],
    current_only_when_empty: bool,
) -> Result<Vec<store::SubjectRow>, String> {
    let picked: Vec<store::SubjectRow> = subjects
        .iter()
        .filter(|s| {
            if codes.is_empty() {
                !current_only_when_empty || s.is_current
            } else {
                codes.iter().any(|w| matches_code(&s.code, w))
            }
        })
        .cloned()
        .collect();

    if picked.is_empty() && !codes.is_empty() {
        return Err(format!("no subject matched {}", codes.join(", ")));
    }
    Ok(picked)
}

/// An empty set means all subjects to query loaders; a bare code includes every term.
pub(crate) fn subject_ids(
    subjects: &[store::SubjectRow],
    codes: &[String],
) -> Result<Vec<i64>, String> {
    if codes.is_empty() {
        return Ok(Vec::new());
    }
    Ok(filter_subjects(subjects, codes, false)?
        .iter()
        .map(|s| s.id)
        .collect())
}
