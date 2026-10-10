//! Chronological ordering for Canvas terms. Term names do not sort as text
//! (`"Summer"` > `"Semester"`) and mean different months in different places —
//! Summer opens the year in Australia and sits mid-year in the US — so each
//! term gets a `YYYY-MM` key: from Canvas's `term.start_at` when it has one,
//! else read from the name. Stored as `subjects.term_sort`, which the frontend
//! and the CLI sort by; neither ranks names itself.

/// Named terms by the month they open in, tested in order: semesters before
/// seasons and months, so `"Semester 2 (July start)"` stays a semester.
const NUMBERED: &[(&str, u8)] = &[
    ("semester 1", 3),
    ("semester 2", 7),
    ("trimester 1", 2),
    ("trimester 2", 6),
    ("trimester 3", 10),
    ("term 1", 2),
    ("term 2", 5),
    ("term 3", 8),
    ("term 4", 10),
    ("quarter 1", 1),
    ("quarter 2", 4),
    ("quarter 3", 7),
    ("quarter 4", 10),
];

const SOUTHERN_SEASONS: &[(&str, u8)] =
    &[("summer", 1), ("autumn", 3), ("winter", 6), ("spring", 9)];

const NORTHERN_SEASONS: &[(&str, u8)] = &[
    ("winter", 1),
    ("spring", 3),
    ("summer", 6),
    ("fall", 9),
    ("autumn", 9),
];

const MONTHS: &[&str] = &[
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// Past every real month, so an unrecognised term sorts after the real ones
/// of its year.
const UNKNOWN_MONTH: u8 = 13;

/// The month a term opens in, from its name. A month-named intensive runs
/// inside a southern term, so June and July rank with Winter there.
fn month_of(name: &str, southern: bool) -> Option<u8> {
    let lower = name.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if let Some((_, m)) = NUMBERED.iter().find(|(n, _)| lower.contains(n)) {
        return Some(*m);
    }
    let seasons = if southern {
        SOUTHERN_SEASONS
    } else {
        NORTHERN_SEASONS
    };
    if let Some((_, m)) = seasons.iter().find(|(s, _)| words.contains(s)) {
        return Some(*m);
    }
    let month = MONTHS.iter().position(|m| words.contains(m))? as u8 + 1;
    Some(if southern && month == 7 { 6 } else { month })
}

/// The first four-digit 19xx or 20xx in a term name.
fn year_of(name: &str) -> Option<u16> {
    name.split(|c: char| !c.is_ascii_digit())
        .filter(|run| run.len() == 4)
        .filter_map(|run| run.parse::<u16>().ok())
        .find(|y| (1900..2100).contains(y))
}

/// `YYYY-MM` from an ISO `start_at`, if it starts with a date.
fn month_of_date(start_at: &str) -> Option<String> {
    let ym = start_at.get(..7)?;
    let ok = ym.as_bytes()[4] == b'-'
        && ym[..4].bytes().all(|b| b.is_ascii_digit())
        && ym[5..].bytes().all(|b| b.is_ascii_digit());
    ok.then(|| ym.to_string())
}

/// The term's sort key, `YYYY-MM`; later terms sort higher. `None` when
/// neither the start date nor the name gives a year.
pub fn term_sort(name: &str, start_at: Option<&str>, southern: bool) -> Option<String> {
    if let Some(ym) = start_at.and_then(month_of_date) {
        return Some(ym);
    }
    let year = year_of(name)?;
    let month = month_of(name, southern).unwrap_or(UNKNOWN_MONTH);
    Some(format!("{year}-{month:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UniMelb's names, read as a southern university reads them.
    fn key(name: &str) -> Option<String> {
        term_sort(name, None, true)
    }

    #[test]
    fn summer_opens_the_year_it_names() {
        assert!("2026 Summer Term" > "2026 Semester 2");
        assert!(key("2026 Summer Term") < key("2026 Semester 2"));
        assert!(key("2026 Summer Term") < key("2026 Semester 1"));
    }

    #[test]
    fn terms_run_in_teaching_order() {
        let mut names = vec![
            "2026 Semester 2",
            "2026 Summer Term",
            "2026 Winter Term",
            "2026 Semester 1",
        ];
        names.sort_by_key(|n| key(n));
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
        assert!(key("2026 Summer Term") > key("2025 Semester 2"));
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
        let latest = terms.iter().max_by_key(|t| key(t)).unwrap();
        assert_eq!(*latest, "2026 Semester 2");
    }

    #[test]
    fn a_month_named_intensive_ranks_as_the_term_it_runs_in() {
        assert_eq!(key("2026 June"), key("2026 Winter Term"));
        assert!(key("2026 June") < key("2026 Semester 2"));
        assert!(key("2026 June") > key("2026 Semester 1"));
        assert_eq!(key("2026 January"), key("2026 Summer Term"));
        assert_eq!(key("2026 Semester 2 (July start)"), key("2026 Semester 2"));
    }

    #[test]
    fn an_unknown_term_does_not_impersonate_a_real_one() {
        assert_eq!(key("2026 Intensive Block").as_deref(), Some("2026-13"));
        assert!(key("2026 Intensive Block") > key("2026 Semester 2"));
        assert_eq!(key("Default Term"), None);
    }

    #[test]
    fn a_northern_year_runs_spring_summer_fall() {
        let north = |n: &str| term_sort(n, None, false);
        let mut names = vec!["Fall 2026", "Summer 2026", "Spring 2027", "Spring 2026"];
        names.sort_by_key(|n| north(n));
        assert_eq!(
            names,
            ["Spring 2026", "Summer 2026", "Fall 2026", "Spring 2027"]
        );
        // The year need not lead the name.
        assert_eq!(north("Fall 2026").as_deref(), Some("2026-09"));
        assert_eq!(north("2026FA Term 3").as_deref(), Some("2026-08"));
    }

    #[test]
    fn a_start_date_beats_the_name() {
        assert_eq!(
            term_sort("2026 Summer Term", Some("2026-11-30T13:00:00Z"), true).as_deref(),
            Some("2026-11")
        );
        // A malformed date falls back to the name.
        assert_eq!(
            term_sort("2026 Semester 1", Some("soon"), true).as_deref(),
            Some("2026-03")
        );
    }
}
