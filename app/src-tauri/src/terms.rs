//! Chronological ordering for Canvas term names.
//!
//! Canvas names a term `"2026 Semester 2"` or `"2026 Summer Term"`, and those
//! strings do *not* sort chronologically: `"Summer"` beats `"Semester"` on the
//! third character (`u` > `e`), so comparing them as text files Summer last in
//! a year it actually starts. Taking the plain `.max()` of the names therefore
//! elected a single summer enrolment over the semester being studied, and
//! every real subject was stamped as past.
//!
//! Not every term is named after a semester either. A short intensive comes
//! back as the month it runs in — `"2026 June"` for `THTR30042_2026_JUN_STH_2`
//! — so the months map onto the term they fall inside: January/February are
//! Summer, June/July are Winter. Left unranked they sorted after every real
//! term, and the same `.max()` elected a one-off intensive over the semester
//! being studied.
//!
//! The year prefix compares fine either way; only the term within a year needs
//! a rank. Mirrored in `app/src/lib/terms.ts`, which the frontend uses to
//! derive the same answer at read time — both sides have to agree, because a
//! stored flag written here is what the CLI and the agent read back.

/// Chronological position within one academic year. UniMelb runs Summer Term
/// in January–February, ahead of Semester 1, with Winter Term between the two
/// semesters. A term named after its month takes the rank of the term it falls
/// inside; the semesters are tested first, so a `"Semester 2 (July)"` is still
/// a semester. An unrecognised term sorts after every real one rather than
/// silently taking a real term's place.
pub fn term_rank(name: &str) -> u8 {
    let lower = name.to_lowercase();
    if lower.contains("summer") {
        0
    } else if lower.contains("semester 1") {
        1
    } else if lower.contains("winter") {
        2
    } else if lower.contains("semester 2") {
        3
    } else if lower.contains("january") || lower.contains("february") {
        0
    } else if lower.contains("june") || lower.contains("july") {
        2
    } else {
        9
    }
}

/// The year a term name opens with; a missing or malformed one sorts oldest.
pub fn term_year(name: &str) -> i32 {
    name.get(..4).and_then(|y| y.parse().ok()).unwrap_or(0)
}

/// Sort key putting the most recent term last, for `max_by_key`.
pub fn term_key(name: &str) -> (i32, u8) {
    (term_year(name), term_rank(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summer_opens_the_year_it_names() {
        // The bug in one line: as text, Summer wins its year.
        assert!("2026 Summer Term" > "2026 Semester 2");
        // Ranked, it does not.
        assert!(term_key("2026 Summer Term") < term_key("2026 Semester 2"));
        assert!(term_key("2026 Summer Term") < term_key("2026 Semester 1"));
    }

    #[test]
    fn terms_run_in_teaching_order() {
        let mut names = vec![
            "2026 Semester 2",
            "2026 Summer Term",
            "2026 Winter Term",
            "2026 Semester 1",
        ];
        names.sort_by_key(|n| term_key(n));
        assert_eq!(
            names,
            vec![
                "2026 Summer Term",
                "2026 Semester 1",
                "2026 Winter Term",
                "2026 Semester 2",
            ]
        );
    }

    #[test]
    fn a_later_year_always_wins() {
        assert!(term_key("2026 Summer Term") > term_key("2025 Semester 2"));
    }

    #[test]
    fn the_newest_of_a_real_enrolment_is_the_semester_being_studied() {
        let terms = [
            "2024 Semester 2",
            "2025 Semester 1",
            "2026 Semester 1",
            "2026 Semester 2",
            "2026 Summer Term",
        ];
        let latest = terms.iter().max_by_key(|t| term_key(t)).unwrap();
        assert_eq!(*latest, "2026 Semester 2");
    }

    #[test]
    fn a_month_named_intensive_ranks_as_the_term_it_runs_in() {
        // The June intensive is a winter term, not an unknown one — otherwise
        // it outranks Semester 2 and steals "current" from the real enrolment.
        assert_eq!(term_rank("2026 June"), term_rank("2026 Winter Term"));
        assert!(term_key("2026 June") < term_key("2026 Semester 2"));
        assert!(term_key("2026 June") > term_key("2026 Semester 1"));
        assert_eq!(term_rank("2026 January"), term_rank("2026 Summer Term"));
        // A semester wins its own name back from the month in the brackets.
        assert_eq!(term_rank("2026 Semester 2 (July start)"), 3);
    }

    #[test]
    fn an_unknown_term_does_not_impersonate_a_real_one() {
        assert_eq!(term_rank("2026 Intensive Block"), 9);
        assert_eq!(term_year("Default Term"), 0);
    }
}
