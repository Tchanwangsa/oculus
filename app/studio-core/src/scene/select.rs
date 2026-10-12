//! `<Morph p>{a}{b}</Morph>` -> two plain trees, one with every Morph
//! replaced by its first child and one by its second. The chosen child's
//! root carries a `Scope` (which Morph, and its `p`), so the morph engine
//! matches inside it by key and blends it at that Morph's own `p`.

use super::parse::NodeError;
use super::{Kind, Node};

/// The Morph a subtree came from: its index in tree order and its `p`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scope {
    pub id: u32,
    pub p: f32,
}

/// The tree at every Morph's `p = 0` and at `p = 1`.
pub struct MorphSides {
    pub a: Node,
    pub b: Node,
}

/// `None` when the tree has no Morph, so it lays out as it is.
pub fn select(root: &Node) -> Result<Option<MorphSides>, NodeError> {
    if !has_morph(root) {
        return Ok(None);
    }
    let path = root.kind.name();
    let a = pick(root, 0, &mut 0, path, false)?;
    let b = pick(root, 1, &mut 0, path, false)?;
    Ok(Some(MorphSides { a, b }))
}

fn has_morph(n: &Node) -> bool {
    n.kind == Kind::Morph || n.children.iter().any(has_morph)
}

fn pick(
    n: &Node,
    side: usize,
    next: &mut u32,
    path: &str,
    inside: bool,
) -> Result<Node, NodeError> {
    let fail = |message: String| NodeError {
        message,
        path: path.to_owned(),
        loc: n.loc,
        is_style: false,
    };
    if n.kind == Kind::Morph {
        if inside {
            return Err(fail(
                "a <Morph> inside another <Morph> is not supported; give each its own subtree"
                    .to_owned(),
            ));
        }
        let [first, second] = n.children.as_slice() else {
            return Err(fail(format!(
                "<Morph> takes exactly two children (the layout at p = 0 and at p = 1), got {}",
                n.children.len()
            )));
        };
        let chosen = if side == 0 { first } else { second };
        let id = *next;
        *next += 1;
        let child_path = format!("{path} > {}[{side}]", chosen.kind.name());
        let mut c = pick(chosen, side, next, &child_path, true)?;
        c.scope = Some(Scope { id, p: n.p });
        return Ok(c);
    }
    let children = n
        .children
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let p = format!("{path} > {}[{i}]", c.kind.name());
            pick(c, side, next, &p, inside)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Node {
        kind: n.kind,
        style: n.style.clone(),
        text: n.text.clone(),
        src: n.src.clone(),
        path: n.path.clone(),
        math: n.math.clone(),
        children,
        key: n.key.clone(),
        loc: n.loc,
        p: n.p,
        scope: n.scope,
    })
}
