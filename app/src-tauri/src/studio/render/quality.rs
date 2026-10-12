//! The two qualities and what each one fixes: output size, motion blur and
//! the encoder's arguments (docs/studio.md, "Rendering a video").

use serde::{Deserialize, Serialize};
use studio_core::{MotionBlur, Purpose};

use super::error::RenderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// A 540 px-high proxy on the hardware encoder, blurred only when the
    /// `motionBlur` token says `always`.
    Draft,
    /// Full size on libx264, blurred per the `motionBlur` token.
    Final,
}

/// A draft's frame height in px; a smaller video renders at its own size.
const DRAFT_HEIGHT: f64 = 540.0;

impl Quality {
    pub fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Final => "final",
        }
    }

    pub(super) fn purpose(self) -> Purpose {
        match self {
            Self::Draft => Purpose::Draft,
            Self::Final => Purpose::Final,
        }
    }

    /// The video codec's arguments. VideoToolbox takes a constant quality
    /// (`-q:v`, 1 to 100) on Apple Silicon. x264 gets a fixed 8 threads:
    /// its default (1.5 per core) held 1.1 GB at 1080p on an 18-core Mac
    /// against 0.78 GB at 8, at the same speed, and a fixed count makes its
    /// output the same on every machine.
    pub(super) fn codec_args(self) -> &'static [&'static str] {
        match self {
            Self::Draft => &["-c:v", "h264_videotoolbox", "-q:v", "65"],
            Self::Final => &[
                "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-threads", "8",
            ],
        }
    }

    /// Render scale over the video's size.
    pub(super) fn scale(self, height: u32) -> f64 {
        match self {
            Self::Draft => (DRAFT_HEIGHT / f64::from(height)).min(1.0),
            Self::Final => 1.0,
        }
    }
}

/// Everything about a render that is not the project: part of every
/// segment's key.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Settings {
    pub quality: Quality,
    pub scale: f64,
    /// Output px, as `Renderer` draws them at `scale`.
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub motion_blur: Option<MotionBlur>,
}

impl Settings {
    /// `size` is the video's; fails on an odd output side, which yuv420p
    /// cannot hold, rather than padding it silently.
    pub fn new(
        quality: Quality,
        size: (u32, u32),
        fps: u32,
        motion_blur: Option<MotionBlur>,
    ) -> Result<Self, RenderError> {
        let scale = quality.scale(size.1);
        let side = |v: u32| (f64::from(v) * scale).round() as u32;
        let (width, height) = (side(size.0), side(size.1));
        if width % 2 == 1 || height % 2 == 1 {
            return Err(RenderError::Render(format!(
                "a {} render of this {}x{} video at scale {scale} is {width}x{height} px; H.264 in yuv420p needs even sides, so change video.json's size",
                quality.name(),
                size.0,
                size.1
            )));
        }
        Ok(Self {
            quality,
            scale,
            width,
            height,
            fps,
            motion_blur,
        })
    }

    /// One line naming every setting, for the segment key.
    pub fn key_text(&self) -> String {
        format!(
            "{} scale={:?} {}x{} fps={} blur={:?} vf={} codec={} tags={}",
            self.quality.name(),
            self.scale,
            self.width,
            self.height,
            self.fps,
            self.motion_blur,
            COLOUR_FILTER,
            self.quality.codec_args().join(" "),
            COLOUR_TAGS.join(" "),
        )
    }

    /// ffmpeg's arguments for one segment: raw RGBA frames on stdin, an
    /// H.264 mp4 at `out` (any extension: the muxer is named).
    pub fn encoder_args(&self, out: &std::path::Path) -> Vec<std::ffi::OsString> {
        let size = format!("{}x{}", self.width, self.height);
        let fps = self.fps.to_string();
        let mut args: Vec<std::ffi::OsString> = Vec::new();
        let mut push = |a: &str| args.push(a.into());
        for a in [
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
        ] {
            push(a);
        }
        for a in self.quality.codec_args() {
            push(a);
        }
        for a in COLOUR_TAGS {
            push(a);
        }
        push("-f");
        push("mp4");
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
