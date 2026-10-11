//! Which node of A becomes which node of B. Roots match, Morph scope roots
//! match their counterparts, and a key matches the same key anywhere in the
//! other tree (inside the same Morph), which is what lets a card move from
//! one row to another. Inside a matched pair, the unkeyed children match by
//! index only when both sides have the same unkeyed structure (count and
//! kinds); otherwise they are unmatched and fade.

use super::MorphError;
use super::flat::Tree;
use std::collections::HashMap;

pub struct Matching {
    pub a_to_b: Vec<Option<usize>>,
    pub b_to_a: Vec<Option<usize>>,
}

impl Matching {
    fn pair(&mut self, ia: usize, ib: usize, work: &mut Vec<(usize, usize)>) {
        if self.a_to_b[ia].is_none() && self.b_to_a[ib].is_none() {
            self.a_to_b[ia] = Some(ib);
            self.b_to_a[ib] = Some(ia);
            work.push((ia, ib));
        }
    }
}

pub fn match_trees(a: &Tree, b: &Tree) -> Result<Matching, MorphError> {
    let (keys_a, keys_b) = (a.keys()?, b.keys()?);
    let mut m = Matching {
        a_to_b: vec![None; a.slots.len()],
        b_to_a: vec![None; b.slots.len()],
    };
    let mut work = Vec::new();
    m.pair(0, 0, &mut work);
    let roots = |t: &Tree| -> HashMap<u32, usize> {
        let slots = t.slots.iter().enumerate();
        slots
            .filter(|(_, s)| s.scope_root)
            .filter_map(|(i, s)| Some((s.scope?, i)))
            .collect()
    };
    let scopes_b = roots(b);
    let mut scope_pairs: Vec<(usize, usize)> = roots(a)
        .into_iter()
        .filter_map(|(id, ia)| Some((ia, *scopes_b.get(&id)?)))
        .collect();
    scope_pairs.sort_unstable();
    for (ia, ib) in scope_pairs {
        m.pair(ia, ib, &mut work);
    }
    let mut key_pairs: Vec<(usize, usize)> = keys_a
        .iter()
        .filter_map(|(k, &ia)| {
            let ib = *keys_b.get(k)?;
            (a.slots[ia].scope == b.slots[ib].scope).then_some((ia, ib))
        })
        .collect();
    key_pairs.sort_unstable();
    for (ia, ib) in key_pairs {
        m.pair(ia, ib, &mut work);
    }
    let free = |t: &Tree, i: usize| -> Vec<usize> {
        let kids = t.slots[i].kids.iter().copied();
        kids.filter(|&k| t.key(k).is_none() && !t.slots[k].scope_root)
            .collect()
    };
    while let Some((ia, ib)) = work.pop() {
        let (ua, ub) = (free(a, ia), free(b, ib));
        let same = ua.len() == ub.len()
            && ua
                .iter()
                .zip(&ub)
                .all(|(&x, &y)| a.slots[x].kind == b.slots[y].kind);
        if same {
            for (x, y) in ua.into_iter().zip(ub) {
                m.pair(x, y, &mut work);
            }
        }
    }
    Ok(m)
}
