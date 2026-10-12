//! The Components playground's commands: the components a video could use,
//! one component's source and live frames, and validated edits with a
//! history to undo them from, and removing a subject's own version
//! (docs/studio.md, "The playground's commands"). Each takes a component
//! name (a PascalCase identifier), a level and a subject code, and reaches
//! only the global and subject libraries.

mod engine;
mod frame;
mod history;
mod name;
mod rows;
mod source;
#[cfg(test)]
mod tests;
mod write;

use serde_json::{Map, Value};
use tauri::ipc::Response;

use super::history::Snapshot;
use super::levels::Level;
use super::videos::slot::on_render_slot;
use crate::runtime::blocking;
use crate::studio::libraries::libraries;
use frame::FrameRequest;
use rows::ComponentRow;
use source::ComponentText;

/// The components over the cascade (the subject's library over the global
/// one), each with its schema and examples. A component that does not load is
/// a row with `problem`.
#[tauri::command]
pub async fn studio_components(subject: Option<String>) -> Result<Vec<ComponentRow>, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        rows::list_in(&library, subject.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn studio_component_source(
    name: String,
    subject: Option<String>,
) -> Result<ComponentText, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        source::source_in(&library, subject.as_deref(), &name)
    })
    .await
}

/// A PNG `width` px wide of the component at `t` seconds, 16:9, in theme
/// `theme`. Props are `props`, else the example named `example`, else the
/// first example. `duration` (default 4 s) is the scene length the
/// component sees as `ctx.duration`. `theme_text`, when given, is a theme
/// file's text drawn in place of the saved `theme` (an unsaved edit).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn studio_component_frame(
    name: String,
    subject: Option<String>,
    theme: String,
    theme_text: Option<String>,
    props: Option<Map<String, Value>>,
    example: Option<String>,
    t: f64,
    width: u32,
    duration: Option<f64>,
) -> Result<Response, String> {
    let bytes = on_render_slot(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        frame::frame_png(
            &library,
            FrameRequest {
                name,
                subject,
                theme,
                theme_text,
                props,
                example,
                t,
                width,
                duration,
            },
        )
    })
    .await?;
    Ok(Response::new(bytes))
}

/// Writes the component at `level` once it loads; the text it replaces goes
/// to `.history` first. Returns the text as written.
#[tauri::command]
pub async fn studio_component_write(
    name: String,
    level: Level,
    subject: Option<String>,
    text: String,
) -> Result<ComponentText, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        write::write_in(&library, level, subject.as_deref(), &name, &text)
    })
    .await
}

/// The snapshots of the component at `level`, newest first.
#[tauri::command]
pub async fn studio_component_history(
    name: String,
    level: Level,
    subject: Option<String>,
) -> Result<Vec<Snapshot>, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        history::list_in(&library, level, subject.as_deref(), &name)
    })
    .await
}

/// Writes snapshot `id` back, snapshotting the current text first so the
/// restore can itself be undone.
#[tauri::command]
pub async fn studio_component_restore(
    name: String,
    level: Level,
    subject: Option<String>,
    id: String,
) -> Result<ComponentText, String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        history::restore_in(&library, level, subject.as_deref(), &name, &id)
    })
    .await
}

/// Removes the subject's own version of a component, so the subject falls
/// back to the global one. The text goes to `.history` first, so restore can
/// bring it back.
#[tauri::command]
pub async fn studio_component_delete(name: String, subject: String) -> Result<(), String> {
    blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        write::delete_in(&library, &subject, &name)
    })
    .await
}
