//! A cover for a playlist Spotify gives no picture, made the way its own
//! apps make one: from the album covers of its first tracks, four distinct
//! ones as a 2×2 grid, else the first track's alone. The tracks come from
//! `contents.rs`, their albums from `metadata.rs`.

use std::collections::{HashMap, HashSet};

use super::metadata::TrackRow;

/// Tracks read per playlist: enough to find four distinct albums in all but
/// a playlist of a few albums' tracks, which then shows the first alone.
pub(super) const TRACKS: usize = 20;
/// Covers in a grid.
const GRID: usize = 4;

/// A playlist's cover from its tracks in order: four URLs when they reach
/// four distinct albums, else the first track's album cover alone, else
/// none. A track with no row or no cover is passed over.
pub(super) fn pick(tracks: &[String], rows: &HashMap<String, TrackRow>) -> Vec<String> {
    let mut seen = HashSet::new();
    let distinct: Vec<&str> = tracks
        .iter()
        .filter_map(|uri| rows.get(uri))
        .filter_map(|row| Some((row.album_key.as_str(), row.image_url.as_deref()?)))
        .filter(|(album, _)| seen.insert(*album))
        .map(|(_, url)| url)
        .take(GRID)
        .collect();
    if distinct.len() == GRID {
        distinct.iter().map(|url| url.to_string()).collect()
    } else {
        distinct
            .first()
            .map(|url| url.to_string())
            .into_iter()
            .collect()
    }
}
