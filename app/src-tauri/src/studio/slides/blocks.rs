//! A parsed PDF's blocks as an agent reads them: index, kind, box and the
//! block's text as one line, and `--find`, which picks the block(s) a phrase
//! like "Figure 3" names.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::geometry::Frac;
use crate::parse::ParseOutput;

/// Characters a one-line snippet keeps.
pub const SNIPPET_CHARS: usize = 80;

/// A block of one page, its index into the page's `blocks`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageBlock {
    pub index: usize,
    pub kind: String,
    pub bbox: Frac,
    /// The block's markdown as plain text on one line (an image's caption
    /// for `![caption](...)`, tags and heading marks dropped), untruncated.
    pub text: String,
}

/// MinerU kinds that are a picture rather than running text.
pub fn is_figure(kind: &str) -> bool {
    matches!(kind, "image" | "chart" | "table")
}

/// Page `page_no`'s blocks; none when the page has none or is not there.
pub fn page_blocks(record: &ParseOutput, page_no: u32) -> Vec<PageBlock> {
    let Some(page) = record.pages.iter().find(|p| p.page_no == page_no) else {
        return Vec::new();
    };
    let units: Vec<u16> = page.markdown.encode_utf16().collect();
    page.blocks
        .iter()
        .enumerate()
        .map(|(index, b)| {
            let (start, end) = (b.start as usize, (b.end as usize).min(units.len()));
            let raw = units
                .get(start.min(end)..end)
                .map(String::from_utf16_lossy)
                .unwrap_or_default();
            PageBlock {
                index,
                kind: b.kind.clone(),
                bbox: b.bbox.map(f64::from),
                text: plain(&raw),
            }
        })
        .collect()
}

static IMAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[([^\]]*)\]\([^)]*\)").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Markdown as one line of plain text.
pub fn plain(markdown: &str) -> String {
    let text = IMAGE.replace_all(markdown, |c: &regex::Captures| {
        let alt = c[1].trim();
        if alt.is_empty() {
            " [image] ".to_owned()
        } else {
            format!(" {alt} ")
        }
    });
    let text = TAG.replace_all(&text, " ");
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim_start().trim_start_matches('#').trim())
        .collect();
    SPACE.replace_all(&lines.join(" "), " ").trim().to_owned()
}

/// `text` cut to [`SNIPPET_CHARS`] with an ellipsis.
pub fn snippet(text: &str) -> String {
    if text.chars().count() <= SNIPPET_CHARS {
        return text.to_owned();
    }
    let cut: String = text.chars().take(SNIPPET_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// The block(s) `--find` picked, on one page.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub page: u32,
    pub blocks: Vec<usize>,
}

/// The block whose text holds `needle` (any case), on `page` or on any page:
///
/// * one figure (image, chart, table) among the matches wins;
/// * else one text block next to a figure is a caption, and both are taken;
/// * else one match is taken as it is.
///
/// No match, or a tie at the tier that decided, is an error listing the
/// candidates, so the caller can name a block instead.
pub fn find(record: &ParseOutput, page: Option<u32>, needle: &str) -> Result<Found, String> {
    let want = needle.trim().to_lowercase();
    if want.is_empty() {
        return Err("--find needs some text, e.g. --find \"Figure 3\"".to_owned());
    }
    let pages: Vec<u32> = match page {
        Some(p) => vec![p],
        None => record.pages.iter().map(|p| p.page_no).collect(),
    };
    // (page, its blocks, the matching indices)
    let mut hits: Vec<(u32, Vec<PageBlock>, usize)> = Vec::new();
    for p in pages {
        let blocks = page_blocks(record, p);
        for b in &blocks {
            if b.text.to_lowercase().contains(&want) {
                hits.push((p, blocks.clone(), b.index));
            }
        }
    }
    if hits.is_empty() {
        let place = page.map_or("any page".to_owned(), |p| format!("page {p}"));
        return Err(format!(
            "no block on {place} contains \"{needle}\"; list them with `oculus studio blocks <FILE>{}` and pass --block or --bbox",
            page.map_or(String::new(), |p| format!(" --page {p}"))
        ));
    }
    let figures: Vec<_> = hits
        .iter()
        .filter(|(_, blocks, i)| is_figure(&blocks[*i].kind))
        .collect();
    if figures.len() == 1 {
        let (p, _, i) = figures[0];
        return Ok(Found {
            page: *p,
            blocks: vec![*i],
        });
    }
    if figures.len() > 1 {
        return Err(ambiguous(needle, &figures));
    }
    let captions: Vec<_> = hits
        .iter()
        .filter_map(|(p, blocks, i)| caption_of(blocks, *i).map(|f| (p, blocks, *i, f)))
        .collect();
    if captions.len() == 1 {
        let (p, _, i, f) = captions[0];
        let mut picked = vec![f, i];
        picked.sort_unstable();
        return Ok(Found {
            page: *p,
            blocks: picked,
        });
    }
    if captions.len() > 1 {
        let tied: Vec<_> = hits
            .iter()
            .filter(|(p, _, i)| captions.iter().any(|(q, _, j, _)| *q == p && j == i))
            .collect();
        return Err(ambiguous(needle, &tied));
    }
    if hits.len() == 1 {
        let (p, _, i) = &hits[0];
        return Ok(Found {
            page: *p,
            blocks: vec![*i],
        });
    }
    Err(ambiguous(needle, &hits.iter().collect::<Vec<_>>()))
}

/// The figure a text block at `i` captions: the neighbour before or after
/// it that is a figure, the nearer by vertical gap when both are.
fn caption_of(blocks: &[PageBlock], i: usize) -> Option<usize> {
    let gap = |f: &PageBlock| {
        let t = &blocks[i].bbox;
        (t[1] - f.bbox[3]).max(f.bbox[1] - t[3]).max(0.0)
    };
    let before = i.checked_sub(1).and_then(|j| blocks.get(j));
    let after = blocks.get(i + 1);
    [before, after]
        .into_iter()
        .flatten()
        .filter(|b| is_figure(&b.kind))
        .min_by(|a, b| gap(a).total_cmp(&gap(b)))
        .map(|b| b.index)
}

fn ambiguous(needle: &str, hits: &[&(u32, Vec<PageBlock>, usize)]) -> String {
    let mut out = format!(
        "\"{needle}\" matches {} blocks; name one with --page and --block:",
        hits.len()
    );
    for (p, blocks, i) in hits.iter().take(12) {
        let b = &blocks[*i];
        out.push_str(&format!(
            "\n       page {p} block {i} ({}): {}",
            b.kind,
            snippet(&b.text)
        ));
    }
    if hits.len() > 12 {
        out.push_str(&format!("\n       … and {} more", hits.len() - 12));
    }
    out
}
