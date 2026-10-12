//! Per-property blends of a matched pair: rects, numbers, colours (OKLab),
//! fills, borders, shadows, transforms, clipInset and reveal. Each is exact
//! at p = 0 and 1, and a property equal on both sides is left as it is.

use super::colour::{MorphColor, clear, mix};
use crate::scene::{Border, Color, Fill, Inset, Origin, Reveal, Shadow, Style};
use vello_cpu::kurbo::Rect;

pub fn lerp(a: f32, b: f32, p: f32) -> f32 {
    a + (b - a) * p
}

pub fn lerp64(a: f64, b: f64, p: f32) -> f64 {
    a + (b - a) * f64::from(p)
}

pub fn rect(a: Rect, b: Rect, p: f32) -> Rect {
    if a == b {
        return b;
    }
    Rect::new(
        lerp64(a.x0, b.x0, p),
        lerp64(a.y0, b.y0, p),
        lerp64(a.x1, b.x1, p),
        lerp64(a.y1, b.y1, p),
    )
}

fn num(a: Option<f32>, b: Option<f32>, default: f32, p: f32) -> Option<f32> {
    if a == b {
        return b;
    }
    Some(lerp(a.unwrap_or(default), b.unwrap_or(default), p))
}

/// The paint-relevant style of a matched pair at `p`; layout keys are B's
/// (layout is already done). `text` is the theme's default text colour.
/// Geometry (radius, transforms) follows `p` past 0..1 when an easing
/// overshoots; opacity, blur, insets and colours stop at the ends.
pub fn style(a: &Style, b: &Style, p: f32, text: Color, c: MorphColor) -> Style {
    let mut s = b.clone();
    let q = p.clamp(0.0, 1.0);
    s.opacity = num(a.opacity, b.opacity, 1.0, q);
    s.radius = num(a.radius, b.radius, 0.0, p).map(|r| r.max(0.0));
    s.blur = num(a.blur, b.blur, 0.0, q);
    s.translate_x = num(a.translate_x, b.translate_x, 0.0, p);
    s.translate_y = num(a.translate_y, b.translate_y, 0.0, p);
    s.scale = num(a.scale, b.scale, 1.0, p);
    s.rotate = num(a.rotate, b.rotate, 0.0, p);
    if a.transform_origin != b.transform_origin {
        let centre = Origin { x: 0.5, y: 0.5 };
        let (oa, ob) = (
            a.transform_origin.unwrap_or(centre),
            b.transform_origin.unwrap_or(centre),
        );
        s.transform_origin = Some(Origin {
            x: lerp(oa.x, ob.x, p),
            y: lerp(oa.y, ob.y, p),
        });
    }
    if a.clip_inset != b.clip_inset {
        let none = Inset([0.0; 4]);
        let (Inset(ia), Inset(ib)) = (a.clip_inset.unwrap_or(none), b.clip_inset.unwrap_or(none));
        s.clip_inset = Some(Inset([0, 1, 2, 3].map(|i| lerp(ia[i], ib[i], q))));
    }
    s.fill = fill(a.fill.as_ref(), b.fill.as_ref(), q, c);
    s.border = border(a.border.as_ref(), b.border.as_ref(), q, c);
    s.shadow = shadow(a.shadow.as_ref(), b.shadow.as_ref(), q, c);
    if a.color != b.color {
        s.color = Some(mix(a.color.unwrap_or(text), b.color.unwrap_or(text), q, c));
    }
    if q < 0.5 {
        s.clip = a.clip;
    }
    s.reveal = reveal(a.reveal, b.reveal, q);
    s
}

pub fn fill(a: Option<&Fill>, b: Option<&Fill>, p: f32, c: MorphColor) -> Option<Fill> {
    if a == b || p >= 1.0 {
        return b.cloned();
    }
    if p <= 0.0 {
        return a.cloned();
    }
    match (a, b) {
        (Some(Fill::Solid(x)), Some(Fill::Solid(y))) => Some(Fill::Solid(mix(*x, *y, p, c))),
        (None, Some(Fill::Solid(y))) => Some(Fill::Solid(mix(clear(*y), *y, p, c))),
        (Some(Fill::Solid(x)), None) => Some(Fill::Solid(mix(*x, clear(*x), p, c))),
        _ => Some(gradient(a, b, p, c)),
    }
}

