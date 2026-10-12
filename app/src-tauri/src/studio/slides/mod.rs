//! Slides: a region of a library PDF page as a video asset, so a scene can
//! show "page 12, Figure 3" for real (docs/studio.md, "Slides crop a library
//! PDF page"). `make` resolves the region from the parse record's blocks
//! (`--find`, `--block`) or a box, renders it and the whole page with hayro,
//! and writes `assets/slides/<name>.png`, `<name>.page.png` and the
//! `<name>.json` sidecar; `pages` lists the blocks an agent picks from. A crop
//! is made once, never at render time, and the segment cache hashes `assets/`.

pub mod blocks;
pub mod geometry;
pub mod preview;
pub mod raster;
pub mod sidecar;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

use crate::parse::{read_record, ParseOutput};
use blocks::{page_blocks, PageBlock};
use geometry::{crop_pixels, padded, page_pixels, round4, union, within};
use raster::{encode_png, page_of, page_size, render_region, with_pdf, Raster};
pub use sidecar::{lint, Select, Sidecar, SLIDES_DIR};
use sidecar::{sha256, ImageRef, Inner};

/// A frame when `video.json` cannot be read for one.
const DEFAULT_FRAME: (u32, u32) = (1920, 1080);

/// What `oculus studio slide` asked for.
pub struct Request<'a> {
    /// The video folder.
    pub video: &'a Path,
    pub data_dir: &'a Path,
    /// The PDF, relative to `data_dir`.
    pub source: &'a str,
    pub page: Option<u32>,
    pub select: Select,
    pub name: Option<String>,
    pub pad: f64,
    pub width: Option<u32>,
    /// Overwrite a slide of the same name cropped from another PDF.
    pub replace: bool,
}

/// What `make` wrote.
#[derive(Debug)]
pub struct Made {
    pub name: String,
    pub sidecar: Sidecar,
    pub png: PathBuf,
    pub page_png: PathBuf,
    pub json: PathBuf,
    /// The blocks the region was made from (none for a box).
    pub picked: Vec<PageBlock>,
}

