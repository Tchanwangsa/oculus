//! A move that failed at the app's launch: said in a dialog, and the app
//! closes before its window loads, so nothing runs against a half-moved
//! library.

use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

pub fn refuse_to_start(app: &tauri::App, error: &str) {
    use tauri::Manager;
    for window in app.webview_windows().into_values() {
        window.destroy().ok();
    }
    app.dialog()
        .message(format!(
            "Oculus could not finish moving your library into its new folder layout, \
             so it will not start. Nothing was copied, and the move resumes \
             where it stopped the next time Oculus opens.\n\n{error}"
        ))
        .title("Oculus could not move your library")
        .kind(MessageDialogKind::Error)
        .blocking_show();
    app.handle().exit(1);
}
