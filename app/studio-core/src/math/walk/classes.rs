//! What katex.css (0.18.5, the app's) says each class does, for the
//! properties layout reads: font size (`katex-sizing`), font family,
//! weight and style, `text-align` (which places lines in a vlist), and the
//! widths, margins and paddings classes give.

use super::super::fonts::{Family, Font};
use katex::ClassList;

/// KaTeX's size ladder: `sizeN` is `SIZES[N - 1]` of the normal size.
const SIZES: [f64; 11] = [0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.2, 1.44, 1.728, 2.074, 2.488];

fn numbered(classes: &ClassList, prefix: &str) -> Option<usize> {
    classes
        .iter()
        .find_map(|c| c.strip_prefix(prefix)?.parse::<usize>().ok())
        .filter(|n| (1..=SIZES.len()).contains(n))
}

/// `.katex-sizing.reset-sizeN.sizeM` (and `.fontsize-ensurer`) sets
/// `font-size` to `SIZES[M] / SIZES[N]` em.
pub fn size_factor(classes: &ClassList) -> f64 {
    if !(classes.contains("katex-sizing") || classes.contains("fontsize-ensurer")) {
        return 1.0;
    }
    match (numbered(classes, "reset-size"), numbered(classes, "size")) {
        (Some(from), Some(to)) => SIZES[to - 1] / SIZES[from - 1],
        _ => 1.0,
    }
}

/// One single-class font rule: the family, weight and style it sets.
type FontRule = (&'static str, Option<Family>, Option<bool>, Option<bool>);

/// In stylesheet order, so a later rule wins as in CSS.
const FONT_RULES: &[FontRule] = &[
    ("textbf", None, Some(true), None),
    ("textit", None, None, Some(true)),
    ("textrm", Some(Family::Main), None, None),
    ("textsf", Some(Family::SansSerif), None, None),
    ("texttt", Some(Family::Typewriter), None, None),
    ("mathnormal", Some(Family::Math), None, Some(true)),
    ("mathit", Some(Family::Main), None, Some(true)),
    ("mathrm", None, None, Some(false)),
    ("mathbf", Some(Family::Main), Some(true), None),
    ("boldsymbol", Some(Family::Math), Some(true), Some(true)),
    ("amsrm", Some(Family::Ams), None, None),
    ("mathbb", Some(Family::Ams), None, None),
    ("textbb", Some(Family::Ams), None, None),
    ("mathcal", Some(Family::Caligraphic), None, None),
    ("mathfrak", Some(Family::Fraktur), None, None),
    ("textfrak", Some(Family::Fraktur), None, None),
    ("mathboldfrak", Some(Family::Fraktur), Some(true), None),
    ("textboldfrak", Some(Family::Fraktur), Some(true), None),
    ("mathtt", Some(Family::Typewriter), None, None),
    ("mathscr", Some(Family::Script), None, None),
    ("textscr", Some(Family::Script), None, None),
    ("mathsf", Some(Family::SansSerif), None, None),
    ("mathboldsf", Some(Family::SansSerif), Some(true), None),
    ("textboldsf", Some(Family::SansSerif), Some(true), None),
    ("mathsfit", Some(Family::SansSerif), None, Some(true)),
    ("mathitsf", Some(Family::SansSerif), None, Some(true)),
    ("textitsf", Some(Family::SansSerif), None, Some(true)),
    ("mainrm", Some(Family::Main), None, Some(false)),
];

const SIZE_FAMILIES: [Family; 4] = [Family::Size1, Family::Size2, Family::Size3, Family::Size4];

/// The element's font from its parent's and its classes. The two-class
/// rules (`.delimsizing.sizeN`, `.op-symbol.small-op`) outrank the rest.
pub fn font(mut f: Font, classes: &ClassList) -> Font {
    for &(class, family, bold, italic) in FONT_RULES {
        if !classes.contains(class) {
            continue;
        }
        if let Some(family) = family {
            f.family = family;
        }
        if let Some(bold) = bold {
            f.bold = bold;
        }
        if let Some(italic) = italic {
            f.italic = italic;
        }
    }
    if classes.contains("delimsizing")
        && let Some(n) = numbered(classes, "size").filter(|n| *n <= 4)
    {
        f.family = SIZE_FAMILIES[n - 1];
    }
    if classes.contains("op-symbol") {
        if classes.contains("small-op") {
            f.family = Family::Size1;
        } else if classes.contains("large-op") {
            f.family = Family::Size2;
        }
    }
    f
}

/// `.delimsizing.mult .delim-sizeN > span`: the family this element gives
/// its child spans.
pub fn child_family(classes: &ClassList) -> Option<Family> {
    if classes.contains("delim-size1") {
        Some(Family::Size1)
    } else if classes.contains("delim-size4") {
        Some(Family::Size4)
    } else {
        None
    }
}

/// `text-align` set on this element (0 left, 0.5 centre, 1 right); it is
/// inherited, and places each line of a vlist in the vlist's width.
pub fn align(classes: &ClassList) -> Option<f64> {
    const LEFT: &[&str] = &["msupsub", "svg-align", "col-align-l"];
    const CENTRE: &[&str] = &[
        "mfrac",
        "op-limits",
        "katex-accent",
        "x-arrow",
        "mover",
        "munder",
        "col-align-c",
    ];
    if classes.contains("col-align-r") {
        Some(1.0)
    } else if CENTRE.iter().any(|c| classes.contains(c)) {
        Some(0.5)
    } else if LEFT.iter().any(|c| classes.contains(c)) {
        Some(0.0)
    } else {
        None
    }
}

/// Widths, margins and paddings from classes, in the element's em.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClassBox {
    pub width: Option<f64>,
    pub margin: (f64, f64),
    pub padding: (f64, f64),
}

pub fn class_box(classes: &ClassList) -> ClassBox {
    let mut b = ClassBox::default();
    if classes.contains("nulldelimiter") {
        b.width = Some(0.12);
    }
    if classes.contains("katex-thinbox")
        || (classes.contains("accent-body") && !classes.contains("accent-full"))
    {
        b.width = Some(0.0);
    }
    if classes.contains("katex-root") {
        b.margin = (0.2777777778, -0.5555555556);
    }
    if classes.contains("cancel-lap") {
        b.margin = (-0.2, -0.2);
    }
    let pads: [(&str, f64, f64); 5] = [
        ("boxpad", 0.3, 0.3),
        ("cancel-pad", 0.2, 0.2),
        ("x-arrow-pad", 0.5, 0.5),
        ("cd-arrow-pad", 0.27778, 0.55556),
        ("anglpad", 0.03889, 0.03889),
    ];
    for (class, left, right) in pads {
        if classes.contains(class) {
            b.padding = (left, right);
        }
    }
    b
}
