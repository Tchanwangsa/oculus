use super::super::raster::{page_of, render_region, with_pdf, Raster};
use super::super::{make, pages, Request, Select, Sidecar};
use super::fixture::{CAPTION, FIGURE, RED};
use super::Setup;

fn request<'a>(s: &'a Setup, video: &'a std::path::Path, select: Select) -> Request<'a> {
    Request {
        video,
        data_dir: s.data(),
        source: s.source,
        page: Some(1),
        select,
        name: None,
        pad: 0.02,
        width: None,
        replace: false,
        exact: false,
    }
}

fn render(s: &Setup, page: u32, region: [f64; 4], w: u32, h: u32) -> Raster {
    let bytes = std::fs::read(s.pdf()).unwrap();
    with_pdf(bytes, move |pdf| {
        render_region(page_of(pdf, page)?, &region, w, h)
    })
    .unwrap()
}

fn pixel(r: &Raster, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * r.width + x) * 3) as usize;
    [r.rgb[i], r.rgb[i + 1], r.rgb[i + 2]]
}

#[test]
fn find_crops_the_figure_with_its_caption() {
    let s = Setup::new("slide-find");
    let video = s.video();
    let made = make(&request(&s, &video, Select::Find("Figure 3".into()))).unwrap();
    assert_eq!(made.name, "p1-figure-3");
    assert_eq!(
        made.picked.iter().map(|b| b.index).collect::<Vec<_>>(),
        [2, 3]
    );
    let sc = &made.sidecar;
    assert_eq!(sc.page, 1);
    assert_eq!(sc.page_size, [720.0, 405.0]);
    // The union of figure and caption, padded by 0.02 x 405 pt; the panel's
    // frame is stroked on its box, so the edges it crosses move out a point.
    let pad = (8.1 / 720.0, 8.1 / 405.0);
    let want = [
        FIGURE[0] - pad.0,
        FIGURE[1] - pad.1,
        FIGURE[2] + pad.0,
        CAPTION[3] + pad.1,
    ];
    for (got, want) in sc.bbox.iter().zip(want) {
        assert!((got - want).abs() < 3e-3, "{:?}", sc.bbox);
    }
    // 2x what fills a 1920x1080 frame's height, capped at 6 megapixels.
    let pixels = u64::from(sc.image.width) * u64::from(sc.image.height);
    assert!(
        pixels <= 6_000_000 && sc.image.height > 2000,
        "{:?}",
        sc.image
    );
    assert_eq!(sc.page_image.width, 1920);
    let (w, h) = image::image_dimensions(&made.png).unwrap();
    assert_eq!((w, h), (sc.image.width, sc.image.height));
    assert_eq!(
        image::image_dimensions(&made.page_png).unwrap(),
        (1920, 1080)
    );
    // The blocks inside, as fractions of the crop.
    let kinds: Vec<_> = sc
        .blocks
        .iter()
        .map(|b| (b.index, b.kind.as_str()))
        .collect();
    assert_eq!(kinds, [(2, "image"), (3, "text")]);
    assert!(
        sc.blocks[0].bbox[0] > 0.0 && sc.blocks[0].bbox[0] < 0.05,
        "{:?}",
        sc.blocks[0].bbox
    );
    assert_eq!(
        sc.blocks[1].text,
        "Figure 3: Each step moves against the slope."
    );
    assert_eq!(sc.props["src"], "slides/p1-figure-3.png");
    assert_eq!(sc.props["page"], "slides/p1-figure-3.page.png");
    assert_eq!(sc.props["source"], "lecture, p. 1");
    let back: Sidecar =
        serde_json::from_str(&std::fs::read_to_string(&made.json).unwrap()).unwrap();
    assert_eq!(&back, sc);
}

#[test]
fn the_same_inputs_write_the_same_bytes() {
    let s = Setup::new("slide-determinism");
    let video = s.video();
    let mut req = request(&s, &video, Select::Blocks(vec![2]));
    req.width = Some(600);
    let one = make(&req).unwrap();
    let bytes =
        |m: &super::super::Made| [&m.png, &m.page_png, &m.json].map(|p| std::fs::read(p).unwrap());
    let first = bytes(&one);
    let two = make(&req).unwrap();
    assert_eq!(first, bytes(&two));
}

#[test]
fn a_crop_is_the_same_pixels_as_that_part_of_the_whole_page() {
    let s = Setup::new("slide-pixels");
    // At 2 px a point, the figure panel starts at a whole pixel.
    let region = [330.0 / 720.0, 80.0 / 405.0, 690.0 / 720.0, 320.0 / 405.0];
    let crop = render(&s, 1, region, 720, 480);
    let page = render(&s, 1, [0.0, 0.0, 1.0, 1.0], 1440, 810);
    let mut worst = 0u8;
    for y in 0..crop.height {
        for x in 0..crop.width {
            let a = pixel(&crop, x, y);
            let b = pixel(&page, x + 660, y + 160);
            for c in 0..3 {
                worst = worst.max(a[c].abs_diff(b[c]));
            }
        }
    }
    assert!(worst <= 8, "worst channel difference {worst}");
    // The panel's pale fill, not the page's white.
    let fill = pixel(&crop, 20, 300);
    assert!(
        fill[0].abs_diff(245) <= 1 && fill[1].abs_diff(247) <= 1 && fill[2] == 255,
        "{fill:?}"
    );
}

