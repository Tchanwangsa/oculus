//! Boxes on a page as fractions of the rendered page (`.pages.json`'s
//! convention: `[x0, y0, x1, y1]`, top-left origin, y down, after the page's
//! rotation and crop box), and the pixel sizes a crop is rendered at.

/// `[x0, y0, x1, y1]` as fractions of the rendered page.
pub type Frac = [f64; 4];

/// Pixels a crop may hold: about 2x a 1080p frame's worth, under the render
/// budget's run-alone threshold.
pub const MAX_CROP_PIXELS: u64 = 6_000_000;

/// The longest side of a crop or page image, in pixels.
pub const MAX_SIDE: u32 = 8192;

/// The smallest crop side, in points: anything thinner is a mistyped box.
const MIN_SIDE_PT: f64 = 2.0;

/// The box around all of `boxes`.
pub fn union(boxes: &[Frac]) -> Option<Frac> {
    boxes.iter().copied().reduce(|a, b| {
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]
    })
}

/// `b` grown by `pad` (a fraction of the page's shorter side, the same in
/// points on every side), clamped to the page, and checked: finite, ordered
/// and at least [`MIN_SIDE_PT`] a side. Rounded to 1e-4 so the sidecar and
/// a re-run with `--bbox` agree.
pub fn padded(b: Frac, pad: f64, page: (f64, f64)) -> Result<Frac, String> {
    if !b.iter().all(|v| v.is_finite()) || !pad.is_finite() || pad < 0.0 {
        return Err(format!(
            "the box {} and the pad {pad} must be finite, the pad not negative",
            fmt_box(&b)
        ));
    }
    let (w, h) = page;
    let p = pad * w.min(h);
    let out = [
        round4((b[0] - p / w).clamp(0.0, 1.0)),
        round4((b[1] - p / h).clamp(0.0, 1.0)),
        round4((b[2] + p / w).clamp(0.0, 1.0)),
        round4((b[3] + p / h).clamp(0.0, 1.0)),
    ];
    if (out[2] - out[0]) * w < MIN_SIDE_PT || (out[3] - out[1]) * h < MIN_SIDE_PT {
        return Err(format!(
            "the box {} is empty or under {MIN_SIDE_PT} pt a side on the page; \
             x0 < x1 and y0 < y1, as fractions of the page from its top left",
            fmt_box(&b)
        ));
    }
    Ok(out)
}

/// The crop's size in points.
pub fn size_pt(b: &Frac, page: (f64, f64)) -> (f64, f64) {
    ((b[2] - b[0]) * page.0, (b[3] - b[1]) * page.1)
}

/// The pixel size of a crop `width` px wide (the height follows the box),
/// or, with no width, enough for `frame` at 2x: the crop fitted inside the
/// frame, doubled. Either is refused past [`MAX_SIDE`]; the default is
/// shrunk under [`MAX_CROP_PIXELS`], an asked-for width is refused past it.
pub fn crop_pixels(
    b: &Frac,
    page: (f64, f64),
    frame: (u32, u32),
    width: Option<u32>,
) -> Result<(u32, u32), String> {
    let (cw, ch) = size_pt(b, page);
    let aspect = cw / ch;
    let side = |w: f64| ((w / aspect).round() as u32).max(1);
    match width {
        Some(w) => {
            let h = side(f64::from(w));
            if w == 0 || w > MAX_SIDE || h > MAX_SIDE {
                return Err(format!(
                    "--width {w} makes a {w}x{h} crop; each side must be 1-{MAX_SIDE} px"
                ));
            }
            if u64::from(w) * u64::from(h) > MAX_CROP_PIXELS {
                return Err(format!(
                    "--width {w} makes a {w}x{h} crop, past {MAX_CROP_PIXELS} pixels; ask for less"
                ));
            }
            Ok((w, h))
        }
        None => {
            let (fw, fh) = (f64::from(frame.0), f64::from(frame.1));
            let fit = fw.min(fh * aspect);
            let cap = (MAX_CROP_PIXELS as f64 * aspect).sqrt();
            let w = (2.0 * fit)
                .min(cap)
                .min(f64::from(MAX_SIDE))
                .min(f64::from(MAX_SIDE) * aspect)
                .floor()
                .max(1.0);
            // The height rounds, so step down until the pair is under the cap.
            let mut w = w as u32;
            while w > 1 && u64::from(w) * u64::from(side(f64::from(w))) > MAX_CROP_PIXELS {
                w -= 1;
            }
            Ok((w, side(f64::from(w))))
        }
    }
}

/// The whole page fitted inside `frame`, at 1x: what the page shows before
/// a zoom into its crop.
pub fn page_pixels(page: (f64, f64), frame: (u32, u32)) -> (u32, u32) {
    let aspect = page.0 / page.1;
    let (fw, fh) = (f64::from(frame.0), f64::from(frame.1));
    let w = fw.min(fh * aspect).floor().max(1.0);
    (w as u32, ((w / aspect).round() as u32).max(1))
}

/// `inner` re-expressed as fractions of `outer`, clipped to it, or `None`
/// when less than half of `inner`'s area lies inside.
pub fn within(inner: &Frac, outer: &Frac) -> Option<Frac> {
    let x0 = inner[0].max(outer[0]);
    let y0 = inner[1].max(outer[1]);
    let x1 = inner[2].min(outer[2]);
    let y1 = inner[3].min(outer[3]);
    let area = (inner[2] - inner[0]) * (inner[3] - inner[1]);
    if x1 <= x0 || y1 <= y0 || area <= 0.0 || (x1 - x0) * (y1 - y0) < 0.5 * area {
        return None;
    }
    let (w, h) = (outer[2] - outer[0], outer[3] - outer[1]);
    Some([
        round4((x0 - outer[0]) / w),
        round4((y0 - outer[1]) / h),
        round4((x1 - outer[0]) / w),
        round4((y1 - outer[1]) / h),
    ])
}

pub fn round4(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}

/// `x0,y0,x1,y1` as `--bbox` takes it.
pub fn fmt_box(b: &Frac) -> String {
    b.iter()
        .map(|v| format!("{}", round4(*v)))
        .collect::<Vec<_>>()
        .join(",")
}

/// `--bbox x0,y0,x1,y1`.
pub fn parse_box(text: &str) -> Result<Frac, String> {
    let parts: Vec<f64> = text
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| format!("--bbox {text}: expected four numbers x0,y0,x1,y1"))?;
    let [x0, y0, x1, y1] = parts[..] else {
        return Err(format!("--bbox {text}: expected four numbers x0,y0,x1,y1"));
    };
    if [x0, y0, x1, y1].iter().any(|v| !(0.0..=1.0).contains(v)) {
        return Err(format!(
            "--bbox {text}: each number is a fraction of the page, 0 to 1, from its top left"
        ));
    }
    Ok([x0, y0, x1, y1])
}
