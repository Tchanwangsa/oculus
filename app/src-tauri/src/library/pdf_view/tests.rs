use super::documents::{guarded, open_at, page_sizes, resolve_in};
use super::layout::{layout, TextGlyph};
use super::links::page_links;
use super::render::{render_page, scale_to};
use super::text::page_text;
use crate::test_support::Scratch;
use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{PixmapSettings, RenderCache, RenderSettings};
use lopdf::{dictionary, Document, Object as Lo, Stream};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Two US-letter pages. Page 1 says "Hello world" in Helvetica and holds a
/// URI link, a `/Dest` link to page 2 and a named GoTo link to page 2.
/// Page 2 is rotated 90° and links back to page 1.
fn sample_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let page1 = doc.new_object_id();
    let page2 = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let rect = |x0: i64, y0: i64, x1: i64, y1: i64| -> Lo {
        vec![x0.into(), y0.into(), x1.into(), y1.into()].into()
    };
    let uri = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(72, 690, 200, 730),
        "A" => dictionary! { "S" => "URI", "URI" => Lo::string_literal("https://example.com/a") },
    });
    let goto = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(72, 600, 150, 620),
        "Dest" => vec![Lo::Reference(page2), "Fit".into()],
    });
    let named = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(72, 500, 150, 520),
        "A" => dictionary! { "S" => "GoTo", "D" => Lo::string_literal("second") },
    });
    let back = doc.add_object(dictionary! {
        "Type" => "Annot", "Subtype" => "Link", "Rect" => rect(0, 0, 100, 50),
        "Dest" => vec![Lo::Reference(page1), "Fit".into()],
    });
    let text = doc.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 24 Tf 72 700 Td (Hello world) Tj ET".to_vec(),
    ));
    let empty = doc.add_object(Stream::new(dictionary! {}, Vec::new()));
    let media = rect(0, 0, 612, 792);
    doc.objects.insert(
        page1,
        Lo::Dictionary(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "MediaBox" => media.clone(),
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            "Contents" => text,
            "Annots" => vec![Lo::Reference(uri), Lo::Reference(goto), Lo::Reference(named)],
        }),
    );
    doc.objects.insert(
        page2,
        Lo::Dictionary(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "MediaBox" => media, "Rotate" => 90,
            "Contents" => empty, "Annots" => vec![Lo::Reference(back)],
        }),
    );
    doc.objects.insert(pages_id, Lo::Dictionary(dictionary! {
        "Type" => "Pages", "Kids" => vec![Lo::Reference(page1), Lo::Reference(page2)], "Count" => 2,
    }));
    let dests = doc.add_object(dictionary! {
        "Names" => vec![Lo::string_literal("second"), vec![Lo::Reference(page2), "Fit".into()].into()],
    });
    let catalog = doc.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => pages_id,
        "Names" => dictionary! { "Dests" => dests },
    });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

fn sample() -> Pdf {
    Pdf::new(Arc::new(sample_pdf())).unwrap()
}

/// One US-letter page drawing `content` with Helvetica as /F1.
fn one_page(content: &str) -> Pdf {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
    });
    let stream = doc.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        "Contents" => stream,
    });
    doc.objects.insert(
        pages_id,
        Lo::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![Lo::Reference(page)], "Count" => 1,
        }),
    );
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    Pdf::new(Arc::new(bytes)).unwrap()
}

fn texts(pdf: &Pdf) -> Vec<String> {
    page_text(pdf, 1)
        .unwrap()
        .lines
        .into_iter()
        .map(|line| line.text)
        .collect()
}

#[test]
fn pdf_view_text_drops_zero_scale_glyphs() {
    let pdf = one_page(
        "BT /F1 24 Tf 1 0 0 1 72 700 Tm (Seen) Tj \
         0 0 0 0 72 600 Tm (latexi sha1_base64) Tj \
         1 0 0 1 72 500 Tm 0 Tz (squashed) Tj ET",
    );
    assert_eq!(texts(&pdf), ["Seen"]);
}

#[test]
fn pdf_view_text_drops_glyphs_off_the_page() {
    let pdf = one_page(
        "BT /F1 24 Tf 1 0 0 1 72 700 Tm (Seen) Tj \
         1 0 0 1 -3000 600 Tm (overlay) Tj \
         1 0 0 1 72 3000 Tm (above) Tj ET",
    );
    assert_eq!(texts(&pdf), ["Seen"]);
}