#[test]
fn a_rotated_page_is_cropped_as_it_is_shown() {
    let s = Setup::new("slide-rotate");
    let crop = render(&s, 2, RED, 80, 60);
    let red = crop
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .filter(|p| p[0] > 200 && p[1] < 60 && p[2] < 60)
        .count();
    assert!(red as f64 > 0.9 * 80.0 * 60.0, "{red} red pixels of 4800");
    let video = s.video();
    let mut req = request(&s, &video, Select::Bbox(RED));
    req.page = Some(2);
    req.pad = 0.0;
    let made = make(&req).unwrap();
    assert_eq!(made.sidecar.page_size, [300.0, 400.0]);
    assert_eq!(made.name, "p2-region");
    // An unparsed page has no blocks to find.
    let mut req = request(&s, &video, Select::Blocks(vec![0]));
    req.page = Some(2);
    assert!(make(&req).unwrap_err().contains("no blocks"));
}

#[test]
fn bad_requests_say_what_would_do() {
    let s = Setup::new("slide-errors");
    let video = s.video();
    let mut req = request(&s, &video, Select::Blocks(vec![9]));
    assert!(make(&req).unwrap_err().contains("blocks 0-3"));
    req.select = Select::Bbox([0.1, 0.1, 0.2, 0.2]);
    req.page = Some(3);
    assert!(make(&req).unwrap_err().contains("pages 1-2"));
    req.page = None;
    assert!(make(&req).unwrap_err().contains("--bbox needs --page"));
    req.page = Some(1);
    req.name = Some("../x".into());
    assert!(make(&req).unwrap_err().contains("--name"));
}

#[test]
fn a_name_from_another_pdf_is_not_overwritten() {
    let s = Setup::new("slide-replace");
    let video = s.video();
    let mut req = request(&s, &video, Select::Blocks(vec![2]));
    req.name = Some("fig".into());
    req.width = Some(200);
    make(&req).unwrap();
    let other = "courses/TEST101/files/other.pdf";
    std::fs::copy(s.pdf(), s.data().join(other)).unwrap();
    std::fs::copy(
        crate::parse::pages_path(&s.pdf()),
        crate::parse::pages_path(&s.data().join(other)),
    )
    .unwrap();
    req.source = other;
    assert!(make(&req).unwrap_err().contains("already a crop of"));
    req.replace = true;
    assert_eq!(make(&req).unwrap().sidecar.source, other);
}

#[test]
fn blocks_lists_pages_and_draws_them() {
    let s = Setup::new("slide-blocks");
    let all = pages(&s.pdf(), None, None).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!((all[0].width, all[0].height), (720.0, 405.0));
    assert_eq!((all[1].width, all[1].height), (300.0, 400.0));
    assert_eq!(all[0].blocks.len(), 4);
    assert_eq!(all[0].blocks[2].text, "[image]");
    assert!(all[1].blocks.is_empty());
    let out = s.data().join("look/page-1.png");
    let one = pages(&s.pdf(), Some(1), Some((&out, 800))).unwrap();
    assert_eq!(one.len(), 1);
    let drawn = image::open(&out).unwrap().to_rgb8();
    assert_eq!(drawn.dimensions(), (800, 450));
    // The figure's box is outlined in indigo at its top-left corner.
    let x = (FIGURE[0] * 800.0) as u32 + 1;
    let y = (FIGURE[1] * 450.0) as u32 + 30;
    assert_eq!(drawn.get_pixel(x, y).0, [0x5e, 0x6a, 0xd2]);
}

/// Remakes the `Slide` starter's sample in `app/studio-core/library/assets/`
/// (`slide-figure.png`, `slide-page.png`): the fixture's Figure 3 with its
/// caption, and its page, sized for a 1280x720 frame. Run with `--ignored`.
#[test]
#[ignore]
fn write_the_starter_sample() {
    let s = Setup::new("slide-starter");
    let video = s.video();
    std::fs::write(
        video.join("video.json"),
        r#"{ "size": { "width": 1280, "height": 720 }, "scenes": [] }"#,
    )
    .unwrap();
    let mut req = request(&s, &video, Select::Find("Figure 3".into()));
    req.width = Some(1400);
    let made = make(&req).unwrap();
    let out =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../studio-core/library/assets");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::copy(&made.png, out.join("slide-figure.png")).unwrap();
    std::fs::copy(&made.page_png, out.join("slide-page.png")).unwrap();
    println!("{}", serde_json::to_string_pretty(&made.sidecar).unwrap());
}
