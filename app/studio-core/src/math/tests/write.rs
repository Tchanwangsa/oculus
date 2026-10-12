//! `<Write>`'s plan: which group each piece writes with, and when.

use crate::math::typeset;
use crate::math::write::plan;
use crate::scene::WriteSpec;

fn spec(groups: &[(Option<&str>, f32)], lag: Option<f32>) -> WriteSpec {
    WriteSpec {
        groups: groups
            .iter()
            .map(|(g, p)| (g.map(str::to_owned), *p))
            .collect(),
        lag,
        stroke: 0.025,
    }
}

#[test]
fn unlisted_pieces_write_with_the_nearest_listed_group_before_them() {
    // a b + c = d: `+` follows `ab`; `=` and `d` follow `c`; `q` before
    // any listed group takes the first listed group in reading order.
    let f = typeset(r"q \k{ab}{ab} + \k{c}{c} = d", true).unwrap();
    let keys: Vec<&str> = f.items.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(keys, ["q", "ab", "ab.1", "+", "c", "=", "d"]);
    let u = plan(
        &f,
        &spec(&[(Some("c"), 0.25), (Some("ab"), 0.75)], Some(0.0)),
    )
    .unwrap();
    assert_eq!(u, [0.75, 0.75, 0.75, 0.75, 0.25, 0.25, 0.25]);
}

#[test]
fn a_group_writes_its_pieces_one_after_another() {
    let f = typeset(r"\k{g}{abcd}", true).unwrap();
    // lag 0.5 over four pieces: the whole runs 2.5 piece lengths.
    let at = |p: f32| plan(&f, &spec(&[(Some("g"), p)], Some(0.5))).unwrap();
    assert_eq!(at(0.0), [0.0; 4]);
    assert_eq!(at(0.2), [0.5, 0.0, 0.0, 0.0]);
    assert_eq!(at(0.6), [1.0, 1.0, 0.5, 0.0]);
    assert_eq!(at(1.0), [1.0; 4]);
    // Past the end and before the start it holds.
    assert_eq!(at(3.0), [1.0; 4]);
    assert_eq!(at(-1.0), [0.0; 4]);
}

#[test]
fn a_number_writes_the_whole_formula_with_manims_lag() {
    let f = typeset(r"x + y", true).unwrap();
    let u = plan(&f, &spec(&[(None, 0.5)], None)).unwrap();
    // Three pieces, lag min(4 / 4, 0.2) = 0.2: 1.4 piece lengths in all.
    let want = [0.7, 0.5, 0.3];
    for (got, want) in u.iter().zip(want) {
        assert!((got - want).abs() < 1e-6, "{u:?}");
    }
}

#[test]
fn an_unknown_group_names_the_formulas_groups() {
    let f = typeset(r"\k{lhs}{x} = \k{rhs}{y}", true).unwrap();
    let e = plan(&f, &spec(&[(Some("left"), 0.5)], None)).unwrap_err();
    assert!(e.contains("`left` is not a \\k group"), "{e}");
    assert!(e.contains("lhs, rhs"), "{e}");
}

#[test]
fn an_outer_group_covers_the_groups_inside_it() {
    let f = typeset(r"\k{all}{\k{a}{x} + y}", true).unwrap();
    let u = plan(&f, &spec(&[(Some("all"), 1.0)], Some(0.0))).unwrap();
    assert_eq!(u, [1.0, 1.0, 1.0]);
}
