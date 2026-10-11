//! `oculus studio themes promote`: a video's theme file into a library
//! (`app_lib::studio::libraries::promote_theme`).

use super::components::{print_promoted, target};
use crate::*;
use app_lib::studio::libraries;

impl Ctx {
    pub(crate) fn studio_themes(&self, action: &StudioThemesAction) -> Result<(), String> {
        match action {
            StudioThemesAction::Promote(args) => {
                let library = libraries::libraries(false).map_err(|e| e.to_string())?;
                let promoted =
                    libraries::promote_theme(&args.dir, &args.file, target(args.to), &library)?;
                print_promoted(self, &promoted)
            }
        }
    }
}
