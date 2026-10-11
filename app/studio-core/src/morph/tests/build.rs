use super::{bx, keyed, leaf};
use crate::layout::{LNode, Typeset};
use crate::morph::{Ink, MorphColor, morph};

const INK: Ink = Ink {
    text: Color::BLACK,
    colour: MorphColor::Oklch,
};
use crate::scene::Color;

fn run(a: LNode, b: LNode, p: f32) -> LNode {
    morph(a, b, p, &mut Typeset::new(), INK).unwrap()
}

fn keys(n: &LNode) -> Vec<&str> {
    n.children
        .iter()
        .map(|c| c.key.as_deref().unwrap_or("-"))
        .collect()
}

#[test]
fn unmatched_a_nodes_go_before_their_next_matched_sibling() {
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![
            keyed("gone1", leaf(0.0, 0.0)),
            keyed("stay1", leaf(10.0, 0.0)),
            keyed("gone2", leaf(20.0, 0.0)),
            keyed("stay2", leaf(30.0, 0.0)),
            keyed("gone3", leaf(40.0, 0.0)),
        ],
    );
    let b = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![
            keyed("stay2", leaf(0.0, 0.0)),
            keyed("new", leaf(10.0, 0.0)),
            keyed("stay1", leaf(20.0, 0.0)),
        ],
    );
    let out = run(a, b, 0.2);
    assert_eq!(
        keys(&out),
        ["gone2", "stay2", "new", "gone1", "stay1", "gone3"]
    );
}

#[test]
fn matched_nodes_move_and_unmatched_ones_fade_in_their_windows() {
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![keyed("card", leaf(0.0, 0.0)), keyed("old", leaf(50.0, 0.0))],
    );
    let b = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![
            keyed("card", leaf(40.0, 20.0)),
            keyed("new", leaf(50.0, 50.0)),
        ],
    );
    let at = |p: f32| {
        let out = run(
            bx(
                0.0,
                0.0,
                100.0,
                100.0,
                vec![keyed("card", leaf(0.0, 0.0)), keyed("old", leaf(50.0, 0.0))],
            ),
            bx(
                0.0,
                0.0,
                100.0,
                100.0,
                vec![
                    keyed("card", leaf(40.0, 20.0)),
                    keyed("new", leaf(50.0, 50.0)),
                ],
            ),
            p,
        );
        let find = |k: &str| {
            out.children
                .iter()
                .find(|c| c.key.as_deref() == Some(k))
                .map(|c| (c.rect, c.style.opacity()))
        };
        (find("card"), find("old"), find("new"))
    };
    let _ = (a, b);
    let (card, old, new) = at(0.25);
    let (r, o) = card.unwrap();
    assert_eq!((r.x0, r.y0, o), (10.0, 5.0, 1.0));
    assert!(old.unwrap().1 > 0.0 && old.unwrap().1 < 1.0, "{old:?}");
    assert_eq!(new.unwrap().1, 0.0);
    let (_, old, new) = at(0.4);
    assert_eq!(old.unwrap().1, 0.0);
    assert_eq!(new.unwrap().1, 0.0);
    let (_, _, new) = at(0.7);
    assert!((new.unwrap().1 - 0.5).abs() < 1e-6, "{new:?}");
}

#[test]
fn a_new_container_does_not_fade_the_matched_card_inside_it() {
    let a = bx(0.0, 0.0, 100.0, 100.0, vec![keyed("card", leaf(0.0, 0.0))]);
    let b = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![bx(
            20.0,
            20.0,
            60.0,
            60.0,
            vec![keyed("card", leaf(30.0, 30.0))],
        )],
    );
    let out = run(a, b, 0.5);
    let wrapper = &out.children[0];
    // No group fade on the new box: its own fill fades instead.
    assert_eq!(wrapper.style.opacity(), 1.0);
    let Some(crate::scene::Fill::Solid(c)) = wrapper.style.fill else {
        panic!("fill")
    };
    assert!(c.a > 0.0 && c.a < 1.0, "{c:?}");
    let card = &wrapper.children[0];
    assert_eq!(card.style.opacity(), 1.0);
    assert_eq!((card.rect.x0, card.rect.y0), (15.0, 15.0));
}

#[test]
fn unmatched_children_ride_with_their_matched_parent() {
    let a = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![keyed(
            "panel",
            bx(0.0, 0.0, 50.0, 50.0, vec![keyed("old", leaf(5.0, 5.0))]),
        )],
    );
    let b = bx(
        0.0,
        0.0,
        100.0,
        100.0,
        vec![keyed("panel", bx(40.0, 0.0, 50.0, 50.0, vec![]))],
    );
    let out = run(a, b, 0.25);
    let old = &out.children[0].children[0];
    assert_eq!(old.key.as_deref(), Some("old"));
    assert_eq!(old.rect.x0, 15.0);
}

#[test]
fn outside_a_morph_nodes_match_by_path_and_move_at_its_p() {
    use crate::morph::morph_scene;
    use crate::scene::{Kind, Scope};
    let scoped = |mut n: LNode, kind: Kind| {
        n.kind = kind;
        n.scope = Some(Scope { id: 0, p: 0.25 });
        n
    };
    // The Morph swaps a Text for a taller Box; the footer after it moves.
    let a = bx(
        0.0,
        0.0,
        100.0,
        200.0,
        vec![
            leaf(0.0, 0.0),
            scoped(bx(0.0, 10.0, 50.0, 20.0, vec![]), Kind::Text),
            leaf(0.0, 30.0),
        ],
    );
    let b = bx(
        0.0,
        0.0,
        100.0,
        200.0,
        vec![
            leaf(0.0, 0.0),
            scoped(
                bx(0.0, 10.0, 50.0, 60.0, vec![keyed("new", leaf(0.0, 10.0))]),
                Kind::Box,
            ),
            leaf(0.0, 70.0),
        ],
    );
    let out = morph_scene(a, b, &mut Typeset::new(), INK).unwrap();
    assert_eq!(out.children.len(), 3);
    assert_eq!(out.children[2].rect.y0, 40.0);
    let morphed = &out.children[1];
    assert_eq!(morphed.rect.height(), 30.0);
    assert!(morphed.scope.is_none());
    // Unmatched inside the Morph: fading in at the Morph's p (0.25 < 0.4).
    assert_eq!(morphed.children[0].style.opacity(), 0.0);
}
