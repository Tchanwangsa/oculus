//! Our layout against KaTeX's own in WebKit (the app's engine): the app's
//! katex 0.18.5 and katex.css at 200 px per em, measured in a WKWebView
//! (each character's range rect and its line's baseline, each rule's
//! border and each svg's visible box, in em from the first base's left
//! and the baseline). Ours must agree to within `TOL`: WebKit rounds each
//! character's rect to about a pixel, 0.005 em at that size.

use crate::math::{Shape, typeset};
use vello_cpu::kurbo::Rect;

const TOL: f64 = 0.006;

type Glyph = (&'static str, f64, f64, f64);
/// A glyph of ours: character, left edge, baseline, size.
type Placed = (String, f64, f64, f64);
type Case = (&'static str, f64, &'static [Glyph], &'static [[f64; 4]]);

#[rustfmt::skip]
const CASES: &[Case] = &[
    (
        r#"\frac{1}{1 + \frac{1}{x}}"#,
        2.603,
        &[("1", 0.1201, 0.7351, 1.0), ("+", 0.8423, 0.7351, 1.0), ("x", 1.9625, 1.0801, 0.7), ("1", 1.9877, 0.3411, 0.7), ("1", 1.0515, -0.677, 1.0)],
        &[[1.9624, 0.4651, 2.3629, 0.5051], [0.12, -0.2701, 2.483, -0.2301]],
    ),
    (
        r#"x^{2} + y_{i}^{n} - e^{-x^2}"#,
        6.1739,
        &[("x", 0.0, 0.0, 1.0), ("2", 0.572, -0.4139, 0.7), ("+", 1.1942, 0.0, 1.0), ("y", 2.1945, 0.0, 1.0), ("i", 2.6845, 0.247, 0.7), ("n", 2.7204, -0.413, 0.7), ("−", 3.4126, 0.0, 1.0), ("e", 4.413, 0.0019, 1.0), ("−", 4.8789, -0.4092, 0.7), ("x", 5.4235, -0.4092, 0.7), ("2", 5.8239, -0.7119, 0.5)],
        &[],
    ),
    (
        r#"\sqrt[3]{8}"#,
        1.3052,
        &[("3", 0.2777, -0.5228, 0.5), ("8", 0.8052, 0.0, 1.0)],
        &[[-0.0278, -0.9961, 1.3052, 0.0839]],
    ),
    (
        r#"\left( \frac{a}{b} \right)"#,
        1.9631,
        &[("(", 0.0, 0.0, 1.0), ("b", 0.767, 0.686, 1.0), ("a", 0.717, -0.677, 1.0), (")", 1.3661, 0.0, 1.0)],
        &[[0.717, -0.27, 1.246, -0.23]],
    ),
    (
        r#"\sum_{i=1}^{n} i"#,
        1.9558,
        &[("i", 0.1539, 1.1777, 0.7), ("=", 0.3954, 1.1777, 0.7), ("1", 0.94, 1.1777, 0.7), ("∑", 0.0, 0.0, 1.0), ("n", 0.512, -1.25, 0.7), ("i", 1.6107, 0.0, 1.0)],
        &[],
    ),
    (
        r#"\begin{pmatrix} a & b \\ c & d \end{pmatrix}"#,
        3.5211,
        &[("(", 0.0, 0.0, 1.0), ("a", 0.736, -0.61, 1.0), ("c", 0.7839, 0.59, 1.0), ("b", 2.3105, -0.61, 1.0), ("d", 2.265, 0.59, 1.0), (")", 2.7851, 0.0, 1.0)],
        &[],
    ),
    (
        r#"\underbrace{a+b}_{2}"#,
        2.1805,
        &[("2", 0.9152, 1.3825, 0.7), ("a", 0.0001, 0.0001, 1.0), ("+", 0.7513, 0.0001, 1.0), ("b", 1.7515, 0.0001, 1.0)],
        &[[0.0, 0.1834, 0.5473, 0.7314], [0.5451, 0.1834, 1.6353, 0.7314], [1.6333, 0.1834, 2.1805, 0.7314]],
    ),
    (
        r#"\left( \begin{matrix} a\\b\\c\\d\\e\\f \end{matrix} \right)"#,
        2.3476,
        &[("a", 0.9093, -3.01, 1.0), ("b", 0.9593, -1.81, 1.0), ("c", 0.9573, -0.61, 1.0), ("d", 0.9137, 0.59, 1.0), ("e", 0.9408, 1.79, 1.0), ("f", 0.875, 2.99, 1.0)],
        &[[0.0, -3.8498, 0.875, 3.3502], [1.4726, -3.8498, 2.3476, 3.3502]],
    ),
    (
        r#"\int_0^1 f(x)\,dx"#,
        4.7735,
        &[("∫", 0.0, -0.0011, 1.0), ("0", 0.5559, 0.9119, 0.7), ("1", 1.0004, -1.1129, 0.7), ("f", 1.5671, -0.0001, 1.0), ("(", 2.1647, -0.0001, 1.0), ("x", 2.5537, -0.0001, 1.0), (")", 3.1257, -0.0001, 1.0), ("d", 3.6813, -0.0001, 1.0), ("x", 4.2013, -0.0001, 1.0)],
        &[],
    ),
    (
        r#"\boxed{x+1}"#,
        2.8945,
        &[("x", 0.3, -0.0001, 1.0), ("+", 1.0942, -0.0001, 1.0), ("1", 2.0944, -0.0001, 1.0)],
        &[[0.0, -0.9844, 2.8945, 0.4233]],
    ),
    (
        r#"\mathllap{x}y \mathrlap{z}w"#,
        1.2688,
        &[("x", -0.572, 0.0, 1.0), ("y", 0.0, 0.0, 1.0), ("z", 0.5259, 0.0, 1.0), ("w", 0.5259, 0.0, 1.0)],
        &[],
    ),
    (
        r#"\binom{n}{k} \cancel{x}"#,
        2.6441,
        &[("(", 0.0, 0.0, 1.0), ("k", 0.7597, 0.686, 1.0), ("n", 0.7359, -0.677, 1.0), (")", 1.336, 0.0, 1.0), ("x", 2.072, 0.0, 1.0)],
        &[[2.072, -0.6305, 2.644, 0.2]],
    ),
];

/// Our glyphs as (character, left edge, baseline, size) and our other
/// pieces' rects, a box frame's four sides as one rect.
fn ours(tex: &str) -> (f64, Vec<Placed>, Vec<Rect>) {
    let f = typeset(tex, true).unwrap();
    let mut glyphs = Vec::new();
    let mut boxes = Vec::new();
    let mut frame: Option<Rect> = None;
    for it in &f.items {
        let r = it.rect;
        match &it.shape {
            Shape::Glyph(g) => {
                let s = r.height() / g.view.height();
                // A glyph's key is its character, then script marks and `#n`.
                let ch = it.key.chars().next().unwrap().to_string();
                glyphs.push((ch, r.x0 - g.view.x0 * s, r.y0 - g.view.y0 * s, s));
            }
            _ if it.key.starts_with("box") => {
                frame = Some(frame.map_or(r, |f| f.union(r)));
            }
            _ => boxes.push(r),
        }
    }
    boxes.extend(frame);
    (f.width, glyphs, boxes)
}

#[test]
fn glyphs_rules_and_svg_sit_where_webkit_puts_them() {
    for &(tex, width, glyphs, rects) in CASES {
        let (w, mut ours_g, mut ours_r) = ours(tex);
        assert!((w - width).abs() < TOL, "{tex}: width {w} vs {width}");
        assert_eq!(ours_g.len(), glyphs.len(), "{tex}: glyph count");
        for &(ch, x, base, size) in glyphs {
            let i = ours_g
                .iter()
                .enumerate()
                .filter(|(_, g)| g.0 == ch)
                .min_by(|a, b| {
                    let d = |g: &Placed| (g.1 - x).abs() + (g.2 - base).abs();
                    d(a.1).total_cmp(&d(b.1))
                })
                .map(|(i, _)| i)
                .unwrap_or_else(|| panic!("{tex}: no {ch}"));
            let g = ours_g.remove(i);
            let off = (g.1 - x)
                .abs()
                .max((g.2 - base).abs())
                .max((g.3 - size).abs());
            assert!(
                off < TOL,
                "{tex}: {ch} at ({}, {}) size {}, WebKit ({x}, {base}) size {size}",
                g.1,
                g.2,
                g.3
            );
        }
        assert_eq!(ours_r.len(), rects.len(), "{tex}: rules and svg");
        for &[x0, y0, x1, y1] in rects {
            let want = Rect::new(x0, y0, x1, y1);
            let d = |r: &Rect| {
                (r.x0 - want.x0)
                    .abs()
                    .max((r.y0 - want.y0).abs())
                    .max((r.x1 - want.x1).abs())
                    .max((r.y1 - want.y1).abs())
            };
            let i = (0..ours_r.len())
                .min_by(|&a, &b| d(&ours_r[a]).total_cmp(&d(&ours_r[b])))
                .unwrap();
            let r = ours_r.remove(i);
            assert!(d(&r) < TOL, "{tex}: {r:?} vs WebKit {want:?}");
        }
    }
}
