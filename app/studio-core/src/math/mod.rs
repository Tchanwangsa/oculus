//! `<Math tex display>`: LaTeX typeset by the maths engine's KaTeX port
//! (`app/math-core/katex`) and, before layout, expanded into a Box of
//! absolutely placed children: a filled Path per glyph (its outline from
//! the bundled KaTeX fonts), a filled Box per rule, a Path per KaTeX SVG.
//! Every child is keyed, so a `<Morph>` between two formulas moves the
//! terms they share and fades the rest (`keys`).
//!
//! `tex` calls KaTeX; `walk` lays its HTML tree out as katex.css would;
//! `keys` names the pieces; `expand` finds Math elements in a scene tree
//! and `build` turns a typeset formula into nodes; `colour` resolves
//! `\color`; `cache` keeps typeset formulas across frames.

mod build;
mod cache;
mod colour;
mod expand;
mod fonts;
mod keys;
mod tex;
mod walk;

pub use cache::MathCache;
pub use expand::{MathEnv, expand};

use serde_json::{Map, Value};
use std::sync::Arc;
use vello_cpu::kurbo::{BezPath, Rect};

/// `fontSize` is the size of the text a formula sits with; KaTeX sets
/// maths this much larger (`.katex { font-size: 1.21em }`), as the app's
/// own maths does.
pub const KATEX_SCALE: f64 = 1.21;

/// A typeset formula in em of its own size, y down from the baseline.
#[derive(Debug)]
pub struct Formula {
    pub width: f64,
    /// Above the baseline.
    pub height: f64,
    /// Below the baseline.
    pub depth: f64,
    /// In reading order.
    pub items: Vec<Item>,
}

#[derive(Debug)]
pub struct Item {
    /// Unique within the formula.
    pub key: String,
    pub rect: Rect,
    pub shape: Shape,
    /// The colour `\color` gave it, as written; `None` takes the Math's.
    pub colour: Option<String>,
}

#[derive(Debug)]
pub enum Shape {
    Glyph(GlyphShape),
    /// A filled rectangle (fraction bars, over- and underlines, `\rule`,
    /// box frames).
    Rule,
    /// Path data in em relative to the item's rect, filled, or stroked at
    /// a width in em.
    Path {
        d: BezPath,
        stroke: Option<f64>,
    },
}

#[derive(Debug)]
pub struct GlyphShape {
    /// In em at the glyph's origin, the same data wherever the glyph is
    /// drawn, so two copies morph as one shape.
    pub outline: Arc<BezPath>,
    /// The outline's box (advance by height and depth), which the item's
    /// rect is a scaled copy of.
    pub view: Rect,
}

/// `tex` typeset and keyed.
pub fn typeset(tex: &str, display: bool) -> Result<Formula, String> {
    let tree = tex::html_tree(tex, display)?;
    let (mut formula, pieces) = walk::walk(&tree)?;
    formula.items = keys::assign(pieces)?;
    Ok(formula)
}

/// A `latex()` prop's check: `tex` typesets, and every `\color` names a
/// colour in `colours` (the theme's) or a hex colour.
pub fn check(tex: &str, colours: Option<&Map<String, Value>>) -> Result<(), String> {
    let formula = typeset(tex, true)?;
    for item in &formula.items {
        if let Some(c) = &item.colour {
            colour::resolve(c, colours)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
