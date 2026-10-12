//! `scene_fingerprint`: what in the folder scene `i`'s frames depend on,
//! as canonical JSON, for an encoder's per-scene segment cache key. Scene
//! `i`'s frames read scene `i` and, while its transition plays, scene
//! `i - 1` at its last moment; nothing else in `video.json` or the
//! manifest reaches them.

use super::Project;
use super::timeline::whole_frames;
use crate::error::{Result, StudioError};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// The fingerprint's shape; a change to what it holds bumps it, so old
/// keys stop matching.
const FORMAT: u32 = 2;

impl Project {
    /// The studio-core inputs of scene `index`'s frames, as canonical JSON
    /// bytes (object keys sorted): the video's size and fps, the theme's
    /// tokens and `video.json`'s `motion` (the transition cascade, morph
    /// colour and blur token), and for scene `index` and the scene before
    /// it, its spec (component, props, hold, transition, id), its narration
    /// words and pauses as components see them, its frame count and, for a
    /// scene without an `id`, its index (`Math.random` is seeded by
    /// component and id, or index when there is no id; see `host::seed`).
    /// An id'd scene's fingerprint therefore survives a move that keeps the
    /// scene before it.
    ///
    /// The caller adds what lives outside the folder's JSON: the contents
    /// of `components/` and `assets/`, the render settings (quality, scale,
    /// motion blur, encoder arguments) and the build of studio-core itself.
    /// Fails like `scene_length` on stale narration in either scene.
    pub fn scene_fingerprint(&self, index: usize) -> Result<Vec<u8>> {
        let previous = match index.checked_sub(1) {
            Some(i) => self.scene_inputs(i)?,
            None => Value::Null,
        };
        let mut v = Map::new();
        v.insert("format".into(), FORMAT.into());
        v.insert("size".into(), to_value(&self.video.size)?);
        v.insert("fps".into(), self.fps().into());
        v.insert("theme".into(), self.theme.tokens.clone());
        v.insert("motion".into(), to_value(&self.video.motion)?);
        v.insert("scene".into(), self.scene_inputs(index)?);
        v.insert("previous".into(), previous);
        serde_json::to_vec(&canonical(Value::Object(v))).map_err(fail)
    }

    /// One scene's spec, words, pauses, frame count and, without an id,
    /// index.
    fn scene_inputs(&self, index: usize) -> Result<Value> {
        let clip = self.narration(index)?;
        let words = clip.map(|clip| {
            let words = clip.words.iter();
            words
                .map(|w| json!([w.text, w.start, w.end]))
                .collect::<Vec<_>>()
        });
        // `pause()` reads these; a scene without them keeps its bytes.
        let pauses: Vec<Value> = clip
            .map(|c| c.pauses.iter().map(|p| json!([p.start, p.end])).collect())
            .unwrap_or_default();
        let frames = whole_frames(self.scene_length(index)?, self.fps());
        let spec = self.scene(index)?;
        let seeded_by_index = match spec.id {
            Some(_) => Value::Null,
            None => index.into(),
        };
        let mut v = Map::new();
        v.insert("spec".into(), to_value(spec)?);
        v.insert("index".into(), seeded_by_index);
        v.insert("words".into(), words.map_or(Value::Null, Value::from));
        if !pauses.is_empty() {
            v.insert("pauses".into(), pauses.into());
        }
        v.insert("frames".into(), frames.into());
        Ok(Value::Object(v))
    }
}

fn to_value(x: &impl Serialize) -> Result<Value> {
    serde_json::to_value(x).map_err(fail)
}

fn fail(e: serde_json::Error) -> StudioError {
    StudioError::Render(format!("scene fingerprint: {e}"))
}

/// `v` with every object's keys sorted, so key order in `video.json` or a
/// theme never changes the bytes.
fn canonical(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, canonical(v)))
                    .collect(),
            )
        }
        Value::Array(items) => Value::Array(items.into_iter().map(canonical).collect()),
        other => other,
    }
}
