//! The documented ceilings, what a page costs against them, and how pages pack into requests.

use crate::embed::raster::RenderedPage;
use crate::embed::EmbedError;

/// Inputs per request. Binds only for small pages.
pub const MAX_INPUTS_PER_REQUEST: usize = 1_000;

/// Tokens per request — the API's, not the account's (see
/// `RequestRun::max_tokens`).
pub const MAX_TOKENS_PER_REQUEST: u64 = 320_000;

/// Tokens in one input. A page cannot be split, so past this is a document
/// error.
pub const MAX_TOKENS_PER_INPUT: u64 = 32_000;

/// Pixels in one image, the API's limit; `refuse_oversized` enforces it.
pub const MAX_PIXELS_PER_IMAGE: u64 = 16_000_000;

/// Bytes in one image.
pub const MAX_BYTES_PER_IMAGE: u64 = 20 * 1024 * 1024;

/// Images bill one token per this many pixels.
pub const PIXELS_PER_TOKEN: u64 = 560;

/// Voyage downscales an image to this many pixels before it bills (observed in
/// its `usage.image_pixels`). A full-DPI slide is over it, so every such page
/// costs `tokens_for` the cap; an uncapped estimate would be ~2x wrong. This
/// affects only the estimate — rendering still happens at `RENDER_DPI`.
pub const BILLED_PIXEL_CAP: u64 = 2_000_000;

/// The most pixels a page is rendered at; past it the page drops DPI. Pixels
/// over [`BILLED_PIXEL_CAP`] are downscaled away, and a 16M-pixel layered page
/// can take ~1 GB to render; every standard page (16:9 at 4.0M) stays at 200 DPI.
pub const MAX_RENDER_PIXELS: u64 = 4_200_000;

/// What one page costs after the downscale, rounded up: the ledger reserves
/// from this and must never come in under what is billed.
pub fn tokens_for(width: u32, height: u32) -> u64 {
    let pixels = u64::from(width) * u64::from(height);
    pixels.min(BILLED_PIXEL_CAP).div_ceil(PIXELS_PER_TOKEN)
}

/// The pixels on the page, for the hard API limits; billing is [`billed_pixels`].
pub fn raw_pixels(page: &RenderedPage) -> u64 {
    u64::from(page.width) * u64::from(page.height)
}

/// The pixels Voyage will charge for, which is what the ledger counts.
pub fn billed_pixels(page: &RenderedPage) -> u64 {
    raw_pixels(page).min(BILLED_PIXEL_CAP)
}

/// Why this one page cannot be sent, if it cannot — a document failure, never
/// a skip (that would be a short record). The rasterizer's DPI clamp usually
/// keeps pixels under the limit; this catches what it cannot, plus bytes.
pub fn refuse_oversized(page: &RenderedPage) -> Result<u64, EmbedError> {
    let tokens = tokens_for(page.width, page.height);
    // The token check cannot fire while the billing cap holds; it enforces the
    // documented limit in case Voyage stops downscaling.
    let code = if raw_pixels(page) > MAX_PIXELS_PER_IMAGE {
        "page-too-many-pixels"
    } else if page.png.len() as u64 > MAX_BYTES_PER_IMAGE {
        "page-too-large"
    } else if tokens > MAX_TOKENS_PER_INPUT {
        "page-too-many-tokens"
    } else {
        return Ok(tokens);
    };
    Err(EmbedError::Document {
        code: format!("{code}-p{}", page.page_no),
    })
}

/// The packing rule, also used by `estimate`: given each page's token cost in
/// order, which pages travel together? Greedy and order-preserving, so
/// `page_no` stays stable from render to record.
pub fn plan(costs: &[u64], max_inputs: usize, max_tokens: u64) -> Vec<Vec<usize>> {
    let max_inputs = max_inputs.max(1);
    let mut requests: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut spent: u64 = 0;

    for (index, cost) in costs.iter().enumerate() {
        let full = current.len() >= max_inputs || spent + cost > max_tokens;
        if full && !current.is_empty() {
            requests.push(std::mem::take(&mut current));
            spent = 0;
        }
        current.push(index);
        spent += cost;
    }
    if !current.is_empty() {
        requests.push(current);
    }
    requests
}
