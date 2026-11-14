use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use super::deliver::run;
use super::{broadcast, snapshot, Dest, Snapshot};
use crate::shell::browser::with_state;

/// For the frontend's mount, which may have missed earlier events.
#[tauri::command]
pub fn browser_downloads(app: AppHandle) -> Snapshot {
    snapshot(&app)
}

/// The Save dialog's answer. Delivered at once if every byte is in, else as
/// soon as they are; the result arrives on `browser-downloads`.
#[tauri::command]
pub fn browser_download_choose(app: AppHandle, id: u32, dest: Dest) -> Result<(), String> {
    let kind = match &dest {
        Dest::Oculus { .. } => "Oculus",
        Dest::Path { .. } => "a path",
        Dest::Discard => "discard",
    };
    let effects = with_state(&app, |s| s.downloads.choose(id, dest))?;
    elog!("[oculus] browser: download {id} → {kind}");
    run(&app, effects);
    broadcast(&app);
    Ok(())
}

/// The native save panel in `~/Downloads`, suggesting the download's name.
/// `None` when cancelled. A path it returns may replace the file there,
/// which the panel has confirmed; the caller then chooses `Dest::Path`.
#[tauri::command]
pub async fn browser_download_save_panel(
    app: AppHandle,
    id: u32,
) -> Result<Option<String>, String> {
    let name = with_state(&app, |s| s.downloads.name(id)).ok_or("no such download")?;
    crate::runtime::blocking::run(move || {
        let mut panel = app
            .dialog()
            .file()
            .set_title("Save download")
            .set_file_name(name);
        if let Some(dir) = dirs::download_dir() {
            panel = panel.set_directory(dir);
        }
        let Some(picked) = panel.blocking_save_file() else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|e| e.to_string())?;
        let shown = path.to_string_lossy().into_owned();
        with_state(&app, |s| s.downloads.confirm(id, path));
        Ok(Some(shown))
    })
    .await
}

/// Shows a saved download in Finder (Explorer on Windows).
#[tauri::command]
pub fn browser_download_reveal(app: AppHandle, id: u32) -> Result<(), String> {
    let data_dir = crate::library::paths::data_dir();
    let path = with_state(&app, |s| s.downloads.saved_path(id, &data_dir))?;
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| e.to_string())
}

/// Opens a saved download in its default app.
#[tauri::command]
pub fn browser_download_open(app: AppHandle, id: u32) -> Result<(), String> {
    let data_dir = crate::library::paths::data_dir();
    let path = with_state(&app, |s| s.downloads.saved_path(id, &data_dir))?;
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| e.to_string())
}

/// Removes a saved, discarded, failed or refused download from the list.
#[tauri::command]
pub fn browser_download_clear(app: AppHandle, id: u32) -> Result<(), String> {
    with_state(&app, |s| s.downloads.clear(id))?;
    broadcast(&app);
    Ok(())
}
