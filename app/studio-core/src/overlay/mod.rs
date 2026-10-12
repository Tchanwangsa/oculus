//! Overlays bound to keys: `Brace`, `Surround`, `Underline`, `Indicate` and
//! a keyed `Arrow`. They are drawn on the finished tree, after layout and
//! after any morph has blended it, so they follow their targets while those
//! move. A target is a key or a key prefix (`eq/lhs` covers `eq/lhs`,
//! `eq/lhs.1`, … and `eq/lhs/…`), and its box is the union of the matching
//! nodes' boxes, through every transform above them, mapped into the
//! overlay's own place in the tree.
//!
//! Indicate goes first and changes its targets' own style (a scale pulse
//! about their joint centre and a flash of colour), so the other overlays
//! measure the pulsed boxes. Each drawing overlay then becomes a Box of
//! Paths (and its label, moved into place); one a morph blended from
//! another (`Placed::was`) sits between the two targets' boxes.

mod arrow;
mod draw;
mod indicate;
mod targets;

use crate::layout::LNode;
use crate::scene::{Color, Kind, OverlayKind};
use targets::Index;
use vello_cpu::kurbo::Affine;

/// A target no key matches: the message and the overlay's TSX position.
#[derive(Debug)]
pub struct OverlayError {
    pub message: String,
    pub loc: Option<(u32, u32)>,
}

/// The theme's colours an overlay defaults to.
#[derive(Clone, Copy, Debug)]
pub struct OverlayInk {
    pub text: Color,
    pub accent: Color,
}

pub fn resolve(root: &mut LNode, ink: OverlayInk) -> Result<(), OverlayError> {
    if !has_overlay(root) {
        return Ok(());
    }
    indicate::apply(root, ink)?;
    let index = Index::new(root);
    draw_all(root, Affine::IDENTITY, &index, ink)
}

fn has_overlay(n: &LNode) -> bool {
    n.overlay.is_some() || n.children.iter().any(has_overlay)
}

/// Draws every overlay below `n`; `parent` is the frame transform of
/// `n`'s parent.
fn draw_all(
    n: &mut LNode,
    parent: Affine,
    index: &Index,
    ink: OverlayInk,
) -> Result<(), OverlayError> {
    if let Some(placed) = n.overlay.take() {
        if !matches!(placed.now.kind, OverlayKind::Indicate { .. }) {
            draw::overlay(n, &placed, parent, index, ink)?;
        }
        n.kind = Kind::Box;
    }
    let t = parent * crate::paint::local_transform(n);
    for c in &mut n.children {
        draw_all(c, t, index, ink)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
