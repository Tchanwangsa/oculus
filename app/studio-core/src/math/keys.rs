//! Each piece's key within its formula, so a Morph between two formulas
//! moves term by term (manim's TransformMatchingTex). A piece in a
//! `\k{name}{…}` group is `name`, then `name.1`, `name.2`, … in reading
//! order (the innermost group wins); any other piece is its label (its
//! character, `frac`, `sqrt`, …) and, from the second time that label is
//! drawn, `#n`: `x`, `x#2`. The Math adds its own prefix (`eq/x`).

use super::Item;
use super::walk::Piece;
use std::collections::HashMap;

pub fn assign(pieces: Vec<Piece<'_>>) -> Result<Vec<Item>, String> {
    let mut groups: HashMap<&str, usize> = HashMap::new();
    let mut labels: HashMap<String, usize> = HashMap::new();
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut out = Vec::with_capacity(pieces.len());
    for p in pieces {
        let group = p.groups[0];
        let key = match group {
            Some(g) => {
                let n = groups.entry(g).or_default();
                *n += 1;
                if *n == 1 {
                    g.to_owned()
                } else {
                    format!("{g}.{}", *n - 1)
                }
            }
            None => {
                let n = labels.entry(p.label.clone()).or_default();
                *n += 1;
                if *n == 1 {
                    p.label.clone()
                } else {
                    format!("{}#{n}", p.label)
                }
            }
        };
        let what = match group {
            Some(g) => format!("a term of \\k{{{g}}}"),
            None => format!("the {} drawn", p.label),
        };
        if let Some(first) = seen.insert(key.clone(), what.clone()) {
            return Err(format!(
                "two terms would both be keyed `{key}` ({first} and {what}); name the \\k group something that is not a symbol in the formula"
            ));
        }
        out.push(Item {
            key,
            groups: p.groups.iter().flatten().map(|g| (*g).to_owned()).collect(),
            rect: p.rect,
            shape: p.shape,
            colour: p.colour.map(str::to_owned),
        });
    }
    Ok(out)
}
