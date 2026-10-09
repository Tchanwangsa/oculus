//! Chronological ordering for Canvas term names, which do not sort as text
//! (`"Summer"` > `"Semester"`, yet Summer opens the year). Month-named
//! intensives (`"2026 June"`) rank as the term they fall inside.
//!
//! Mirrored in `app/src/lib/format/terms.ts` — both must agree, since the flag stored
//! from here is what the CLI and the agent read back.

/// Position within one academic year: Summer (Jan–Feb), Semester 1, Winter,
/// Semester 2. Semesters are tested before months, so `"Semester 2 (July)"`
/// stays a semester. Unrecognised terms sort after every real one.
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
        assert!("2026 Summer Term" > "2026 Semester 2");
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
        assert_eq!(term_rank("2026 June"), term_rank("2026 Winter Term"));
        assert!(term_key("2026 June") < term_key("2026 Semester 2"));
        assert!(term_key("2026 June") > term_key("2026 Semester 1"));
        assert_eq!(term_rank("2026 January"), term_rank("2026 Summer Term"));
        assert_eq!(term_rank("2026 Semester 2 (July start)"), 3);
    }

    #[test]
    fn an_unknown_term_does_not_impersonate_a_real_one() {
        assert_eq!(term_rank("2026 Intensive Block"), 9);
        assert_eq!(term_year("Default Term"), 0);
    }
}
