//! `library.json`: a library folder's own settings. Today that is only the
//! theme a new video in the library's reach takes when it names none.

use crate::error::{Result, StudioError};
use crate::theme::valid_theme_name;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The settings file at a library's root.
pub const SETTINGS_FILE: &str = "library.json";

/// `library.json`, e.g. `{"defaultTheme": "warm"}`. A library without the
/// file has no settings.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LibrarySettings {
    /// A theme name the cascade resolves from this library: its own
    /// `themes/`, then any farther library, then the bundled set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_theme: Option<String>,
}

impl LibrarySettings {
    /// The settings of the library at `root`; the default when it has no
    /// `library.json`. A file that does not parse is an error naming it.
    pub fn read(root: &Path) -> Result<LibrarySettings> {
        let path = root.join(SETTINGS_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LibrarySettings::default());
            }
            Err(e) => return Err(StudioError::io(&path, e)),
        };
        let fail = |message: String| StudioError::Project {
            path: path.clone(),
            message,
        };
        let settings: LibrarySettings =
            serde_json::from_str(&text).map_err(|e| fail(e.to_string()))?;
        settings.check().map_err(fail)?;
        Ok(settings)
    }

    fn check(&self) -> std::result::Result<(), String> {
        match self.default_theme.as_deref() {
            Some(name) if !valid_theme_name(name) => Err(format!(
                "defaultTheme `{name}` is not a theme name: lowercase letters, digits and `-`"
            )),
            _ => Ok(()),
        }
    }

    /// Writes the settings to `root/library.json` through a temporary file
    /// and a rename; settings with nothing set remove the file.
    pub fn write(&self, root: &Path) -> Result<()> {
        let path = root.join(SETTINGS_FILE);
        if *self == LibrarySettings::default() {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Err(StudioError::io(&path, e))
                }
                _ => Ok(()),
            };
        }
        self.check().map_err(StudioError::Render)?;
        std::fs::create_dir_all(root).map_err(|e| StudioError::io(root, e))?;
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| StudioError::Render(format!("cannot write {SETTINGS_FILE}: {e}")))?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, text + "\n").map_err(|e| StudioError::io(&tmp, e))?;
        std::fs::rename(&tmp, &path).map_err(|e| StudioError::io(&path, e))
    }
}
