//! A video preset's render settings: output size, motion blur and the
//! encoder's arguments, all part of every segment's key (docs/studio.md,
//! "Rendering a video").

use studio_core::MotionBlur;

use super::error::RenderError;
use super::preset::{Preset, Video};
use super::toolkit::{Codec, Toolkit};

/// The largest output side: twice an 8192 px video would not encode.
const MAX_SIDE: u32 = 8192;

/// Everything about a video render that is not the project: part of every
/// segment's key.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Settings {
    pub preset: Preset,
    pub scale: f64,
    /// Output px, as `Renderer` draws them at `scale`.
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub motion_blur: Option<MotionBlur>,
    pub codec: Codec,
}

impl Settings {
    /// `size` is the video's. Fails on an odd output side, which yuv420p
    /// cannot hold, rather than padding it silently, on one over 8192 px,
    /// and when `toolkit` has no encoder for the preset.
    pub fn new(
        preset: Preset,
        video: Video,
        size: (u32, u32),
        fps: u32,
        motion_blur: Option<MotionBlur>,
        toolkit: &Toolkit,
    ) -> Result<Self, RenderError> {
        let scale = video.scale(size);
        let side = |v: u32| (f64::from(v) * scale).round() as u32;
        let (width, height) = (side(size.0), side(size.1));
        let name = preset.name();
        if width > MAX_SIDE || height > MAX_SIDE {
            return Err(RenderError::Render(format!(
                "a {name} render of this {}x{} video is {width}x{height} px; frames are at most {MAX_SIDE} px a side, so pick a smaller preset",
                size.0, size.1
            )));
        }
        if width % 2 == 1 || height % 2 == 1 {
            return Err(RenderError::Render(format!(
                "a {name} render of this {}x{} video at scale {scale} is {width}x{height} px; H.264 in yuv420p needs even sides, so change video.json's size",
                size.0, size.1
            )));
        }
        let codec = toolkit.codec(video.class, (width, height), fps)?;
        Ok(Self {
            preset,
            scale,
            width,
            height,
            fps,
            motion_blur,
            codec,
        })
    }

    /// One line naming every setting, for the segment key.
    pub fn key_text(&self) -> String {
        format!(
            "{} scale={:?} {}x{} fps={} blur={:?} vf={} codec={} tags={}",
            self.preset.name(),
            self.scale,
            self.width,
            self.height,
            self.fps,
            self.motion_blur,
            COLOUR_FILTER,
            self.codec.args.join(" "),
            COLOUR_TAGS.join(" "),
        )
    }

    /// ffmpeg's arguments for one segment: raw RGBA frames on stdin, an
    /// H.264 mp4 at `out` (any extension: the muxer is named).
    pub fn encoder_args(&self, out: &std::path::Path) -> Vec<std::ffi::OsString> {
        let size = format!("{}x{}", self.width, self.height);
        let fps = self.fps.to_string();
        let mut args: Vec<std::ffi::OsString> = [
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &size,
            "-r",
            &fps,
            "-i",
            "-",
            "-an",
            "-vf",
            COLOUR_FILTER,
            "-pix_fmt",
            "yuv420p",
        ]
        .iter()
        .map(Into::into)
        .collect();
        args.extend(self.codec.args.iter().map(Into::into));
        args.extend(COLOUR_TAGS.iter().map(Into::into));
        args.extend(["-f", "mp4"].map(Into::into));
        args.push(out.into());
        args
    }
}

/// RGB to BT.709 limited range, said explicitly: swscale's default matrix is
/// BT.601, which shifts the indigo.
const COLOUR_FILTER: &str = "scale=out_color_matrix=bt709:out_range=tv";

/// The stream's colour tags, matching the conversion.
const COLOUR_TAGS: [&str; 8] = [
    "-colorspace",
    "bt709",
    "-color_primaries",
    "bt709",
    "-color_trc",
    "bt709",
    "-color_range",
    "tv",
];
