use super::super::preset::Class;
use super::super::toolkit::Toolkit;
use super::{mac, MAC_DECODERS, MAC_DEMUXERS};

/// The Windows LGPL build as `app/scripts/ffmpeg/configure-flags.txt` had
/// it before Studio: no H.264 encoder, no AAC encoder, no rawvideo or concat
/// demuxer.
fn windows_lgpl() -> Toolkit {
    Toolkit::parse(
        "Encoders:\n ------\n A....D libopus  libopus Opus\n A....D pcm_s16le  PCM\n V....D mjpeg  MJPEG\n V....D rawvideo  raw video\n",
        "Decoders:\n ------\n V....D h264  H.264\n A....D pcm_s16le  PCM\n",
        "File formats:\n --\n D  mov,mp4,m4a,3gp,3g2,mj2 QuickTime / MOV\n D  wav  WAV\n D  matroska,webm  Matroska\n",
    )
}

/// The same build with Studio's additions: Media Foundation's H.264, the
/// native AAC encoder, and the rawvideo and concat demuxers.
fn windows_studio() -> Toolkit {
    Toolkit::parse(
        "Encoders:\n ------\n V....D h264_mf  H264 via MediaFoundation (codec h264)\n A....D aac  AAC\n V....D rawvideo  raw video\n",
        "Decoders:\n ------\n V....D rawvideo  raw video\n A....D pcm_s16le  PCM\n",
        "File formats:\n --\n D  concat  Virtual concatenation script\n D  rawvideo  raw video\n D  wav  WAV\n",
    )
}

#[test]
fn the_lists_parse_to_names() {
    let t = mac();
    for name in ["libx264", "h264_videotoolbox", "aac", "rawvideo"] {
        assert!(t.has_encoder(name), "{name}");
    }
    assert!(!t.has_encoder("Encoders:") && !t.has_encoder("V....."));
    assert!(t.check_frames().is_ok() && t.check_audio().is_ok());
    // A comma-joined demuxer line names each.
    let t = Toolkit::parse("", MAC_DECODERS, MAC_DEMUXERS);
    assert!(t.check_frames().is_ok());
}

#[test]
fn macos_drafts_on_videotoolbox_and_finals_on_x264() {
    let t = mac();
    let draft = t.codec(Class::Fast, (960, 540), 30).unwrap();
    assert_eq!(draft.args, ["-c:v", "h264_videotoolbox", "-q:v", "65"]);
    let final_ = t
        .codec(Class::Quality { crf: 18 }, (1920, 1080), 30)
        .unwrap();
    assert_eq!(
        final_.args,
        ["-c:v", "libx264", "-preset", "medium", "-crf", "18", "-threads", "8"]
    );
}

#[test]
fn the_windows_lgpl_build_names_what_it_lacks() {
    let t = windows_lgpl();
    let e = t.check_frames().unwrap_err().to_string();
    assert!(
        e.contains("the `rawvideo` demuxer and the `rawvideo` decoder and the `concat` demuxer"),
        "{e}"
    );
    let e = t.check_audio().unwrap_err().to_string();
    assert!(e.contains("the `aac` encoder") && !e.contains("wav"), "{e}");
    let e = t
        .codec(Class::Fast, (960, 540), 30)
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("no H.264 encoder") && e.contains("h264_mf"),
        "{e}"
    );
}

#[test]
fn media_foundation_takes_a_bitrate_for_its_size() {
    let t = windows_studio();
    assert!(t.check_frames().is_ok() && t.check_audio().is_ok());
    let final_ = t
        .codec(Class::Quality { crf: 18 }, (1920, 1080), 30)
        .unwrap();
    assert_eq!(final_.name, "h264_mf");
    // 0.3 bits a pixel: 1920 x 1080 x 30 x 0.3 = 18,662 kb/s.
    assert_eq!(
        final_.args,
        [
            "-c:v",
            "h264_mf",
            "-rate_control",
            "u_vbr",
            "-b:v",
            "18662k"
        ]
    );
    let draft = t.codec(Class::Fast, (960, 540), 30).unwrap();
    assert_eq!(draft.args.last().unwrap(), "1555k");
    let small = t
        .codec(Class::Quality { crf: 23 }, (1280, 720), 30)
        .unwrap();
    assert_eq!(small.args.last().unwrap(), "4147k");
}

#[test]
fn without_videotoolbox_or_x264_each_falls_back() {
    // An LGPL macOS build: VideoToolbox only, so finals use it at a high quality.
    let t = Toolkit::parse(
        "Encoders:\n ------\n V....D h264_videotoolbox  VT\n",
        "",
        "",
    );
    let final_ = t
        .codec(Class::Quality { crf: 18 }, (1920, 1080), 30)
        .unwrap();
    assert_eq!(final_.args, ["-c:v", "h264_videotoolbox", "-q:v", "80"]);
    // x264 only: drafts take veryfast.
    let t = Toolkit::parse("Encoders:\n ------\n V....D libx264  x264\n", "", "");
    let draft = t.codec(Class::Fast, (960, 540), 30).unwrap();
    assert_eq!(
        draft.args,
        ["-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-threads", "8"]
    );
}
