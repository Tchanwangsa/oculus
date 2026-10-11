//! What the viewer sends the frontend, as `docs/viewers.md` describes it.

use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct PageSize {
    pub(super) width: f32,
    pub(super) height: f32,
}

#[derive(Serialize, Debug)]
pub struct OpenedPdf {
    pub(super) pages: Vec<PageSize>,
}

/// One line of page text in reading order. `chars` holds `text`'s UTF-16
/// length + 1 stops along the line: x for horizontal lines, y for vertical.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct TextLine {
    pub(super) text: String,
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) vertical: bool,
    pub(super) chars: Vec<f32>,
}

#[derive(Serialize, Debug, Default)]
pub struct PageText {
    pub(super) lines: Vec<TextLine>,
}

/// A link annotation's box and target: a URI, or a 1-based page.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Link {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) page: Option<u32>,
}

/// Points to two decimals: finer than any screen, and a shorter IPC body.
pub(super) fn round(value: f64) -> f32 {
    ((value * 100.0).round() / 100.0) as f32
}
