//! Spotify's tracks and Oculus's own made one list. Spotify dates its
//! entries only to the day (UTC is assumed) while Oculus's carry the
//! millisecond, so the two meet by day: newest day first, and within a day
//! Oculus's tracks Spotify does not list (newest first), then Spotify's in
//! its order, then any track Spotify lists on an older day that Oculus
//! played on this one. Each URI is shown once, at its first place.

use std::cmp::Reverse;
use std::collections::HashSet;

use crate::music::recent_tracks::{Played, CAP};

const DAY_MS: u64 = 86_400_000;

/// A track of Spotify's list and the day it was played on, in days since
/// the Unix epoch; `i64::MIN` when the list dated nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::music::library) struct Dated {
    pub(super) uri: String,
    pub(super) day: i64,
}

/// Spotify's tracks in order, each with a day: an undated one takes the
/// day of the one before it (the list is newest first), the undated ones
/// leading the list the first dated one's, and all of them `i64::MIN` when
/// none is dated.
pub(super) fn dated(tracks: Vec<(String, Option<i64>)>) -> Vec<Dated> {
    let mut last = tracks.iter().find_map(|(_, day)| *day).unwrap_or(i64::MIN);
    tracks
        .into_iter()
        .map(|(uri, day)| {
            last = day.unwrap_or(last);
            Dated { uri, day: last }
        })
        .collect()
}

/// Both lists as one, up to `CAP`: Spotify's entries at their day's UTC
/// midnight (0 when undated), Oculus's at their own time.
pub(super) fn merge(spotify: &[Dated], local: &[Played]) -> Vec<Played> {
    let listed: HashSet<&str> = spotify.iter().map(|d| d.uri.as_str()).collect();
    let mut local: Vec<&Played> = local.iter().collect();
    local.sort_by_key(|p| Reverse(p.played_at_ms));
    // (day, group within the day, rank within the group, the entry).
    let mut placed: Vec<(i64, u8, usize, Played)> = Vec::new();
    for (rank, p) in local.into_iter().enumerate() {
        let day = i64::try_from(p.played_at_ms / DAY_MS).unwrap_or(i64::MAX);
        let group = if listed.contains(p.uri.as_str()) {
            2
        } else {
            0
        };
        placed.push((day, group, rank, p.clone()));
    }
    for (rank, d) in spotify.iter().enumerate() {
        let played_at_ms = u64::try_from(d.day)
            .ok()
            .and_then(|day| day.checked_mul(DAY_MS))
            .unwrap_or(0);
        placed.push((
            d.day,
            1,
            rank,
            Played {
                uri: d.uri.clone(),
                played_at_ms,
            },
        ));
    }
    placed.sort_by_key(|(day, group, rank, _)| (Reverse(*day), *group, *rank));
    let mut seen = HashSet::new();
    placed
        .into_iter()
        .map(|(_, _, _, p)| p)
        .filter(|p| seen.insert(p.uri.clone()))
        .take(CAP)
        .collect()
}
