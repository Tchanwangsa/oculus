//! Turning MinerU's `content_list.json` into page records, for both backends.
//!
//! This decides what the markdown *says*, so a subtle change here silently
//! rewrites the library rather than failing. The tests pin the exact output
//! the existing library was rendered with.

mod boilerplate;
mod content;
mod item;
mod order;
mod page;
#[cfg(test)]
mod tests;

use crate::parse::{ParseBlock, ParseError, ParsePage};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;

use boilerplate::find_boilerplate;
use content::*;
use order::compare;
use page::render_page;

/// Crops render at ~1.5x: 20k px² rejects small template furniture while
/// retaining the smallest real figure in the measured deck (58k px²).
const MIN_IMAGE_AREA: u64 = 20_000;

/// Boilerplate is detected in fixed 64-page windows. **Bug-compatibility, not
/// a heuristic**: changing it rewrites markdown already in the library with no
/// version bump (`PARSER_VERSION` guards shape, not text).
const RENDER_GROUP_PAGES: i64 = 64;

/// Render a backend result and copy only the referenced, useful image crops.
/// Items must carry **absolute** `page_idx` values, or the 64-page windows
/// would restart at every task boundary.
///
/// `source_images` is the extracted result's `images` directory; `images_dir`
/// the staging directory; `images_rel` the link prefix. Returns the page
/// records and the number of images copied.
pub fn render(
    content_list: &[Value],
    total_pages: u32,
    source_images: &Path,
    images_dir: &Path,
    images_rel: &str,
) -> Result<(Vec<ParsePage>, u32), ParseError> {
    // 1. Which crops the markdown will reference, deduped and in stable order.
    let wanted: BTreeSet<String> = content_list
        .iter()
        .filter(|item| IMAGE_TYPES.contains(&kind_of(item)))
        .filter_map(|item| img_path(item).map(basename))
        .collect();

    let mut dropped: HashSet<String> = HashSet::new();
    let mut image_count = 0u32;
    if !wanted.is_empty() && source_images.is_dir() {
        fs::create_dir_all(images_dir)
            .map_err(|e| ParseError::Io(format!("create {}: {e}", images_dir.display())))?;
        for name in &wanted {
            let source = source_images.join(name);
            // Named but not shipped: not counted, though its item still links.
            if !source.is_file() {
                continue;
            }
            if too_small(&source) {
                dropped.insert(name.clone());
                continue;
            }
            fs::copy(&source, images_dir.join(name)).map_err(|e| {
                ParseError::Io(format!(
                    "copy {} -> {}: {e}",
                    source.display(),
                    images_dir.display()
                ))
            })?;
            image_count += 1;
        }
    }

    // 2. Detect boilerplate per 64-page window, against that window's own
    //    page count (the last is short).
    let mut windows: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
    for item in content_list {
        windows
            .entry(page_idx(item).div_euclid(RENDER_GROUP_PAGES))
            .or_default()
            .push(item);
    }

    let mut by_page: BTreeMap<i64, (String, Vec<ParseBlock>)> = BTreeMap::new();
    for (window, items) in windows {
        let window_pages = RENDER_GROUP_PAGES.min(total_pages as i64 - window * RENDER_GROUP_PAGES);
        let boilerplate = find_boilerplate(&items, window_pages);

        let mut per_page: BTreeMap<i64, Vec<&Value>> = BTreeMap::new();
        for item in items {
            per_page.entry(page_idx(item) + 1).or_default().push(item);
        }
        for (page_no, mut page_items) in per_page {
            page_items.sort_by(|left, right| compare(left, right));
            if let Some(page) = render_page(&page_items, images_rel, &dropped, &boilerplate) {
                // Windows partition by page: no page is written twice.
                by_page.insert(page_no, page);
            }
        }
    }

    // 3. Blank and furniture-only pages still need records: `page_no` is the
    //    citation join key.
    for page_no in 1..=i64::from(total_pages) {
        by_page.entry(page_no).or_default();
    }

    let pages = by_page
        .into_iter()
        // A broken offset below the first page has nowhere to attach.
        .filter(|(page_no, _)| *page_no >= 1)
        .map(|(page_no, (markdown, blocks))| ParsePage {
            page_no: page_no as u32,
            markdown,
            blocks,
        })
        .collect();
    Ok((pages, image_count))
}

/// Is this crop too small to be a real figure? Reads the header only. An
/// unreadable image **fails open**: losing a figure is worse than keeping
/// furniture.
fn too_small(path: &Path) -> bool {
    match imagesize::size(path) {
        Ok(size) => (size.width as u64) * (size.height as u64) < MIN_IMAGE_AREA,
        Err(_) => false,
    }
}
