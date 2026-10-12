//! `oculus studio beats`: one scene's timeline of what changes on screen
//! (`SceneLook::beats`), capped for an agent's context.

use super::studio_error;
use crate::*;
use app_lib::studio::{brief, libraries};

/// Lines printed before the rest are counted as cut.
const MAX_LINES: usize = 80;

impl Ctx {
    pub(crate) fn studio_beats(&self, args: &StudioBeatsArgs) -> Result<(), String> {
        let project = libraries::open(&args.dir, None, None).map_err(studio_error)?;
        let index = brief::scene_index(&project, &args.scene)?;
        let look = project
            .look(&[index])
            .pop()
            .ok_or("no samples")?
            .map_err(studio_error)?;
        let beats = look.beats();
        if self.json {
            return self.emit(&serde_json::json!({
                "index": index,
                "id": look.id,
                "component": look.component,
                "length": look.length,
                "beats": beats,
            }));
        }
        println!(
            "{} ({}), {:.2} s, {} beat{}; scene seconds",
            look.name(),
            look.component,
            look.length,
            beats.len(),
            if beats.len() == 1 { "" } else { "s" }
        );
        for b in beats.iter().take(MAX_LINES) {
            println!("{}", b.line());
        }
        if beats.len() > MAX_LINES {
            println!(
                "… {} more cut; --json gives every beat",
                beats.len() - MAX_LINES
            );
        }
        Ok(())
    }
}