#[test]
fn pdf_view_paths_stay_inside_the_library_roots() {
    let root = Path::new("/data");
    assert_eq!(
        resolve_in(root, "courses/X/files/a.pdf"),
        Ok(PathBuf::from("/data/courses/X/files/a.pdf"))
    );
    assert!(resolve_in(root, "lectures/a.pdf").is_ok());
    assert!(resolve_in(root, "agents/a.pdf").is_ok());
    for bad in [
        "",
        "/etc/passwd",
        "../a.pdf",
        "courses/../../a.pdf",
        "data/a.pdf",
        "oculus.db",
    ] {
        assert_eq!(
            resolve_in(root, bad),
            Err("outside-library".to_string()),
            "{bad}"
        );
    }
}

#[test]
fn pdf_view_opens_through_the_cache_and_reports_errors() {
    let root = Scratch::new("pdf-view-open");
    std::fs::create_dir_all(root.join("courses/X")).unwrap();
    std::fs::write(root.join("courses/X/a.pdf"), sample_pdf()).unwrap();
    std::fs::write(root.join("courses/X/bad.pdf"), b"not a pdf").unwrap();

    let pdf = open_at(&root, "courses/X/a.pdf").unwrap();
    let again = open_at(&root, "courses/X/a.pdf").unwrap();
    assert!(
        Arc::ptr_eq(&pdf, &again),
        "an unchanged file comes from the cache"
    );
    let sizes = page_sizes(&pdf).pages;
    assert_eq!(sizes.len(), 2);
    assert_eq!((sizes[0].width, sizes[0].height), (612.0, 792.0));
    assert_eq!(
        (sizes[1].width, sizes[1].height),
        (792.0, 612.0),
        "rotation swaps the sides"
    );

    assert_eq!(
        open_at(&root, "courses/X/missing.pdf").err().as_deref(),
        Some("not-found")
    );
    assert_eq!(
        open_at(&root, "courses/X/bad.pdf").err().as_deref(),
        Some("invalid")
    );
    assert_eq!(
        open_at(&root, "../a.pdf").err().as_deref(),
        Some("outside-library")
    );
    assert_eq!(
        page_text(&pdf, 3).err().as_deref(),
        Some("page-out-of-range")
    );
    assert_eq!(
        page_text(&pdf, 0).err().as_deref(),
        Some("page-out-of-range")
    );
}

#[test]
fn pdf_view_renders_exactly_the_requested_size() {
    let pdf = sample();
    for (width, height) in [(300, 388), (612, 792), (1001, 1297), (97, 13)] {
        let rgba = render_page(&pdf, 1, width, height).unwrap();
        assert_eq!(
            rgba.len(),
            (width * height * 4) as usize,
            "{width}x{height}"
        );
        assert!(rgba.chunks_exact(4).all(|px| px[3] == 255), "opaque");
    }
    let rgba = render_page(&pdf, 1, 612, 792).unwrap();
    let dark = rgba.chunks_exact(4).filter(|px| px[0] < 128).count();
    assert!(dark > 100, "the text paints: {dark} dark pixels");
    // The text sits at x 72..~200, baseline y 92 from the top: nothing
    // to its right on that row band.
    let px = |x: usize, y: usize| rgba[(y * 612 + x) * 4];
    assert!((70..100).any(|y| (72..200).any(|x| px(x, y) < 128)));
    assert!((70..100).all(|y| (400..612).all(|x| px(x, y) == 255)));

    assert_eq!(
        render_page(&pdf, 1, 0, 10).err().as_deref(),
        Some("invalid-size")
    );
    assert_eq!(
        render_page(&pdf, 1, 8193, 10).err().as_deref(),
        Some("too-large")
    );
    assert_eq!(
        render_page(&pdf, 1, 8000, 8000).err().as_deref(),
        Some("too-large")
    );
}

#[test]
fn pdf_view_a_panic_inside_hayro_fails_only_its_request() {
    assert_eq!(
        guarded(|| -> u8 { panic!("malformed") }),
        Err("render-failed".to_string())
    );
    assert_eq!(guarded(|| 7), Ok(7));
}

