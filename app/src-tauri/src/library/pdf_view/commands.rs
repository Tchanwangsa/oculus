//! The Tauri commands behind the PDF viewer.

use std::sync::OnceLock;

use super::documents::{open_at, page_sizes, resolve_in};
use super::links::page_links;
use super::render::{check_size, render_page};
use super::text::page_text;
use super::wire::{Link, OpenedPdf, PageText};

#[tauri::command]
pub async fn pdf_open(path: String) -> Result<OpenedPdf, String> {
    crate::runtime::blocking::run(move || {
        let pdf = open_at(&crate::library::paths::data_dir(), &path)?;
        Ok(page_sizes(&pdf))
    })
    .await
}

/// Raw RGBA8, exactly `width` × `height`, sent as a binary IPC body.
#[tauri::command]
pub async fn pdf_render(
    path: String,
    page: u32,
    width: u32,
    height: u32,
) -> Result<tauri::ipc::Response, String> {
    check_size(width, height)?;
    let rgba = on_render_slot(move || {
        let pdf = open_at(&crate::library::paths::data_dir(), &path)?;
        render_page(&pdf, page, width, height)
    })
    .await?;
    Ok(tauri::ipc::Response::new(rgba))
}

#[tauri::command]
pub async fn pdf_text(path: String, page: u32) -> Result<PageText, String> {
    on_render_slot(move || {
        let pdf = open_at(&crate::library::paths::data_dir(), &path)?;
        page_text(&pdf, page)
    })
    .await
}

#[tauri::command]
pub async fn pdf_links(path: String, page: u32) -> Result<Vec<Link>, String> {
    crate::runtime::blocking::run(move || {
        let pdf = open_at(&crate::library::paths::data_dir(), &path)?;
        page_links(&pdf, page)
    })
    .await
}

#[tauri::command]
pub async fn pdf_close(path: String) -> Result<(), String> {
    crate::library::pdf_render::forget(&resolve_in(&crate::library::paths::data_dir(), &path)?);
    Ok(())
}

/// Runs page interpretation on a big-stack render thread once one of the
/// render slots (half the cores) is free. Queued here, a request waits without
/// holding a thread.
async fn on_render_slot<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    static SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let slots = SLOTS.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        tokio::sync::Semaphore::new((cores / 2).max(1))
    });
    let _slot = slots.acquire().await.map_err(|error| error.to_string())?;
    crate::library::pdf_render::on_render_thread(work).await
}
