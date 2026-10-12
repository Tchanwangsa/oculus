use std::path::Path;

use super::super::error::RenderError;
use super::super::preset::{Output, Preset};
use super::super::settings::Settings;
use super::{mac, settings};

fn try_settings(preset: Preset, size: (u32, u32)) -> Result<Settings, RenderError> {
    let Output::Video(video) = preset.output() else {
        unreachable!()
    };
    Settings::new(preset, video, size, 30, None, &mac())
}

#[test]
fn each_video_preset_has_its_size() {
    let size = |p, s| {
        let s = settings(p, s, 30);
        (s.scale, s.width, s.height)
    };
    assert_eq!(size(Preset::Draft, (1920, 1080)), (0.5, 960, 540));
    assert_eq!(size(Preset::Draft, (640, 360)), (1.0, 640, 360));
    assert_eq!(size(Preset::Final, (1920, 1080)), (1.0, 1920, 1080));
    assert_eq!(size(Preset::Small, (1920, 1080)).1, 1280);
    assert_eq!(size(Preset::Double, (1920, 1080)), (2.0, 3840, 2160));
    // Portrait scales by its shorter side, so its draft is as sharp.
    assert_eq!(size(Preset::Draft, (1080, 1920)), (0.5, 540, 960));
    assert_eq!(size(Preset::Small, (1080, 1920)).2, 1280);
}

#[test]
fn an_odd_or_oversized_side_is_an_error_naming_the_size() {
    let e = try_settings(Preset::Final, (641, 360))
        .unwrap_err()
        .to_string();
    assert!(e.contains("641x360") && e.contains("scale 1"), "{e}");
    // Half of 1922 is odd although 1922 is not.
    let e = try_settings(Preset::Draft, (1922, 1080))
        .unwrap_err()
        .to_string();
    assert!(e.contains("961x540") && e.contains("scale 0.5"), "{e}");
    // 1366 * 540 / 768 = 960.47, drawn 960 wide.
    assert!(try_settings(Preset::Draft, (1366, 768)).is_ok());
    let e = try_settings(Preset::Double, (4800, 2700))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("2x render") && e.contains("9600x5400") && e.contains("8192"),
        "{e}"
    );
}

#[test]
fn the_encoder_converts_to_bt709_and_tags_it() {
    let s = settings(Preset::Final, (640, 360), 24);
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
    let draft = settings(Preset::Draft, (640, 360), 24);
    assert!(draft.key_text().contains("h264_videotoolbox"));
    assert_ne!(draft.key_text(), s.key_text());
    let small = settings(Preset::Small, (1920, 1080), 24);
    assert!(small.key_text().contains("-crf 23"), "{}", small.key_text());
}

#[test]
fn presets_parse_by_name_and_write_to_renders() {
    for p in Preset::ALL {
        assert_eq!(Preset::parse(p.name()), Ok(p));
    }
    let e = Preset::parse("4k").unwrap_err();
    assert!(
        e.contains("draft, final, small, 2x, audio, subtitles"),
        "{e}"
    );
    let dir = Path::new("/v");
    assert_eq!(
        Preset::Final.default_out(dir),
        Path::new("/v/renders/final.mp4")
    );
    assert_eq!(
        Preset::Audio.default_out(dir),
        Path::new("/v/renders/audio.m4a")
    );
    assert_eq!(
        Preset::Subtitles.default_out(dir),
        Path::new("/v/renders/subtitles.vtt")
    );
    assert_eq!(serde_json::to_value(Preset::Double).unwrap(), "2x");
}