/// Two fills, at least one a gradient: both become gradients on the union
/// of their stop offsets (a colour is one flat gradient, no fill a clear
/// one), blended stop by stop, the angle turning the short way.
fn gradient(a: Option<&Fill>, b: Option<&Fill>, p: f32, c: MorphColor) -> Fill {
    let angle = |f: Option<&Fill>| match f {
        Some(Fill::Linear { angle_deg, .. }) => Some(*angle_deg),
        _ => None,
    };
    let (aa, ab) = (angle(a), angle(b));
    let (aa, ab) = (aa.or(ab).unwrap_or(0.0), ab.or(aa).unwrap_or(0.0));
    let stops = |f: Option<&Fill>| -> Vec<(f32, Color)> {
        match f {
            Some(Fill::Linear { stops, .. }) => stops.clone(),
            Some(Fill::Solid(c)) => vec![(0.0, *c), (1.0, *c)],
            None => vec![(0.0, Color::CLEAR), (1.0, Color::CLEAR)],
        }
    };
    let (sa, sb) = (stops(a), stops(b));
    let same_offsets = sa.len() == sb.len() && sa.iter().zip(&sb).all(|(x, y)| x.0 == y.0);
    let stops = if same_offsets {
        sa.iter()
            .zip(&sb)
            .map(|(x, y)| (y.0, mix(x.1, y.1, p, c)))
            .collect()
    } else {
        let mut offsets: Vec<f32> = sa.iter().chain(&sb).map(|s| s.0).collect();
        offsets.sort_by(f32::total_cmp);
        offsets.dedup();
        offsets
            .into_iter()
            .map(|o| (o, mix(sample(&sa, o), sample(&sb, o), p, c)))
            .collect()
    };
    let turn = (ab - aa + 540.0).rem_euclid(360.0) - 180.0;
    Fill::Linear {
        angle_deg: aa + turn * p,
        stops,
    }
}

/// A gradient's colour at an offset, straight sRGB between stops.
fn sample(stops: &[(f32, Color)], o: f32) -> Color {
    let mut prev = stops[0];
    for &s in stops {
        if o <= s.0 {
            if s.0 <= prev.0 {
                return s.1;
            }
            let t = (o - prev.0) / (s.0 - prev.0);
            let (x, y) = (prev.1, s.1);
            return Color {
                r: lerp(x.r, y.r, t),
                g: lerp(x.g, y.g, t),
                b: lerp(x.b, y.b, t),
                a: lerp(x.a, y.a, t),
            };
        }
        prev = s;
    }
    prev.1
}

/// A border that appears or goes grows from or shrinks to zero width.
fn border(a: Option<&Border>, b: Option<&Border>, p: f32, c: MorphColor) -> Option<Border> {
    if a == b {
        return b.cloned();
    }
    let (wa, wb) = (a.map_or(0.0, |x| x.width), b.map_or(0.0, |x| x.width));
    let (ca, cb) = match (a, b) {
        (Some(x), Some(y)) => (x.color, y.color),
        (Some(x), None) => (x.color, x.color),
        (None, Some(y)) => (y.color, y.color),
        (None, None) => return None,
    };
    Some(Border {
        width: lerp(wa, wb, p),
        color: mix(ca, cb, p, c),
    })
}

/// A shadow that appears or goes keeps its geometry and fades.
fn shadow(a: Option<&Shadow>, b: Option<&Shadow>, p: f32, c: MorphColor) -> Option<Shadow> {
    if a == b {
        return b.cloned();
    }
    let (sa, sb) = match (a, b) {
        (Some(x), Some(y)) => (x.clone(), y.clone()),
        (Some(x), None) => (
            x.clone(),
            Shadow {
                color: clear(x.color),
                ..x.clone()
            },
        ),
        (None, Some(y)) => (
            Shadow {
                color: clear(y.color),
                ..y.clone()
            },
            y.clone(),
        ),
        (None, None) => return None,
    };
    Some(Shadow {
        x: lerp(sa.x, sb.x, p),
        y: lerp(sa.y, sb.y, p),
        blur: lerp(sa.blur, sb.blur, p),
        spread: lerp(sa.spread, sb.spread, p),
        color: mix(sa.color, sb.color, p, c),
    })
}

/// A missing reveal is a finished one.
fn reveal(a: Option<Reveal>, b: Option<Reveal>, p: f32) -> Option<Reveal> {
    if a == b {
        return b;
    }
    let (ra, rb) = match (a, b) {
        (Some(x), Some(y)) => (x, y),
        (Some(x), None) => (x, Reveal { progress: 1.0, ..x }),
        (None, Some(y)) => (Reveal { progress: 1.0, ..y }, y),
        (None, None) => return None,
    };
    Some(Reveal {
        progress: lerp(ra.progress, rb.progress, p),
        stagger: lerp(ra.stagger, rb.stagger, p),
        ..rb
    })
}

/// Fades a node's own paint (fill, border, shadow) without a group, for an
/// unmatched box whose matched descendants must stay fully visible.
pub fn fade_own(s: &mut Style, k: f32) {
    if k >= 1.0 {
        return;
    }
    s.fill = s.fill.take().map(|f| match f {
        Fill::Solid(c) => Fill::Solid(c.with_alpha_mul(k)),
        Fill::Linear { angle_deg, stops } => Fill::Linear {
            angle_deg,
            stops: stops
                .into_iter()
                .map(|(o, c)| (o, c.with_alpha_mul(k)))
                .collect(),
        },
    });
    if let Some(b) = &mut s.border {
        b.color = b.color.with_alpha_mul(k);
    }
    if let Some(sh) = &mut s.shadow {
        sh.color = sh.color.with_alpha_mul(k);
    }
}

/// Multiplies a node's group opacity.
pub fn fade(s: &mut Style, k: f32) {
    if k < 1.0 {
        s.opacity = Some(s.opacity() * k);
    }
}
