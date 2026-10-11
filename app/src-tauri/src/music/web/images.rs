//! Which of a Web API object's images a list row shows.

use serde::Deserialize;

/// The smallest image at least this wide is chosen: rows draw covers at
/// about 40 px, twice that on a Retina screen.
const MIN_WIDTH: u32 = 120;

#[derive(Debug, Deserialize)]
pub(super) struct Image {
    url: Option<String>,
    width: Option<u32>,
}

/// The URL as Spotify gives it (`i.scdn.co`, or `mosaic.scdn.co` for a
/// playlist's four-cover grid, which has no file id to rebuild from); only
/// https is accepted. Without widths, the first, which Spotify lists largest.
pub(super) fn pick(images: Option<&[Image]>) -> Option<String> {
    let usable: Vec<&Image> = images?
        .iter()
        .filter(|i| i.url.as_deref().is_some_and(|u| u.starts_with("https://")))
        .collect();
    let chosen = usable
        .iter()
        .filter(|i| i.width.is_some_and(|w| w >= MIN_WIDTH))
        .min_by_key(|i| i.width)
        .or(usable.first())?;
    chosen.url.clone()
}
