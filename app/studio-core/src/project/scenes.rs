//! Each scene's narration, length and word cues, resolved from the script,
//! the manifest and the theme's `narration` tokens. A narrated scene lasts
//! `max(hold, lead + clip + tail)`; a silent one lasts its `hold`; either
//! rounds up to whole frames.

use super::timeline::whole_frames;
use super::{ManifestScene, Project, SceneSpec, ScriptSection, WordCues};
use crate::error::{Result, StudioError};
use crate::render::SceneCall;

/// A scene with its length and cues resolved, as a render calls it.
#[derive(Clone, Debug)]
pub(crate) struct Staged {
    pub spec: SceneSpec,
    pub length: f64,
    /// `None` for a silent scene.
    pub cues: Option<WordCues>,
}

impl Staged {
    /// The call for this scene (index `index`) at scene-relative `t`.
    pub fn call(&self, index: usize, t: f64) -> SceneCall<'_> {
        SceneCall {
            component: &self.spec.component,
            props: &self.spec.props,
            t,
            duration: self.length,
            index,
            words: self.cues.as_ref(),
        }
    }
}

impl Project {
    /// Scene `index`'s script section, if it is narrated.
    pub fn section(&self, index: usize) -> Option<&ScriptSection> {
        let id = self.video.scenes.get(index)?.id.as_deref()?;
        self.script.as_ref()?.section(id)
    }

    /// Scene `index`'s clip: `None` for a silent scene, a `Narration`
    /// error when it has script text but the manifest has no clip for that
    /// exact text.
    pub fn narration(&self, index: usize) -> Result<Option<&ManifestScene>> {
        let Some(section) = self.section(index) else {
            self.scene(index)?;
            return Ok(None);
        };
        let clip = self.manifest.as_ref().map(|m| m.scenes.get(&section.id));
        let why = match clip {
            Some(Some(clip)) if clip.text == section.text => return Ok(Some(clip)),
            Some(Some(_)) => "its clip in narration/manifest.json is for older text",
            Some(None) => "narration/manifest.json has no clip for it",
            None => "there is no narration/manifest.json",
        };
        Err(StudioError::Narration {
            scene: section.id.clone(),
            message: format!(
                "no narration for its current text ({why}); run `oculus studio narrate {}`",
                self.dir.display()
            ),
        })
    }

    /// Scene `index`'s length in seconds, rounded up to whole frames at
    /// the video's fps (`Timeline`). Fails on stale narration and on a
    /// silent scene without a hold.
    pub fn scene_length(&self, index: usize) -> Result<f64> {
        let fps = self.fps();
        let frames = whole_frames(self.written_length(index)?, fps);
        Ok(frames as f64 / f64::from(fps))
    }

    /// The length the folder asks for: the hold for a silent scene, at
    /// least `lead + clip + tail` for a narrated one.
    fn written_length(&self, index: usize) -> Result<f64> {
        let scene = self.scene(index)?;
        let Some(clip) = self.narration(index)? else {
            return scene.hold.ok_or_else(|| self.no_hold(index, scene));
        };
        let n = self.theme.narration;
        let spoken = n.lead + clip.duration + n.tail;
        Ok(scene.hold.map_or(spoken, |h| h.max(spoken)))
    }

    /// Scene `index`'s words for `SceneCall::words`: `None` for a silent
    /// scene, an error on stale narration.
    pub fn word_cues(&self, index: usize) -> Result<Option<WordCues>> {
        let lead = self.theme.narration.lead;
        let id = self.scene(index)?.id.as_deref();
        Ok(match (self.narration(index)?, id) {
            (Some(clip), Some(id)) => Some(WordCues::new(id, clip, lead)),
            _ => None,
        })
    }

    pub(crate) fn staged(&self, index: usize) -> Result<Staged> {
        Ok(Staged {
            spec: self.scene(index)?.clone(),
            length: self.scene_length(index)?,
            cues: self.word_cues(index)?,
        })
    }

    pub(super) fn scene(&self, index: usize) -> Result<&SceneSpec> {
        self.video
            .scenes
            .get(index)
            .ok_or(StudioError::SceneOutOfRange {
                index,
                count: self.video.scenes.len(),
            })
    }

    fn no_hold(&self, index: usize, scene: &SceneSpec) -> StudioError {
        let narrate = match &scene.id {
            Some(id) => format!(", or narrate it with a `## {id}` section in script.md"),
            None => ", or give it an `id` and a script.md section to narrate it".to_owned(),
        };
        StudioError::Project {
            path: self.dir.join("video.json"),
            message: format!(
                "scenes[{index}] has no hold and no narration, so it has no length; give it `hold` (seconds){narrate}"
            ),
        }
    }
}
