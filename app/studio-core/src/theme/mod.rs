//! Themes: tokens only (colours, fonts, type scale, space, radii, motion,
//! narration timing, captions).
//! Components read them from `ctx.theme`; a retheme is an edit here, never
//! in a component. Bundled themes live in the crate's `themes/` folder.

mod schema;

pub use schema::{DEFAULT_PAUSE, NarrationTokens};

use crate::captions::CaptionStyle;
use crate::error::{Result, StudioError};
use crate::scene::Color;
use crate::transition::MotionSpec;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

const BUNDLED: &[(&str, &str)] = &[("default", include_str!("../../themes/default.json"))];

/// A library theme's name, which is its file stem: lowercase letters, digits
/// and `-`, at most 64 characters. Every name the app writes, promotes or
/// scaffolds into a video follows it, and it keeps `themes/<name>.json`
/// inside its library.
pub fn valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// A validated theme.
#[derive(Clone, Debug)]
pub struct Theme {
    /// The theme as components see it (`ctx.theme`).
    pub tokens: Value,
    /// `color.bg`: the canvas under every frame.
    pub background: Color,
    /// `color.text`: the colour of `Text` without `color`.
    pub text_color: Color,
    /// `font.body`: the family of `Text` without a `fontFamily`.
    pub body_font: String,
    /// `motion.transition`, `motion.magicMove`, `motion.morphColor` and
    /// `motion.motionBlur`: the bottom of the motion cascade.
    pub motion: MotionSpec,
    /// `motion.easing`: named cubic-bezier points.
    pub easings: BTreeMap<String, [f64; 4]>,
    /// `narration`: the silence before and after a scene's narration, and
    /// a bare `[pause]`'s length.
    pub narration: NarrationTokens,
    /// `captions`: how burned-in captions look.
    pub captions: CaptionStyle,
}

impl Theme {
    /// A theme compiled into the crate, by name.
    pub fn bundled(name: &str) -> Result<Theme> {
        let Some((_, json)) = BUNDLED.iter().find(|(n, _)| *n == name) else {
            return Err(StudioError::Project {
                path: format!("themes/{name}.json").into(),
                message: format!(
                    "no bundled theme `{name}`; bundled themes: {} (or give a path ending in .json, relative to the project)",
                    Self::bundled_names().join(", ")
                ),
            });
        };
        Self::parse(json, Path::new(&format!("themes/{name}.json")))
    }

    /// A bundled theme's file text, as compiled in.
    pub fn bundled_text(name: &str) -> Option<&'static str> {
        BUNDLED
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, json)| *json)
    }

    pub fn bundled_names() -> Vec<&'static str> {
        BUNDLED.iter().map(|(n, _)| *n).collect()
    }

    pub fn from_file(path: &Path) -> Result<Theme> {
        let json = std::fs::read_to_string(path).map_err(|e| StudioError::io(path, e))?;
        Self::parse(&json, path)
    }

    /// Parses and validates theme JSON; `path` is only used in messages.
    pub fn parse(json: &str, path: &Path) -> Result<Theme> {
        let fail = |message: String| StudioError::Project {
            path: path.to_path_buf(),
            message,
        };
        let mut tokens: Value = serde_json::from_str(json).map_err(|e| fail(e.to_string()))?;
        schema::fill_defaults(&mut tokens);
        let checked = schema::check(&tokens).map_err(fail)?;
        Ok(Theme {
            tokens,
            background: checked.background,
            text_color: checked.text,
            body_font: checked.body_font,
            motion: checked.motion,
            easings: checked.easings,
            narration: checked.narration,
            captions: checked.captions,
        })
    }

    /// The token names under a dotted section such as `color` or
    /// `motion.spring`, or `None` if the section does not exist.
    pub fn section(&self, kind: &str) -> Option<&serde_json::Map<String, Value>> {
        let mut v = &self.tokens;
        for part in kind.split('.') {
            v = v.get(part)?;
        }
        v.as_object()
    }
}

#[cfg(test)]
mod tests;
