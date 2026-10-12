use super::{bx, keyed, leaf};
use crate::layout::LNode;
use crate::morph::flat::Tree;
use crate::morph::matching::match_trees;
use crate::scene::Kind;

fn pairs(a: LNode, b: LNode) -> Vec<Option<usize>> {
    let (a, b) = (
        Tree::new(a, false, 0.5, true),
        Tree::new(b, false, 0.5, false),
    );
    match_trees(&a, &b).unwrap().a_to_b
}

#[test]
fn a_key_matches_across_depth() {
    // Preorder: root 0, row 1, card 2, row 3 (A); root 0, row 1, row 2, card 3 (B).
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![
            bx(0.0, 0.0, 100.0, 50.0, vec![keyed("card", leaf(0.0, 0.0))]),
            bx(0.0, 50.0, 100.0, 50.0, vec![]),
        ],
    );
    let b = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![
            bx(0.0, 0.0, 100.0, 50.0, vec![]),
            bx(0.0, 50.0, 100.0, 50.0, vec![keyed("card", leaf(0.0, 50.0))]),
        ],
    );
    assert_eq!(pairs(a, b), [Some(0), Some(1), Some(3), Some(2)]);
}

#[test]
fn unkeyed_children_match_by_index_when_the_structure_is_the_same() {
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![leaf(0.0, 0.0), leaf(20.0, 0.0)],
    );
    let b = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![leaf(0.0, 40.0), leaf(20.0, 40.0)],
    );
    assert_eq!(pairs(a, b), [Some(0), Some(1), Some(2)]);
}

#[test]
fn unkeyed_children_with_a_different_structure_are_unmatched() {
    // One more child, or a different kind, and none of them match.
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![leaf(0.0, 0.0), leaf(20.0, 0.0)],
    );
    let b = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![leaf(0.0, 0.0), leaf(20.0, 0.0), leaf(40.0, 0.0)],
    );
    assert_eq!(pairs(a, b), [Some(0), None, None]);
    let mut text = leaf(20.0, 0.0);
    text.kind = Kind::Text;
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![leaf(0.0, 0.0), leaf(20.0, 0.0)],
    );
    let b = bx(0.0, 0.0, 100.0, 100.0, vec![leaf(0.0, 0.0), text]);
    assert_eq!(pairs(a, b), [Some(0), None, None]);
}

#[test]
fn keyed_children_do_not_count_towards_the_unkeyed_structure() {
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![keyed("x", leaf(0.0, 0.0)), leaf(20.0, 0.0)],
    );
    let b = bx(0.0, 0.0, 100.0, 100.0, vec![leaf(20.0, 0.0)]);
    assert_eq!(pairs(a, b), [Some(0), None, Some(1)]);
}

#[test]
fn a_duplicate_key_names_both_nodes() {
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![
            keyed("x", leaf(0.0, 0.0)),
            bx(0.0, 0.0, 9.0, 9.0, vec![keyed("x", leaf(0.0, 0.0))]),
        ],
    );
    let (a, b) = (
        Tree::new(a, false, 0.5, true),
        Tree::new(leaf(0.0, 0.0), false, 0.5, false),
    );
    let Err(e) = match_trees(&a, &b) else {
        panic!("duplicate keys matched")
    };
    assert!(e.in_a);
    assert_eq!(
        e.message,
        "Box > Box[1] > Box[0]: key `x` is also on Box > Box[0]; keys must be unique in a scene's tree (not just among siblings) so magic move can match them"
    );
}
