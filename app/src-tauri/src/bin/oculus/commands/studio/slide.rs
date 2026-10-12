//! `oculus studio slide` and `blocks`: a crop of a library PDF page into a
//! video's `assets/slides/`, and the blocks to pick one from
//! (`app_lib::studio::slides`, docs/studio.md, "Slides crop a library PDF
//! page").

use crate::*;
use app_lib::studio::slides::{self, blocks::snippet, geometry, Request, Select};

/// How wide `blocks --png` draws the page.
const PREVIEW_WIDTH: u32 = 1600;

impl Ctx {
    pub(crate) fn studio_slide(&self, args: &StudioSlideArgs) -> Result<(), String> {
        let select = match (&args.find, args.block.is_empty(), &args.bbox) {
            (Some(text), true, None) => Select::Find(text.clone()),
            (None, false, None) => Select::Blocks(args.block.clone()),
            (None, true, Some(b)) => Select::Bbox(geometry::parse_box(b)?),
            _ => {
                return Err(
                    "name the region one way: --find TEXT, --block I or --bbox X0,Y0,X1,Y1"
                        .to_owned(),
                )
            }
        };
        let source = self.library_pdf(&args.file, args.subject.as_ref())?;
        let made = slides::make(&Request {
            video: &args.dir,
            data_dir: &self.data_dir,
            source: &source,
            page: args.page,
            select,
            name: args.name.clone(),
            pad: args.pad,
            width: args.width,
            replace: args.replace,
        })?;
        let sc = &made.sidecar;
        if self.json {
            return self.emit(&serde_json::json!({
                "name": made.name,
                "png": made.png.to_string_lossy(),
                "pagePng": made.page_png.to_string_lossy(),
                "json": made.json.to_string_lossy(),
                "sidecar": sc,
            }));
        }
        println!(
            "{} {} ({}×{}), {}.page.png, {}.json",
            paint("wrote", GREEN),
            made.png.display(),
            sc.image.width,
            sc.image.height,
            made.name,
            made.name
        );
        let picked: Vec<String> = made
            .picked
            .iter()
            .map(|b| format!("{} {}", b.index, b.kind))
            .collect();
        let from = if picked.is_empty() {
            String::new()
        } else {
            format!(", block {}", picked.join(" + "))
        };
        println!(
            "{}",
            paint(
                &format!(
                    "  {} p. {}{from} · crop {}",
                    sc.source,
                    sc.page,
                    geometry::fmt_box(&sc.bbox)
                ),
                DIM
            )
        );
        println!("  props: {}", sc.props);
        if !sc.blocks.is_empty() {
            println!("  blocks inside, as fractions of the crop (for highlight boxes):");
            for b in &sc.blocks {
                println!(
                    "    {:>2} {:<8} [{}]  {}",
                    b.index,
                    b.kind,
                    geometry::fmt_box(&b.bbox),
                    b.text
                );
            }
        }
        Ok(())
    }

    pub(crate) fn studio_blocks(&self, args: &StudioBlocksArgs) -> Result<(), String> {
        let source = self.library_pdf(&args.file, args.subject.as_ref())?;
        let pdf = self.data_dir.join(&source);
        let png = args.png.as_deref().map(|p| (p, PREVIEW_WIDTH));
        let pages = slides::pages(&pdf, args.page, png)?;
        if self.json {
            return self.emit(&serde_json::json!({
                "path": source,
                "png": args.png.as_ref().map(|p| p.to_string_lossy()),
                "pages": pages,
            }));
        }
        println!("{}", paint(&source, BOLD));
        let mut none = 0;
        for page in &pages {
            println!(
                "page {} · {}×{} pt · {} block{}",
                page.page,
                page.width,
                page.height,
                page.blocks.len(),
                if page.blocks.len() == 1 { "" } else { "s" }
            );
            none += usize::from(page.blocks.is_empty());
            for b in &page.blocks {
                println!(
                    "  {:>2} {:<8} [{}]  {}",
                    b.index,
                    b.kind,
                    geometry::fmt_box(&b.bbox),
                    snippet(&b.text)
                );
            }
        }
        if none == pages.len() {
            println!(
                "{}",
                paint(
                    "  no blocks: the PDF is not parsed, or was parsed before blocks were recorded; crop with --bbox",
                    DIM
                )
            );
        }
        if let Some(out) = &args.png {
            println!("{} {}", paint("wrote", GREEN), out.display());
        }
        Ok(())
    }
}
