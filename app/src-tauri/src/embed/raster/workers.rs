//! A document's pages on several render threads at once, delivered in order.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::sync::mpsc;
use std::sync::{Condvar, Mutex};

use hayro::hayro_syntax::page::Page;
use hayro::hayro_syntax::Pdf;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};

use super::{page_pixels, RasterError, RenderedPage, RENDER_DPI};
use crate::library::pdf_render::budget::{self, Budget, Lane};
use crate::library::pdf_render::{self, guarded, RenderError};
use crate::providers::ratelimit::hold;

/// Pages rendered ahead of the caller, per worker: enough to keep the workers
/// busy while it packs a request, few enough that waiting PNGs stay small.
const AHEAD_PER_WORKER: u32 = 2;

/// Which page goes next, and how far ahead of the caller the workers are.
struct Order {
    next: u32,
    delivered: u32,
    stop: bool,
}

pub(super) fn render_document<F>(
    document: &Pdf,
    budget: &Budget,
    max_pixels: Option<u64>,
    mut on_page: F,
) -> Result<u32, RasterError>
where
    F: FnMut(RenderedPage) -> Result<(), RasterError>,
{
    let count = document.pages().len() as u32;
    let workers = budget.embed_slots().min(count as usize).max(1);
    let ahead = AHEAD_PER_WORKER * workers as u32;
    let order = Mutex::new(Order {
        next: 1,
        delivered: 0,
        stop: false,
    });
    let moved = Condvar::new();
    let (sender, results) = mpsc::channel::<(u32, Result<RenderedPage, RasterError>)>();

    std::thread::scope(|scope| {
        let mut started = 0;
        let mut spawn_error = None;
        for index in 0..workers {
            let sender = sender.clone();
            let (order, moved) = (&order, &moved);
            let spawned = std::thread::Builder::new()
                .name(format!("pdf-raster-{index}"))
                .stack_size(pdf_render::RENDER_STACK)
                .spawn_scoped(scope, move || loop {
                    let page_no = {
                        let mut state = hold(order);
                        while !state.stop
                            && state.next <= count
                            && state.next > state.delivered + ahead
                        {
                            state = moved.wait(state).unwrap_or_else(|e| e.into_inner());
                        }
                        if state.stop || state.next > count {
                            return;
                        }
                        state.next += 1;
                        state.next - 1
                    };
                    let page = &document.pages()[page_no as usize - 1];
                    let result = render_one(page, page_no, budget, max_pixels);
                    if sender.send((page_no, result)).is_err() {
                        return;
                    }
                });
            match spawned {
                Ok(_) => started += 1,
                Err(error) => spawn_error = Some(error.to_string()),
            }
        }
        drop(sender);

        let stop = |state: &Mutex<Order>| {
            hold(state).stop = true;
            moved.notify_all();
        };
        if started == 0 {
            return Err(RasterError::Worker(spawn_error.unwrap_or_default()));
        }

        // Pages arrive in any order; the caller sees them in page order.
        let mut waiting: BTreeMap<u32, Result<RenderedPage, RasterError>> = BTreeMap::new();
        let mut delivered = 0;
        while delivered < count {
            let Ok((page_no, result)) = results.recv() else {
                // Every worker gone with pages outstanding: only a panic
                // outside `guarded` does that, and the scope re-raises it.
                stop(&order);
                return Err(RasterError::Page {
                    page_no: delivered + 1,
                    message: "the render thread stopped".into(),
                });
            };
            waiting.insert(page_no, result);
            while let Some(result) = waiting.remove(&(delivered + 1)) {
                let outcome = result.and_then(&mut on_page);
                if let Err(error) = outcome {
                    stop(&order);
                    return Err(error);
                }
                delivered += 1;
                hold(&order).delivered = delivered;
                moved.notify_all();
            }
        }
        Ok(count)
    })
}

/// Rasterise one 1-based page into a PNG, under an embedder reservation.
pub(super) fn render_one(
    page: &Page<'_>,
    page_no: u32,
    budget: &Budget,
    max_pixels: Option<u64>,
) -> Result<RenderedPage, RasterError> {
    let failed = |message: String| RasterError::Page { page_no, message };
    let (width, height) = guarded(|| page_pixels(page, RENDER_DPI, max_pixels))
        .map_err(|_| failed("the renderer crashed on its page box".into()))?;
    let _held = budget.reserve(Lane::Embedder, budget::estimate(width, height));

    let rgba = pdf_render::render_rgba(page, width, height).map_err(|error| match error {
        RenderError::BadSize => failed(format!("{width}x{height} is past the renderer's size")),
        RenderError::Panicked => failed("the renderer crashed on it".into()),
    })?;
    let rgb = into_rgb(rgba);

    let mut png = Vec::new();
    // Fast compression: the bytes live for one request, so encode speed wins.
    PngEncoder::new_with_quality(
        Cursor::new(&mut png),
        CompressionType::Fast,
        FilterType::Adaptive,
    )
    .write_image(&rgb, width, height, ExtendedColorType::Rgb8)
    .map_err(|error| failed(error.to_string()))?;

    Ok(RenderedPage {
        page_no,
        width,
        height,
        png,
    })
}

/// Opaque RGBA8 to RGB8 in the same allocation, so a page never holds both.
pub(super) fn into_rgb(mut pixels: Vec<u8>) -> Vec<u8> {
    let count = pixels.len() / 4;
    for index in 0..count {
        let (from, to) = (index * 4, index * 3);
        pixels[to] = pixels[from];
        pixels[to + 1] = pixels[from + 1];
        pixels[to + 2] = pixels[from + 2];
    }
    pixels.truncate(count * 3);
    pixels
}