/// Renders the region and writes the slide's three files.
pub fn make(req: &Request<'_>) -> Result<Made, String> {
    let pdf = req.data_dir.join(req.source);
    let bytes = std::fs::read(&pdf).map_err(|e| format!("{}: {e}", pdf.display()))?;
    let sha = sha256(&bytes);
    let record = read_record(&pdf);

    let (page, picked, boxes) = match &req.select {
        Select::Find(text) => {
            let found = blocks::find(need_blocks(record.as_ref(), req.source)?, req.page, text)?;
            let all = page_blocks(need_blocks(record.as_ref(), req.source)?, found.page);
            let picked: Vec<PageBlock> = found.blocks.iter().map(|i| all[*i].clone()).collect();
            let boxes = picked.iter().map(|b| b.bbox).collect();
            (found.page, picked, boxes)
        }
        Select::Blocks(indices) => {
            let page = need_page(req.page, "--block")?;
            let all = page_blocks(need_blocks(record.as_ref(), req.source)?, page);
            if all.is_empty() {
                return Err(format!(
                    "{} page {page} has no blocks; look with `oculus studio blocks {} --page {page}`, or pass --bbox",
                    req.source, req.source
                ));
            }
            let picked = indices
                .iter()
                .map(|i| {
                    all.get(*i).cloned().ok_or_else(|| {
                        format!(
                            "page {page} has blocks 0-{}; there is no block {i}",
                            all.len() - 1
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let boxes = picked.iter().map(|b| b.bbox).collect();
            (page, picked, boxes)
        }
        Select::Bbox(b) => (need_page(req.page, "--bbox")?, Vec::new(), vec![*b]),
    };
    let wanted = union(&boxes).ok_or("pick at least one block")?;

    let name = match &req.name {
        Some(n) => valid_name(n)?,
        None => default_name(&req.select, page),
    };
    let folder = req.video.join("assets").join(SLIDES_DIR);
    let json_path = folder.join(format!("{name}.json"));
    if !req.replace {
        if let Some(old) = std::fs::read_to_string(&json_path)
            .ok()
            .and_then(|t| serde_json::from_str::<Sidecar>(&t).ok())
        {
            if old.source != req.source {
                return Err(format!(
                    "slides/{name} is already a crop of {}; pick another --name, or pass --replace",
                    old.source
                ));
            }
        }
    }

    let frame = frame_size(req.video);
    let (pad, width) = (req.pad, req.width);
    let (size, region, crop, whole) = with_pdf(bytes, move |pdf| {
        let p = page_of(pdf, page)?;
        let size = page_size(p);
        let region = padded(wanted, pad, size)?;
        let (cw, ch) = crop_pixels(&region, size, frame, width)?;
        let crop = render_region(p, &region, cw, ch)?;
        let (pw, ph) = page_pixels(size, frame);
        let whole = render_region(p, &[0.0, 0.0, 1.0, 1.0], pw, ph)?;
        Ok((size, region, crop, whole))
    })?;

    let inside: Vec<Inner> = record
        .as_ref()
        .map(|r| page_blocks(r, page))
        .unwrap_or_default()
        .into_iter()
        .filter_map(|b| {
            within(&b.bbox, &region).map(|bbox| Inner {
                index: b.index,
                kind: b.kind,
                bbox,
                text: blocks::snippet(&b.text),
            })
        })
        .collect();
    let src = format!("{SLIDES_DIR}/{name}.png");
    let page_src = format!("{SLIDES_DIR}/{name}.page.png");
    let (cw_pt, ch_pt) = geometry::size_pt(&region, size);
    let props = json!({
        "src": src,
        "aspect": round4(cw_pt / ch_pt),
        "page": page_src,
        "pageAspect": round4(size.0 / size.1),
        "bbox": region,
        "source": source_label(req.source, page),
    });
    let sidecar = Sidecar {
        source: req.source.to_owned(),
        sha256: sha,
        page,
        page_size: [round4(size.0), round4(size.1)],
        bbox: region,
        select: req.select.clone(),
        pad: req.pad,
        width: req.width,
        image: image_ref(&src, &crop),
        page_image: image_ref(&page_src, &whole),
        blocks: inside,
        props,
    };

    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let png = folder.join(format!("{name}.png"));
    let page_png = folder.join(format!("{name}.page.png"));
    write_atomic(&png, &encode_png(&crop)?)?;
    write_atomic(&page_png, &encode_png(&whole)?)?;
    let mut text = serde_json::to_string_pretty(&sidecar).map_err(|e| e.to_string())?;
    text.push('\n');
    write_atomic(&json_path, text.as_bytes())?;
    Ok(Made {
        name,
        sidecar,
        png,
        page_png,
        json: json_path,
        picked,
    })
}

/// One page's size and blocks, as `oculus studio blocks` prints them.
#[derive(Debug, Serialize)]
pub struct PageInfo {
    pub page: u32,
    /// Points, as rendered.
    pub width: f64,
    pub height: f64,
    pub blocks: Vec<PageBlock>,
}

/// Each page's size and blocks (or only `page`'s), and with `png`, that
/// page rendered `png_width` px wide with its blocks drawn on.
pub fn pages(
    pdf: &Path,
    page: Option<u32>,
    png: Option<(&Path, u32)>,
) -> Result<Vec<PageInfo>, String> {
    let bytes = std::fs::read(pdf).map_err(|e| format!("{}: {e}", pdf.display()))?;
    let record = read_record(pdf);
    let png_width = png.map(|(_, w)| w);
    let (sizes, raster) = with_pdf(bytes, move |doc| {
        let count = doc.pages().len() as u32;
        let wanted: Vec<u32> = match page {
            Some(p) => vec![page_of(doc, p).map(|_| p)?],
            None => (1..=count).collect(),
        };
        let sizes: Vec<(u32, (f64, f64))> = wanted
            .iter()
            .map(|p| page_of(doc, *p).map(|pg| (*p, page_size(pg))))
            .collect::<Result<_, _>>()?;
        let raster = match (png_width, page) {
            (Some(w), Some(p)) => {
                let pg = page_of(doc, p)?;
                let size = page_size(pg);
                let h = ((f64::from(w) * size.1 / size.0).round() as u32).max(1);
                Some(render_region(pg, &[0.0, 0.0, 1.0, 1.0], w, h)?)
            }
            _ => None,
        };
        Ok((sizes, raster))
    })?;
    let infos: Vec<PageInfo> = sizes
        .into_iter()
        .map(|(p, (w, h))| PageInfo {
            page: p,
            width: round4(w),
            height: round4(h),
            blocks: record
                .as_ref()
                .map(|r| page_blocks(r, p))
                .unwrap_or_default(),
        })
        .collect();
    if let (Some(mut raster), Some((out, _))) = (raster, png) {
        if let Some(info) = infos.first() {
            preview::annotate(&mut raster, &info.blocks);
        }
        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        write_atomic(out, &encode_png(&raster)?)?;
    }
    Ok(infos)
}

fn need_blocks<'a>(
    record: Option<&'a ParseOutput>,
    source: &str,
) -> Result<&'a ParseOutput, String> {
    match record {
        Some(r) if r.pages.iter().any(|p| !p.blocks.is_empty()) => Ok(r),
        Some(_) => Err(format!(
            "{source} was parsed before block boxes were recorded; `oculus index --reparse` adds them, or pass --page and --bbox"
        )),
        None => Err(format!(
            "{source} has not been parsed, so it has no blocks; pass --page and --bbox (fractions of the page)"
        )),
    }
}

fn need_page(page: Option<u32>, flag: &str) -> Result<u32, String> {
    page.ok_or_else(|| format!("{flag} needs --page N"))
}

fn image_ref(src: &str, raster: &Raster) -> ImageRef {
    ImageRef {
        src: src.to_owned(),
        width: raster.width,
        height: raster.height,
    }
}

/// `[A-Za-z0-9_-]`, 1-64 characters, not ending in `.page`.
pub fn valid_name(name: &str) -> Result<String, String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(name.to_owned())
    } else {
        Err(format!(
            "--name {name}: use 1-64 letters, digits, `-` or `_` (it names assets/slides/<name>.png)"
        ))
    }
}

/// `figure-3` for `--find "Figure 3"`, `p12-b3-4` for blocks, `p12-region`.
fn default_name(select: &Select, page: u32) -> String {
    match select {
        Select::Find(text) => {
            let mut slug = String::new();
            for c in text.to_lowercase().chars() {
                if c.is_ascii_alphanumeric() {
                    slug.push(c);
                } else if !slug.ends_with('-') && !slug.is_empty() {
                    slug.push('-');
                }
            }
            let slug: String = slug.trim_end_matches('-').chars().take(48).collect();
            if slug.is_empty() {
                format!("p{page}-found")
            } else {
                format!("p{page}-{}", slug.trim_end_matches('-'))
            }
        }
        Select::Blocks(b) => format!(
            "p{page}-b{}",
            b.iter().map(usize::to_string).collect::<Vec<_>>().join("-")
        ),
        Select::Bbox(_) => format!("p{page}-region"),
    }
}

/// `lecture 04, p. 12` from `courses/X/files/lecture_04.pdf`.
pub fn source_label(source: &str, page: u32) -> String {
    let stem = Path::new(source)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = stem.trim_end_matches(".docx").trim_end_matches(".pptx");
    let words: Vec<&str> = stem.split(['_', ' ']).filter(|w| !w.is_empty()).collect();
    format!("{}, p. {page}", words.join(" "))
}

/// The video's frame size from `video.json`, or 1920x1080.
fn frame_size(video: &Path) -> (u32, u32) {
    let size = std::fs::read_to_string(video.join("video.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| {
            let s = v.get("size")?;
            Some((
                u32::try_from(s.get("width")?.as_u64()?).ok()?,
                u32::try_from(s.get("height")?.as_u64()?).ok()?,
            ))
        });
    size.filter(|(w, h)| *w > 0 && *h > 0)
        .unwrap_or(DEFAULT_FRAME)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}