#[test]
fn pdf_view_scale_truncates_to_the_requested_pixels() {
    for side in [612.0f32, 792.0, 595.28, 841.89, 1.0, 13.7] {
        for pixels in [1u32, 13, 97, 300, 1001, 1600, 4096, 8192] {
            let scale = scale_to(side, pixels);
            assert_eq!((side * scale) as u16 as u32, pixels, "{side} -> {pixels}");
        }
    }
}

#[test]
fn pdf_view_text_reads_hello_world_with_stops() {
    let pdf = sample();
    let text = page_text(&pdf, 1).unwrap();
    assert_eq!(text.lines.len(), 1, "{:?}", text.lines);
    let line = &text.lines[0];
    assert_eq!(line.text, "Hello world");
    assert!(!line.vertical);
    assert_eq!(line.chars.len(), line.text.encode_utf16().count() + 1);
    assert!(
        line.chars.windows(2).all(|w| w[0] <= w[1]),
        "{:?}",
        line.chars
    );
    assert!((line.chars[0] - 72.0).abs() < 0.5, "{:?}", line.chars);
    assert!((line.x - 72.0).abs() < 0.5);
    // Em box of a 24pt font with its baseline 92pt from the top.
    assert!((line.y - (92.0 - 19.2)).abs() < 0.5, "{}", line.y);
    assert!((line.height - 24.0).abs() < 0.5, "{}", line.height);
    assert!((line.x + line.width - line.chars[line.chars.len() - 1]).abs() < 0.01);
    assert!(page_text(&pdf, 2).unwrap().lines.is_empty());
}

#[test]
fn pdf_view_resolves_links() {
    let pdf = sample();
    let links = page_links(&pdf, 1).unwrap();
    assert_eq!(links.len(), 3, "{links:?}");
    assert_eq!(links[0].uri.as_deref(), Some("https://example.com/a"));
    assert_eq!(
        (links[0].x, links[0].y, links[0].width, links[0].height),
        (72.0, 62.0, 128.0, 40.0)
    );
    assert_eq!((links[1].page, links[1].uri.as_deref()), (Some(2), None));
    assert_eq!(links[2].page, Some(2), "named destination");
    // Page 2 is rotated 90° clockwise: its bottom-left corner box
    // lands at the rendered page's top-left.
    let back = page_links(&pdf, 2).unwrap();
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].page, Some(1));
    assert_eq!(
        (back[0].x, back[0].y, back[0].width, back[0].height),
        (0.0, 0.0, 50.0, 100.0)
    );
}

#[test]
fn pdf_view_layout_reads_columns_and_word_gaps() {
    fn word(v: &mut Vec<TextGlyph>, s: &str, x: f32, y: f32, advance: f32) {
        for (i, ch) in s.chars().enumerate() {
            let x0 = x + i as f32 * advance;
            v.push(TextGlyph {
                ch,
                rect: [x0, y, x0 + 6.0, y + 10.0],
            });
        }
    }
    let text = |v: Vec<TextGlyph>| -> Vec<String> {
        layout(v)
            .iter()
            .map(|line| {
                line.iter()
                    .map(|(g, space)| {
                        if *space {
                            format!(" {}", g.ch)
                        } else {
                            g.ch.to_string()
                        }
                    })
                    .collect()
            })
            .collect()
    };
    let mut v = Vec::new();
    word(&mut v, "Left1", 10.0, 10.0, 6.0);
    word(&mut v, "Right1", 200.0, 10.0, 6.0);
    word(&mut v, "Left2", 10.0, 24.0, 6.0);
    word(&mut v, "Right2", 200.0, 24.0, 6.0);
    assert_eq!(text(v), ["Left1", "Left2", "Right1", "Right2"]);

    let mut v = Vec::new();
    word(&mut v, "CHAPTER", 10.0, 10.0, 9.0);
    word(&mut v, "FOUR", 10.0 + 7.0 * 9.0 + 5.0, 10.0, 9.0);
    assert_eq!(text(v), ["CHAPTER FOUR"]);

    // Drawn right word first: the space still lands between the words.
    let mut v = Vec::new();
    word(&mut v, "two", 34.0, 10.0, 6.0);
    word(&mut v, "one", 10.0, 10.0, 6.0);
    assert_eq!(text(v), ["one two"]);

    let mut v = Vec::new();
    for (i, ch) in "Bold".chars().enumerate() {
        let x = 10.0 + i as f32 * 7.0;
        v.push(TextGlyph {
            ch,
            rect: [x, 10.0, x + 6.0, 20.0],
        });
        v.push(TextGlyph {
            ch,
            rect: [x + 0.3, 10.0, x + 6.3, 20.0],
        });
    }
    assert_eq!(text(v), ["Bold"]);
}

