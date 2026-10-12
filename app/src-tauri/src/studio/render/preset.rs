//! The export presets: the size each renders at and the class of encoder it
//! wants (docs/studio.md, "Rendering a video"). `draft` and `final` keep the
//! names renders.json and `renders/` already use.

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
}

/// What a preset renders: frames at `Video::scale` on an encoder of `Class`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Video {
    /// The output's shorter side in px, never above the video's own; `None`
    /// keeps the video's own size.
    pub short_side: Option<f64>,
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
    pub const ALL: [Preset; 3] = [Self::Draft, Self::Final, Self::Small];

    pub fn name(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Final => "final",
            Self::Small => "small",
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

    pub fn video(self) -> Video {
        match self {
            Self::Draft => Video {
                short_side: Some(540.0),
                class: Class::Fast,
            },
            Self::Final => Video {
                short_side: None,
                class: Class::Quality { crf: 18 },
            },
            Self::Small => Video {
                short_side: Some(720.0),
                class: Class::Quality { crf: 23 },
            },
        }
    }

    /// What the `motionBlur` token decides on: only a draft is a draft.
    pub(super) fn purpose(self) -> Purpose {
        match self {
            Self::Draft => Purpose::Draft,
            _ => Purpose::Final,
        }
    }

    /// `<dir>/renders/<name>.mp4`.
    pub fn default_out(self, dir: &Path) -> PathBuf {
        dir.join("renders").join(format!("{}.mp4", self.name()))
    }
}

impl Video {
    /// Render scale over a video of `size`.
    pub fn scale(self, (width, height): (u32, u32)) -> f64 {
        match self.short_side {
            Some(side) => (side / f64::from(width.min(height))).min(1.0),
            None => 1.0,
        }
    }
}
