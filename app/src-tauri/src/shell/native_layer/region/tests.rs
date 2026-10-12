use super::*;

fn px(left: i32, top: i32, right: i32, bottom: i32) -> Px {
    Px {
        left,
        top,
        right,
        bottom,
    }
}

fn hole(left: f64, top: f64, right: f64, bottom: f64, bl: f64, br: f64) -> Hole {
    Hole {
        left,
        top,
        right,
        bottom,
        bl,
        br,
    }
}

fn cover(left: f64, top: f64, right: f64, bottom: f64, radius: f64) -> Cover {
    Cover {
        left,
        top,
        right,
        bottom,
        radius,
    }
}

#[test]
fn a_hole_lands_in_the_containers_own_physical_pixels() {
    // The page's container sits at (300, 120) in the client area, at 1.5x.
    let frame = px(300, 120, 1500, 1020);
    let region = page_region(
        &[hole(200.0, 80.0, 1000.0, 680.0, 12.0, 0.0)],
        &[],
        frame,
        1.5,
    );
    assert_eq!(
        region.holes,
        [Shape {
            rect: px(0, 0, 1200, 900),
            top_left: 0,
            top_right: 0,
            bottom_left: 18,
            bottom_right: 0,
        }]
    );
    assert!(region.covers.is_empty());
}

#[test]
fn fractional_edges_round_outward() {
    let region = page_region(
        &[hole(10.2, 10.2, 20.6, 20.6, 0.0, 0.0)],
        &[],
        px(0, 0, 100, 100),
        1.0,
    );
    assert_eq!(region.holes[0].rect, px(10, 10, 21, 21));
}

#[test]
fn float_noise_does_not_grow_an_exact_edge() {
    let region = page_region(
        &[hole(0.0, 0.0, 19.999_999_9, 20.000_000_1, 0.0, 0.0)],
        &[],
        px(0, 0, 100, 100),
        1.0,
    );
    assert_eq!(region.holes[0].rect, px(0, 0, 20, 20));
}

#[test]
fn no_hole_over_a_page_is_an_empty_region() {
    // A parked dock page: far left of the window, where no slot is.
    let frame = px(-5200, 80, -4000, 900);
    let region = page_region(
        &[hole(100.0, 60.0, 900.0, 700.0, 10.0, 10.0)],
        &[cover(150.0, 70.0, 300.0, 200.0, 8.0)],
        frame,
        1.0,
    );
    assert_eq!(region, PageRegion::default());
    assert_eq!(page_region(&[], &[], frame, 1.0), PageRegion::default());
}

#[test]
fn only_covers_over_a_hole_are_kept() {
    let region = page_region(
        &[hole(100.0, 100.0, 500.0, 500.0, 0.0, 0.0)],
        &[
            cover(450.0, 80.0, 700.0, 300.0, 6.0),
            cover(600.0, 600.0, 700.0, 700.0, 6.0),
        ],
        px(100, 100, 500, 500),
        2.0,
    );
    assert_eq!(region.covers.len(), 1);
    assert_eq!(
        region.covers[0],
        Shape {
            rect: px(800, 60, 1300, 500),
            top_left: 12,
            top_right: 12,
            bottom_left: 12,
            bottom_right: 12,
        }
    );
}

#[test]
fn radii_are_clamped_to_the_shape() {
    // A 40 x 10 hole: a bottom radius stops at the height; at half the width
    // for a wide one.
    let flat = page_region(
        &[hole(0.0, 0.0, 40.0, 10.0, 30.0, -4.0)],
        &[],
        px(0, 0, 100, 100),
        1.0,
    );
    assert_eq!(
        (flat.holes[0].bottom_left, flat.holes[0].bottom_right),
        (10, 0)
    );
    let tall = page_region(
        &[hole(0.0, 0.0, 10.0, 40.0, 30.0, f64::NAN)],
        &[],
        px(0, 0, 100, 100),
        1.0,
    );
    assert_eq!(
        (tall.holes[0].bottom_left, tall.holes[0].bottom_right),
        (5, 0)
    );
    // A cover rounds all four corners, so half its shorter side.
    let region = page_region(
        &[hole(0.0, 0.0, 100.0, 100.0, 0.0, 0.0)],
        &[cover(10.0, 10.0, 70.0, 30.0, 99.0)],
        px(0, 0, 100, 100),
        1.0,
    );
    assert_eq!(region.covers[0].top_left, 10);
}

#[test]
fn a_hole_past_the_page_still_counts_where_it_overlaps() {
    let region = page_region(
        &[hole(0.0, 0.0, 300.0, 300.0, 8.0, 8.0)],
        &[],
        px(100, 100, 200, 200),
        1.0,
    );
    assert_eq!(region.holes[0].rect, px(-100, -100, 200, 200));
}

#[test]
fn each_rounded_corner_cuts_its_square_and_circle_box() {
    let shape = Shape {
        rect: px(10, 20, 110, 220),
        top_left: 0,
        top_right: 0,
        bottom_left: 8,
        bottom_right: 4,
    };
    assert_eq!(
        shape.cuts(),
        [
            Cut {
                square: px(10, 212, 18, 220),
                circle: px(10, 204, 26, 220),
            },
            Cut {
                square: px(106, 216, 110, 220),
                circle: px(102, 212, 110, 220),
            },
        ]
    );
    let round = Shape {
        rect: px(0, 0, 50, 50),
        top_left: 5,
        top_right: 5,
        bottom_left: 5,
        bottom_right: 5,
    };
    let cuts = round.cuts();
    assert_eq!(cuts.len(), 4);
    assert_eq!(cuts[0].square, px(0, 0, 5, 5));
    assert_eq!(cuts[1].circle, px(40, 0, 50, 10));
}
