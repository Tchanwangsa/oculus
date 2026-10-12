//! `render`'s tests: the pure parts here, and end to end through the real
//! ffmpeg (`end_to_end`, skipped when there is none or it lacks what a test
//! needs).

mod audio;
mod cache;
mod end_to_end;
mod keys;
mod plan;
mod reorder;
mod running;
mod settings;
mod toolkit;

use super::preset::Preset;
use super::settings::Settings;
use super::toolkit::Toolkit;

/// macOS's GPL static build: VideoToolbox and libx264, the AAC encoder and
/// every demuxer. Trimmed from its `-encoders`, `-decoders` and `-demuxers`.
pub(super) const MAC_ENCODERS: &str = "Encoders:
 V..... = Video
 A..... = Audio
 ------
 V....D libx264              libx264 H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10 (codec h264)
 V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)
 VF...D rawvideo             raw video
 A....D aac                  AAC (Advanced Audio Coding)
 A....D pcm_s16le            PCM signed 16-bit little-endian
";
pub(super) const MAC_DECODERS: &str = "Decoders:
 V..... = Video
 ------
 V....D h264                 H.264 / AVC / MPEG-4 AVC / MPEG-4 part 10
 V..... rawvideo             raw video
 A....D pcm_s16le            PCM signed 16-bit little-endian
";
pub(super) const MAC_DEMUXERS: &str = "File formats:
 D. = Demuxing supported
 .E = Muxing supported
 --
 D  concat          Virtual concatenation script
 D  mov,mp4,m4a,3gp,3g2,mj2 QuickTime / MOV
 D  rawvideo        raw video
 D  wav             WAV / WAVE (Waveform Audio)
";

pub(super) fn mac() -> Toolkit {
    Toolkit::parse(MAC_ENCODERS, MAC_DECODERS, MAC_DEMUXERS)
}

/// `preset`'s settings for a `size` video at `fps` on the macOS build.
pub(super) fn settings(preset: Preset, size: (u32, u32), fps: u32) -> Settings {
    Settings::new(preset, preset.video(), size, fps, None, &mac()).unwrap()
}
