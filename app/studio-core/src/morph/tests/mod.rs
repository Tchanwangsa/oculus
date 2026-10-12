//! The morph engine on hand-built trees: colour, matching and the built
//! tree's order, fades and geometry.

mod build;
mod colour;
mod matching;

use crate::layout::LNode;
use crate::scene::{Color, Fill, Kind, Style};
use vello_cpu::kurbo::Rect;

/// A box at (x, y, w, h) with children.
fn bx(x: f64, y: f64, w: f64, h: f64, kids: Vec<LNode>) -> LNode {
    let r = Rect::new(x, y, x + w, y + h);
    LNode {
        kind: Kind::Box,
        style: Style {
            fill: Some(Fill::Solid(Color::BLACK)),
            ..Style::default()
        },
        rect: r,
        content: r,
        text: None,
        image: None,
        path: None,
        children: kids,
        key: None,
        loc: None,
        scope: None,
    }
}

fn keyed(k: &str, mut n: LNode) -> LNode {
    n.key = Some(k.to_owned());
    n
}

fn leaf(x: f64, y: f64) -> LNode {
    bx(x, y, 10.0, 10.0, Vec::new())
}
