//! The Studio page's commands: the videos under `<data>/studio/videos/`, one
//! video's detail, and PNG stills of it (docs/studio.md, "The Studio page's
//! commands"). Each takes a slug and reaches no folder outside `videos/`.

mod detail;
mod frame;
mod new;
mod rows;
pub(super) mod slot;
#[cfg(test)]
mod tests;
mod thumbnail;

use tauri::ipc::Response;

use crate::db::store::{open_pool, setting, studio_links};
use crate::studio::libraries::libraries;
use crate::studio::videos::videos_dir;
use crate::studio::voice;
use detail::VideoDetail;
use frame::FrameAt;
use new::{Made, NewVideoOptions};
use rows::VideoView;
use slot::on_render_slot;

/// The videos newest first; `subject` is a course code.
#[tauri::command]
pub async fn studio_videos(subject: Option<String>) -> Result<Vec<VideoView>, String> {
    let links = studio_links(&open_pool().await?).await?;
    crate::runtime::blocking::run(move || rows::list_in(&videos_dir(), subject.as_deref(), &links))
        .await
}

/// Scaffold a video; its slug comes from the title.
#[tauri::command]
pub async fn studio_new_video(options: NewVideoOptions) -> Result<Made, String> {
    crate::runtime::blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        new::create_in(&videos_dir(), &library, options)
    })
    .await
}

#[tauri::command]
pub async fn studio_video_detail(slug: String) -> Result<VideoDetail, String> {
    let pool = open_pool().await?;
    let links = studio_links(&pool).await?;
    let row = setting(&pool, voice::SETTINGS_KEY).await?;
    let settings_voice = voice::settings_from(row.as_deref()).voice;
    crate::runtime::blocking::run(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        detail::detail_in(
            &videos_dir(),
            &library,
            &links,
            settings_voice.as_deref(),
            &slug,
        )
    })
    .await
}

/// Turns the video's burned-in captions on or off (video.json's
/// `captions`); the next render re-renders its narrated scenes.
#[tauri::command]
pub async fn studio_set_captions(slug: String, on: bool) -> Result<(), String> {
    crate::runtime::blocking::run(move || {
        let dir = crate::studio::videos::video_in(&videos_dir(), &slug)?;
        crate::studio::videos::set_captions(&dir, on)
    })
    .await
}

/// The WebVTT captions beside the video's `preset` render
/// (`renders/<preset>.vtt`), for the player's CC.
#[tauri::command]
pub async fn studio_render_captions(slug: String, preset: String) -> Result<String, String> {
    crate::runtime::blocking::run(move || {
        let preset = crate::studio::render::Preset::parse(&preset)?;
        let dir = crate::studio::videos::video_in(&videos_dir(), &slug)?;
        let path = preset.default_out(&dir).with_extension("vtt");
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    })
    .await
}

/// A PNG `width` pixels wide. Rejects with the reason when the narration is
/// stale.
#[tauri::command]
pub async fn studio_frame(slug: String, at: FrameAt, width: u32) -> Result<Response, String> {
    let bytes = on_render_slot(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        frame::frame_png(&videos_dir(), &library, &slug, at, width)
    })
    .await?;
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn studio_thumbnail(slug: String) -> Result<Response, String> {
    let bytes = on_render_slot(move || {
        let library = libraries(false).map_err(|e| e.to_string())?;
        thumbnail::thumbnail_in(&videos_dir(), &library, &slug)
    })
    .await?;
    Ok(Response::new(bytes))
}
