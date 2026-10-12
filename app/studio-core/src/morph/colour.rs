//! Colour blending in OKLab (Björn Ottosson's), with alpha premultiplied
//! so a transparent end fades without tinting the other. The `morphColor`
//! motion token picks the path: `oklch` (default) is polar between two
//! saturated colours, chroma interpolating and the hue turning the short
//! way, so near-complementary colours (indigo and orange) keep their chroma
//! instead of crossing grey; with a near-grey end, whose hue means nothing,
//! it is the straight line. `oklab` is always the straight OKLab line.

use crate::scene::Color;
use std::f64::consts::PI;

/// How a morph blends colours (the `morphColor` motion token).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MorphColor {
    #[default]
    Oklch,
    Oklab,
}

/// Lightness, a, b.
pub type Lab = [f64; 3];

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f64) -> f64 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn to_oklab(c: Color) -> Lab {
    let [r, g, b] = [c.r, c.g, c.b].map(|v| to_linear(f64::from(v)));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// Back to straight sRGB, clipped to the gamut.
pub fn from_oklab([lightness, a, b]: Lab, alpha: f32) -> Color {
    let l = (lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m = (lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s = (lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    let r = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
    let g = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
    let bl = -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s;
    let ch = |v: f64| to_srgb(v.clamp(0.0, 1.0)).clamp(0.0, 1.0) as f32;
    Color {
        r: ch(r),
        g: ch(g),
        b: ch(bl),
        a: alpha,
    }
}

/// Chroma: distance from the grey axis.
#[cfg(test)]
pub fn chroma(c: Color) -> f64 {
    let [_, a, b] = to_oklab(c);
    a.hypot(b)
}

/// `a` at p = 0 to `b` at p = 1, exact at both ends.
pub fn mix(a: Color, b: Color, p: f32, space: MorphColor) -> Color {
    if a == b || p <= 0.0 {
        return a;
    }
    if p >= 1.0 {
        return b;
    }
    let (p, q) = (f64::from(p), 1.0 - f64::from(p));
    let (wa, wb) = (f64::from(a.a) * q, f64::from(b.a) * p);
    let alpha = wa + wb;
    if alpha <= 0.0 {
        return Color { a: 0.0, ..b };
    }
    let (la, lb) = (to_oklab(a), to_oklab(b));
    let (ca, cb) = (la[1].hypot(la[2]), lb[1].hypot(lb[2]));
    let avg = |x: f64, y: f64| (x * wa + y * wb) / alpha;
    let lightness = avg(la[0], lb[0]);
    if space == MorphColor::Oklab || ca < GREY || cb < GREY {
        return from_oklab(
            [lightness, avg(la[1], lb[1]), avg(la[2], lb[2])],
            alpha as f32,
        );
    }
    let (ha, hb) = (la[2].atan2(la[1]), lb[2].atan2(lb[1]));
    let turn = (hb - ha + 3.0 * PI).rem_euclid(2.0 * PI) - PI;
    let h = ha + turn * wb / alpha;
    let c = avg(ca, cb);
    from_oklab([lightness, c * h.cos(), c * h.sin()], alpha as f32)
}

/// OKLab chroma below which a colour counts as grey (its hue is noise).
const GREY: f64 = 0.02;

/// The same colour, fully transparent: a fade end that keeps its hue.
pub fn clear(c: Color) -> Color {
    Color { a: 0.0, ..c }
}
