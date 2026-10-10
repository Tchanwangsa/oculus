//! The wall clock and the calendar, without a date crate.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

pub fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default()
}

pub use keyd_core::clock::civil_from_days;

/// `YYYY-MM-DD` for a day number since the epoch.
pub fn ymd(days: i64) -> String {
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Today in UTC, as `YYYY-MM-DD`.
pub fn today_utc() -> String {
    ymd((now_secs() as i64).div_euclid(86_400))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_map_to_the_right_calendar_date() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        assert_eq!(civil_from_days(19_617), (2023, 9, 17));
        assert_eq!(ymd(11_016), "2000-02-29");
    }
}
