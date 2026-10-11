use super::*;

fn length(pieces: &[Piece]) -> f64 {
    pieces
        .iter()
        .flat_map(|p| p.path.segments())
        .map(|s| s.arclen(1e-9))
        .sum()
}

fn path(d: &str) -> BezPath {
    BezPath::from_svg(d).unwrap()
}

#[test]
fn trims_a_line_by_arc_length() {
    let pieces = trim(&path("M 0 0 L 100 0"), 0.25, 0.5);
    assert_eq!(pieces.len(), 1);
    let segs: Vec<PathSeg> = pieces[0].path.segments().collect();
    assert!((segs[0].start().x - 25.0).abs() < 1e-9);
    assert!((segs[0].end().x - 50.0).abs() < 1e-9);
    assert!((pieces[0].phase - 25.0).abs() < 1e-9);
}

#[test]
fn trims_curves_exactly() {
    let p = path("M 0 0 C 50 100 100 -100 150 0");
    let full = p.segments().map(|s| s.arclen(1e-9)).sum::<f64>();
    let half = trim(&p, 0.0, 0.5);
    assert!(
        (length(&half) - full / 2.0).abs() < 1e-3,
        "{}",
        length(&half)
    );
}

#[test]
fn spans_subpaths_in_order() {
    // Two 100-long subpaths: [0.25, 0.75] is the second half of the first
    // and the first half of the second.
    let pieces = trim(&path("M 0 0 L 100 0 M 0 10 L 100 10"), 0.25, 0.75);
    assert_eq!(pieces.len(), 2);
    assert!((pieces[0].phase - 50.0).abs() < 1e-9);
    assert_eq!(pieces[1].phase, 0.0);
    assert!((length(&pieces) - 100.0).abs() < 1e-6);
}

#[test]
fn keeps_corners_joined_and_closes_whole_closed_subpaths() {
    let square = path("M 0 0 L 10 0 L 10 10 L 0 10 Z");
    let three_sides = trim(&square, 0.0, 0.75);
    assert_eq!(three_sides.len(), 1);
    assert_eq!(three_sides[0].path.elements().len(), 4);
    let all = trim(&square, 0.0, 0.999_999_9);
    assert!(!matches!(
        all[0].path.elements().last(),
        Some(PathEl::ClosePath)
    ));
    assert!(trim(&square, 0.5, 0.5).is_empty());
    assert!(trim(&square, 0.8, 0.2).is_empty());
}
