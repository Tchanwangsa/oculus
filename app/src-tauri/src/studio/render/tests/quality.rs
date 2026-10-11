use std::path::Path;

use super::super::quality::{Quality, Settings};

#[test]
fn a_draft_is_540_high_and_never_upscaled() {
    let d = Settings::new(Quality::Draft, (1920, 1080), 30, None).unwrap();
    assert_eq!((d.scale, d.width, d.height), (0.5, 960, 540));
    let small = Settings::new(Quality::Draft, (640, 360), 30, None).unwrap();
    assert_eq!((small.scale, small.width, small.height), (1.0, 640, 360));
    let f = Settings::new(Quality::Final, (1920, 1080), 60, None).unwrap();
    assert_eq!((f.scale, f.width, f.height), (1.0, 1920, 1080));
}

#[test]
fn an_odd_side_is_an_error_naming_the_size_and_scale() {
    let e = Settings::new(Quality::Final, (641, 360), 30, None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("641x360") && e.contains("scale 1"), "{e}");
    // Half of 1922 is odd although 1922 is not.
    let e = Settings::new(Quality::Draft, (1922, 1080), 30, None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("961x540") && e.contains("scale 0.5"), "{e}");
    // 1366 * 540 / 768 = 960.47, drawn 960 wide.
    assert!(Settings::new(Quality::Draft, (1366, 768), 30, None).is_ok());
}

#[test]
fn the_encoder_converts_to_bt709_and_tags_it() {
    let s = Settings::new(Quality::Final, (640, 360), 24, None).unwrap();
    let args: Vec<String> = s
        .encoder_args(Path::new("/x/seg.mp4.part"))
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let line = args.join(" ");
    assert!(line.starts_with("-f rawvideo -pix_fmt rgba -s 640x360 -r 24 -i - -an"));
    assert!(line.contains("-vf scale=out_color_matrix=bt709:out_range=tv -pix_fmt yuv420p"));
    assert!(line.contains("-c:v libx264 -preset medium -crf 18 -threads 8"));
    assert!(
        line.contains("-colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv")
    );
    assert!(line.ends_with("-f mp4 /x/seg.mp4.part"));
    let draft = Settings::new(Quality::Draft, (640, 360), 24, None).unwrap();
    assert!(draft.key_text().contains("h264_videotoolbox"));
    assert_ne!(draft.key_text(), s.key_text());
}
