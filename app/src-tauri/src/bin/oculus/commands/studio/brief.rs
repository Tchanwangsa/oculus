//! `oculus studio brief`: one scene builder's whole input as compact text
//! or JSON (`app_lib::studio::brief`).

use super::studio_error;
use crate::*;
use app_lib::studio::{brief, libraries, videos};

impl Ctx {
    pub(crate) fn studio_brief(&self, args: &StudioBriefArgs) -> Result<(), String> {
        let project = libraries::open(&args.dir, None, None).map_err(studio_error)?;
        let index = brief::scene_index(&project, &args.scene)?;
        let dir = std::path::absolute(&args.dir)
            .map_err(|e| format!("cannot resolve {}: {e}", args.dir.display()))?;
        let target = brief::target(&dir, &videos::videos_dir());
        let brief = brief::build(&project, index, &dir, &target);
        if self.json {
            return self.emit(&brief);
        }
        print!("{brief}");
        Ok(())
    }
}
