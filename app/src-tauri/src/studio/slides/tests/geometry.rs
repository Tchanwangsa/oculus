use super::super::geometry::*;

const PAGE: (f64, f64) = (720.0, 405.0);

#[test]
fn padding_is_the_same_in_points_on_every_side_and_stays_on_the_page() {
    let b = padded([0.25, 0.25, 0.75, 0.75], 0.02, PAGE).unwrap();
    // 0.02 of the shorter side is 8.1 pt.
    assert_eq!(
        b,
        [
            round4(0.25 - 8.1 / 720.0),
            round4(0.25 - 8.1 / 405.0),
            round4(0.75 + 8.1 / 720.0),
            round4(0.75 + 8.1 / 405.0)
        ]
    );
    let edge = padded([0.0, 0.0, 1.0, 0.5], 0.05, PAGE).unwrap();
    assert_eq!(edge, [0.0, 0.0, 1.0, round4(0.5 + 0.05 * 405.0 / 405.0)]);
}

#[test]
fn an_empty_or_inverted_box_is_refused() {
    assert!(padded([0.5, 0.5, 0.5, 0.6], 0.0, PAGE).is_err());
    assert!(padded([0.6, 0.5, 0.4, 0.6], 0.0, PAGE).is_err());
    assert!(padded([0.1, 0.1, f64::NAN, 0.6], 0.0, PAGE).is_err());
    assert!(padded([0.1, 0.1, 0.2, 0.2], -1.0, PAGE).is_err());
    assert!(parse_box("0.1,0.2,0.3").is_err());
    assert!(parse_box("0.1,0.2,0.3,1.4").is_err());
    assert_eq!(
        parse_box(" 0.1, 0.2,0.3,0.4").unwrap(),
        [0.1, 0.2, 0.3, 0.4]
    );
}

#[test]
fn the_default_crop_is_twice_the_frame_it_fills() {
    // A wide box fills the frame's width: 2 x 1920.
    let wide = [0.0, 0.0, 1.0, 0.5]; // 720 x 202.5 pt
    assert_eq!(
        crop_pixels(&wide, PAGE, (1920, 1080), None).unwrap(),
        (3840, 1080)
    );
    // A tall box fills its height: 2 x 1080 high.
    let tall = [0.0, 0.0, 0.25, 1.0]; // 180 x 405 pt
    let (w, h) = crop_pixels(&tall, PAGE, (1920, 1080), None).unwrap();
    assert_eq!(h, 2160);
    assert_eq!(w, 960);
    // The whole page at 2x is past the pixel cap, so it shrinks under it.
    let (w, h) = crop_pixels(&[0.0, 0.0, 1.0, 1.0], PAGE, (1920, 1080), None).unwrap();
    assert!(u64::from(w) * u64::from(h) <= MAX_CROP_PIXELS, "{w}x{h}");
    assert!(w > 3000, "{w}");
}

#[test]
fn an_asked_for_width_is_kept_or_refused() {
    let b = [0.0, 0.0, 0.5, 0.5]; // 360 x 202.5 pt
    assert_eq!(
        crop_pixels(&b, PAGE, (1920, 1080), Some(800)).unwrap(),
        (800, 450)
    );
    assert!(crop_pixels(&b, PAGE, (1920, 1080), Some(0)).is_err());
    assert!(crop_pixels(&b, PAGE, (1920, 1080), Some(8000)).is_err());
}

#[test]
fn the_page_image_fits_the_frame() {
    assert_eq!(page_pixels(PAGE, (1920, 1080)), (1920, 1080));
    assert_eq!(page_pixels((595.0, 842.0), (1920, 1080)), (763, 1080));
}

#[test]
fn an_inner_box_is_a_fraction_of_the_crop() {
    let crop = [0.4, 0.2, 0.8, 0.6];
    assert_eq!(
        within(&[0.5, 0.3, 0.6, 0.4], &crop),
        Some([0.25, 0.25, 0.5, 0.5])
    );
    // Clipped to the crop when most of it is inside.
    assert_eq!(
        within(&[0.35, 0.3, 0.8, 0.4], &crop),
        Some([0.0, 0.25, 1.0, 0.5])
    );
    // Mostly outside: not in it.
    assert_eq!(within(&[0.1, 0.1, 0.45, 0.3], &crop), None);
    assert_eq!(
        union(&[[0.1, 0.5, 0.2, 0.6], [0.3, 0.1, 0.4, 0.2]]),
        Some([0.1, 0.1, 0.4, 0.6])
    );
}
