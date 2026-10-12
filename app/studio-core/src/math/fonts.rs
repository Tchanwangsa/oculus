//! The KaTeX fonts (`fonts/katex/`, SIL OFL 1.1, from the katex npm package
//! the app uses), read with skrifa: which face a KaTeX family, weight and
//! style select, and each character's advance and outline. They are never
//! registered with parley, so `Text` cannot name them. Glyphs need no
//! shaping: the fonts have no kern, GPOS or GSUB tables, so a browser
//! advances by `hmtx` alone, as this does.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider};
use std::sync::LazyLock;
use vello_cpu::kurbo::{BezPath, Rect, Shape};

/// A `font-family` katex.css names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Main,
    Math,
    Ams,
    Caligraphic,
    Fraktur,
    SansSerif,
    Script,
    Typewriter,
    Size1,
    Size2,
    Size3,
    Size4,
}

/// The computed font of a KaTeX element: family, `font-weight: bold` and
/// `font-style: italic`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Font {
    pub family: Family,
    pub bold: bool,
    pub italic: bool,
}

impl Font {
    /// `.katex`'s own font.
    pub const MAIN: Font = Font {
        family: Family::Main,
        bold: false,
        italic: false,
    };
}

macro_rules! faces {
    ($($file:literal),* $(,)?) => {
        /// Every bundled face, by file name.
        const FACES: &[(&str, &[u8])] = &[$(($file, include_bytes!(concat!("../../fonts/katex/", $file)))),*];
    };
}

faces!(
    "KaTeX_AMS-Regular.ttf",
    "KaTeX_Caligraphic-Bold.ttf",
    "KaTeX_Caligraphic-Regular.ttf",
    "KaTeX_Fraktur-Bold.ttf",
    "KaTeX_Fraktur-Regular.ttf",
    "KaTeX_Main-Bold.ttf",
    "KaTeX_Main-BoldItalic.ttf",
    "KaTeX_Main-Italic.ttf",
    "KaTeX_Main-Regular.ttf",
    "KaTeX_Math-BoldItalic.ttf",
    "KaTeX_Math-Italic.ttf",
    "KaTeX_SansSerif-Bold.ttf",
    "KaTeX_SansSerif-Italic.ttf",
    "KaTeX_SansSerif-Regular.ttf",
    "KaTeX_Script-Regular.ttf",
    "KaTeX_Size1-Regular.ttf",
    "KaTeX_Size2-Regular.ttf",
    "KaTeX_Size3-Regular.ttf",
    "KaTeX_Size4-Regular.ttf",
    "KaTeX_Typewriter-Regular.ttf",
);

static FONTS: LazyLock<Vec<Option<FontRef<'static>>>> = LazyLock::new(|| {
    FACES
        .iter()
        .map(|(_, bytes)| FontRef::new(bytes).ok())
        .collect()
});

/// The face a browser picks for `font`: the exact one when it exists,
/// otherwise the family's nearest (style before weight, as CSS font
/// matching does; KaTeX_Math has only italic faces).
fn face_name(font: Font) -> &'static str {
    let Font {
        family,
        bold,
        italic,
    } = font;
    match family {
        Family::Main => match (bold, italic) {
            (false, false) => "KaTeX_Main-Regular.ttf",
            (true, false) => "KaTeX_Main-Bold.ttf",
            (false, true) => "KaTeX_Main-Italic.ttf",
            (true, true) => "KaTeX_Main-BoldItalic.ttf",
        },
        Family::Math if bold => "KaTeX_Math-BoldItalic.ttf",
        Family::Math => "KaTeX_Math-Italic.ttf",
        Family::Ams => "KaTeX_AMS-Regular.ttf",
        Family::Caligraphic if bold => "KaTeX_Caligraphic-Bold.ttf",
        Family::Caligraphic => "KaTeX_Caligraphic-Regular.ttf",
        Family::Fraktur if bold => "KaTeX_Fraktur-Bold.ttf",
        Family::Fraktur => "KaTeX_Fraktur-Regular.ttf",
        Family::SansSerif if italic => "KaTeX_SansSerif-Italic.ttf",
        Family::SansSerif if bold => "KaTeX_SansSerif-Bold.ttf",
        Family::SansSerif => "KaTeX_SansSerif-Regular.ttf",
        Family::Script => "KaTeX_Script-Regular.ttf",
        Family::Typewriter => "KaTeX_Typewriter-Regular.ttf",
        Family::Size1 => "KaTeX_Size1-Regular.ttf",
        Family::Size2 => "KaTeX_Size2-Regular.ttf",
        Family::Size3 => "KaTeX_Size3-Regular.ttf",
        Family::Size4 => "KaTeX_Size4-Regular.ttf",
    }
}

fn font_ref(name: &str) -> Option<&'static FontRef<'static>> {
    let i = FACES.iter().position(|(n, _)| *n == name)?;
    FONTS.get(i)?.as_ref()
}

/// One character in one face. Lengths are in em.
pub struct Glyph {
    pub advance: f64,
    /// The outline in em, y down from the baseline; empty for a space.
    pub outline: BezPath,
    /// Which face it came from, for tests.
    #[cfg(test)]
    pub face: &'static str,
}

impl Glyph {
    pub fn ink(&self) -> Option<Rect> {
        (!self.outline.elements().is_empty()).then(|| self.outline.bounding_box())
    }
}

/// `ch` in `font`'s face; a character that face lacks comes from
/// KaTeX_Main in the same weight and style, then KaTeX_AMS, as KaTeX's
/// own fallbacks reach them. `None` when no bundled face has it.
pub fn glyph(font: Font, ch: char) -> Option<Glyph> {
    let main = Font {
        family: Family::Main,
        ..font
    };
    let ams = Font {
        family: Family::Ams,
        bold: false,
        italic: false,
    };
    [font, main, ams]
        .into_iter()
        .find_map(|f| glyph_in(face_name(f), ch))
}

fn glyph_in(name: &'static str, ch: char) -> Option<Glyph> {
    let f = font_ref(name)?;
    let gid = f.charmap().map(ch)?;
    let upem = f64::from(
        f.metrics(Size::unscaled(), LocationRef::default())
            .units_per_em,
    );
    let advance = f
        .glyph_metrics(Size::unscaled(), LocationRef::default())
        .advance_width(gid)?;
    let mut pen = Pen {
        path: BezPath::new(),
        k: 1.0 / upem.max(1.0),
    };
    if let Some(outline) = f.outline_glyphs().get(gid) {
        outline
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                &mut pen,
            )
            .ok()?;
    }
    Some(Glyph {
        advance: f64::from(advance) / upem.max(1.0),
        outline: pen.path,
        #[cfg(test)]
        face: name,
    })
}

/// Font units -> em, y flipped to grow down.
struct Pen {
    path: BezPath,
    k: f64,
}

impl Pen {
    fn p(&self, x: f32, y: f32) -> (f64, f64) {
        (f64::from(x) * self.k, -f64::from(y) * self.k)
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.path.move_to(p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.p(x, y);
        self.path.line_to(p);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.p(cx0, cy0), self.p(x, y));
        self.path.quad_to(c, p);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (a, b, p) = (self.p(cx0, cy0), self.p(cx1, cy1), self.p(x, y));
        self.path.curve_to(a, b, p);
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

#[cfg(test)]
pub(super) fn face_files() -> impl Iterator<Item = &'static str> {
    FACES.iter().map(|(n, _)| *n)
}
