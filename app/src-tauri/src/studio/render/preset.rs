//! The export presets: what each writes, at what size and on which class of
//! encoder (docs/studio.md, "Rendering a video"). `draft` and `final` keep
//! the names renders.json and `renders/` already use.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use studio_core::Purpose;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Preset {
    /// A 540 px proxy on the fastest encoder, blurred only when the
    /// `motionBlur` token says `always`.
    #[serde(rename = "draft")]
    Draft,
    /// The video's size at crf 18, blurred per the `motionBlur` token.
    #[serde(rename = "final")]
    Final,
    /// A 720 px version at crf 23, for sharing.
    #[serde(rename = "small")]
    Small,
    /// Twice the video's size over the same layout (4K for a 1080p video).
    #[serde(rename = "2x")]
    Double,
    /// The narration track alone, as AAC in an m4a.
    #[serde(rename = "audio")]
    Audio,
    /// The captions' phrases as WebVTT, and SubRip beside it.
    #[serde(rename = "subtitles")]
    Subtitles,
}

/// What a preset writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Output {
    /// Frames at `Video::scale` on an encoder of `Class`.
    Video(Video),
    Audio,
    Subtitles,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Video {
    /// The output's shorter side in px, never above the video's own; `None`
    /// scales by `factor` instead.
    pub short_side: Option<f64>,
    pub factor: f64,
    pub class: Class,
}

/// The encoder a preset wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Speed first: the hardware encoder where there is one.
    Fast,
    /// Quality first: libx264 at this crf where there is one.
    Quality { crf: u8 },
}

impl Preset {
    pub const ALL: [Preset; 6] = [
        Self::Draft,
        Self::Final,
        Self::Small,
        Self::Double,
        Self::Audio,
        Self::Subtitles,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Final => "final",
            Self::Small => "small",
            Self::Double => "2x",
            Self::Audio => "audio",
            Self::Subtitles => "subtitles",
        }
    }

    /// The preset `name` names, or an error listing them.
    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|p| p.name() == name)
            .ok_or_else(|| {
                let names: Vec<&str> = Self::ALL.iter().map(|p| p.name()).collect();
                format!(
                    "no export preset `{name}`; the presets are {}",
                    names.join(", ")
                )
            })
    }

    pub fn output(self) -> Output {
        let video = |short_side, factor, class| {
            Output::Video(Video {
                short_side,
                factor,
                class,
            })
        };
        match self {
            Self::Draft => video(Some(540.0), 1.0, Class::Fast),
            Self::Final => video(None, 1.0, Class::Quality { crf: 18 }),
            Self::Small => video(Some(720.0), 1.0, Class::Quality { crf: 23 }),
            Self::Double => video(None, 2.0, Class::Quality { crf: 18 }),
            Self::Audio => Output::Audio,
            Self::Subtitles => Output::Subtitles,
        }
    }

    /// The output file's extension.
    pub fn extension(self) -> &'static str {
        match self.output() {
            Output::Video(_) => "mp4",
            Output::Audio => "m4a",
            Output::Subtitles => "vtt",
        }
    }

    /// What the `motionBlur` token decides on: only a draft is a draft.
    pub(super) fn purpose(self) -> Purpose {
        match self {
            Self::Draft => Purpose::Draft,
            _ => Purpose::Final,
        }
    }

    /// `<dir>/renders/<name>.<extension>`.
    pub fn default_out(self, dir: &Path) -> PathBuf {
        dir.join("renders")
            .join(format!("{}.{}", self.name(), self.extension()))
    }
}

impl Video {
    /// Render scale over a video of `size`.
    pub fn scale(self, (width, height): (u32, u32)) -> f64 {
        match self.short_side {
            Some(side) => (side / f64::from(width.min(height))).min(1.0),
            None => self.factor,
        }
    }
}