/// Times a real slide deck from the live library (read-only):
/// `cargo test pdf_view_bench -- --ignored --nocapture`.
#[test]
#[ignore]
fn pdf_view_bench_a_real_deck() {
    fn find(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "pdf") {
                out.push(path);
            }
        }
    }
    let mut found = Vec::new();
    find(
        &crate::library::paths::data_dir().join("courses"),
        &mut found,
    );
    found.sort();
    let wanted = std::env::var("PDF_VIEW_BENCH").ok();
    let deck = found
        .iter()
        .filter(|path| {
            wanted
                .as_deref()
                .map_or(true, |w| path.to_string_lossy().contains(w))
        })
        .find_map(|path| {
            let pdf = Pdf::new(Arc::new(std::fs::read(path).ok()?)).ok()?;
            let page = pdf.pages().first()?;
            let (w, h) = page.render_dimensions();
            (pdf.pages().len() >= 10 && w > h).then(|| (path.clone(), pdf))
        })
        .expect("a landscape PDF of 10+ pages under courses/");
    let (path, pdf) = deck;
    println!("deck: {} ({} pages)", path.display(), pdf.pages().len());
    let start = std::time::Instant::now();
    let bytes = std::fs::read(&path).unwrap();
    let reopened = Pdf::new(Arc::new(bytes)).unwrap();
    println!("open: {:.1} ms", start.elapsed().as_secs_f64() * 1e3);
    drop(reopened);

    let pages = 1..=pdf.pages().len().min(10) as u32;
    let mut render_ms = Vec::new();
    let mut text_ms = Vec::new();
    for page in pages.clone() {
        let (w, h) = pdf.pages()[page as usize - 1].render_dimensions();
        let height = (1600.0 * h / w).round() as u32;
        let start = std::time::Instant::now();
        let rgba = render_page(&pdf, page, 1600, height).unwrap();
        render_ms.push(start.elapsed().as_secs_f64() * 1e3);
        assert_eq!(rgba.len(), (1600 * height * 4) as usize);
        if let Some(dir) = std::env::var_os("PDF_VIEW_BENCH_OUT") {
            let image = image::RgbaImage::from_raw(1600, height, rgba).unwrap();
            image
                .save(Path::new(&dir).join(format!("page-{page}.png")))
                .unwrap();
        }
        let start = std::time::Instant::now();
        let text = page_text(&pdf, page).unwrap();
        text_ms.push(start.elapsed().as_secs_f64() * 1e3);
        if page == 1 {
            println!(
                "page 1 at 1600x{height}, {} lines; first: {:?}",
                text.lines.len(),
                text.lines.first().map(|l| &l.text)
            );
        }
        if let Some(dir) = std::env::var_os("PDF_VIEW_BENCH_OUT") {
            let lines: Vec<&str> = text.lines.iter().map(|line| line.text.as_str()).collect();
            std::fs::write(
                Path::new(&dir).join(format!("page-{page}.txt")),
                lines.join("\n"),
            )
            .unwrap();
        }
    }
    println!("render ms (fresh cache) per page: {render_ms:.1?}");
    println!("text ms per page: {text_ms:.1?}");

    // The same pages through one shared RenderCache, as a per-thread
    // cache would see them.
    let cache = RenderCache::new();
    let mut shared_ms = Vec::new();
    for page in pages {
        let page = &pdf.pages()[page as usize - 1];
        let (w, h) = page.render_dimensions();
        let height = (1600.0 * h / w).round() as u32;
        let settings = PixmapSettings {
            x_scale: scale_to(w, 1600),
            y_scale: scale_to(h, height),
            bg_color: WHITE,
        };
        let start = std::time::Instant::now();
        let _ = hayro::render(
            page,
            &cache,
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &settings,
        );
        shared_ms.push(start.elapsed().as_secs_f64() * 1e3);
    }
    println!("render ms (shared cache) per page: {shared_ms:.1?}");
}
