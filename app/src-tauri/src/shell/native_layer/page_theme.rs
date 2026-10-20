//! Page appearance is separate from the glass frame and from the main
//! macOS webview, whose media query keeps reporting the OS for System mode.

use std::sync::Mutex;
use tauri::{AppHandle, Manager, Webview};

#[derive(Default)]
pub struct PageTheme(Mutex<Option<bool>>);

#[tauri::command]
pub fn native_set_page_theme(
    app: AppHandle,
    dark: bool,
    follow_system: bool,
) -> Result<(), String> {
    // Tauri forwards the window theme to existing WebView2 pages and gives
    // it to new ones. Auto also restores the shared profile's OS preference.
    #[cfg(windows)]
    if let Some(window) = app.get_window(crate::shell::browser::MAIN) {
        window
            .set_theme(if follow_system {
                None
            } else {
                Some(if dark {
                    tauri::Theme::Dark
                } else {
                    tauri::Theme::Light
                })
            })
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(windows))]
    let _ = follow_system;
    app.run_on_main_thread({
        let app = app.clone();
        move || {
            *app.state::<PageTheme>()
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(dark);
            for (label, page) in app.webviews() {
                if label.starts_with(crate::shell::browser::LABEL_PREFIX) {
                    apply(&page);
                }
            }
        }
    })
    .map_err(|e| e.to_string())
}

/// Called for new pages too, so background tabs inherit the last preference.
#[cfg(target_os = "macos")]
pub(crate) fn apply(page: &Webview<tauri::Wry>) {
    let app = page.app_handle().clone();
    let result = page.with_webview(move |platform| {
        let Some(dark) = *app
            .state::<PageTheme>()
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
        else {
            return;
        };
        unsafe {
            use objc2::runtime::AnyObject;
            use objc2::{class, msg_send};
            use objc2_foundation::ns_string;

            let view: *mut AnyObject = platform.inner().cast();
            if view.is_null() {
                return;
            }
            let name = if dark {
                ns_string!("NSAppearanceNameDarkAqua")
            } else {
                ns_string!("NSAppearanceNameAqua")
            };
            let appearance: *mut AnyObject = msg_send![class!(NSAppearance), appearanceNamed: name];
            let _: () = msg_send![view, setAppearance: appearance];
        }
    });
    if let Err(e) = result {
        elog!("[oculus] native layer: page theme: {e}");
    }
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apply(_page: &Webview<tauri::Wry>) {}
