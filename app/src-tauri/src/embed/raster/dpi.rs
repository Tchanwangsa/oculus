//! How many pixels a page gets, within the backend's per-image ceiling.

/// Pixels for a page this many points wide at `dpi`, bit-exact with MuPDF's
/// `fz_round_rect` (`ceil(x - 0.001)`; the epsilon stops an integral edge
/// gaining a blank column) so page geometry matches the existing renders.
pub(super) fn pixels_for(points: f32, dpi: u32) -> u32 {
    let scaled = points * dpi as f32 / 72.0;
    ((scaled - 0.001).ceil() as i64).max(1) as u32
}

/// The DPI one page renders at: [`RENDER_DPI`], or less when its pixels would
/// exceed the backend's per-image ceiling (a poster page in a deck), which
/// would otherwise fail the whole document. Costs nothing: the backend
/// downscales far below that ceiling before it bills. Rounded down and then
/// verified, because [`pixels_for`] rounds up.
pub(super) fn dpi_for_page(
    width_pt: f32,
    height_pt: f32,
    dpi: u32,
    max_pixels: Option<u64>,
) -> u32 {
    let Some(max_pixels) = max_pixels.filter(|max| *max > 0) else {
        return dpi;
    };
    let pixels =
        |at: u32| u64::from(pixels_for(width_pt, at)) * u64::from(pixels_for(height_pt, at));
    if pixels(dpi) <= max_pixels {
        return dpi;
    }
    let scale = (max_pixels as f64 / pixels(dpi) as f64).sqrt();
    let mut at = ((dpi as f64 * scale).floor() as u32).max(1);
    while at > 1 && pixels(at) > max_pixels {
        at -= 1;
    }
    at
}
