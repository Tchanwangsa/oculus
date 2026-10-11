//! The starter set: component and theme files compiled into the crate, so
//! seeding needs no file beside the binary.

use crate::error::{Result, StudioError};
use crate::project::{ComponentSource, valid_name};

/// `(name, TSX)` for each starter component, in `library/components/`.
pub const STARTER_COMPONENTS: &[(&str, &str)] = &[];

/// `(name, JSON)` for each starter theme, in `library/themes/`.
pub const STARTER_THEMES: &[(&str, &str)] = &[];

/// The starter components as a `ComponentSource` (no folder involved).
pub struct Starter;

impl ComponentSource for Starter {
    fn read(&self, name: &str) -> Result<String> {
        STARTER_COMPONENTS
            .iter()
            .find(|(n, _)| valid_name(name) && *n == name)
            .map(|(_, text)| (*text).to_owned())
            .ok_or_else(|| StudioError::ComponentNotFound {
                name: name.to_owned(),
                message: format!(
                    "not in the starter set; available: {}",
                    self.names().join(", ")
                ),
            })
    }

    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = STARTER_COMPONENTS
            .iter()
            .map(|(n, _)| (*n).to_owned())
            .collect();
        names.sort();
        names
    }
}
