//! WebView2 routes focused-page keys through its controller rather than the
//! host's Win32 message loop. Dispatch app shortcuts through the same menu
//! handler, leaving browser editing and find keys with the page.

use super::*;

#[cfg(any(target_os = "windows", test))]
pub(super) fn menu_label<'a>(
    text: &str,
    accelerator: Option<&'a str>,
) -> (String, Option<&'a str>) {
    let Some(key) = accelerator else {
        return (text.to_string(), None);
    };
    if !key.contains("Ctrl") {
        return (text.to_string(), Some(key));
    }
    // The host message loop runs TranslateAccelerator before WebView2 sees
    // keys. Custom Ctrl shortcuts belong only to the controller callback.
    let label = key
        .replace("CmdOrCtrl", "Ctrl")
        .replace("Shift+Ctrl+", "Ctrl+Shift+")
        .replace("BracketLeft", "[")
        .replace("BracketRight", "]")
        .replace("Equal", "=")
        .replace("Minus", "-");
    (format!("{text}\t{label}"), None)
}

#[derive(Debug, PartialEq)]
enum Shortcut {
    Item(&'static str),
    Slot(usize),
}

impl Shortcut {
    fn focuses_shell(&self, page: bool) -> bool {
        page && matches!(
            self,
            Self::Slot(_)
                | Self::Item(
                    SEARCH
                        | ADDRESS
                        | NEW_TAB
                        | CLOSE_TAB
                        | REOPEN_TAB
                        | LAST_TAB
                        | SIDE_PANEL
                        | SIDE_NEXT
                        | SIDE_PREV
                )
        )
    }
}

fn shortcut(key: u32, ctrl: bool, shift: bool, alt: bool, page: bool) -> Option<Shortcut> {
    if !ctrl || alt {
        // Ctrl+Alt can be AltGr on international keyboard layouts.
        return None;
    }
    let id = match (key, shift) {
        (0x4B, false) => SEARCH,
        (0x54, false) => NEW_TAB,
        (0x54, true) => REOPEN_TAB,
        (0x59, true) => SIDE_PANEL,
        (0x57, false) => CLOSE_TAB,
        (0x57, true) => CLOSE_WINDOW,
        (0x4C, false) => ADDRESS,
        (0x31..=0x38, false) => return Some(Shortcut::Slot((key - 0x31) as usize)),
        (0x39, false) => LAST_TAB,
        (0x09, false) => SIDE_NEXT,
        (0x09, true) => SIDE_PREV,
        (0x52, false) => RELOAD,
        (0x52, true) => HARD_RELOAD,
        (0xDB, false) => BACK,
        (0xDD, false) => FORWARD,
        (0xBB, _) => ZOOM_IN,
        (0xBD, false) => ZOOM_OUT,
        (0x30, false) => ZOOM_RESET,
        // A browser site owns its editing history, selection and find.
        (0x5A, false) if !page => UNDO,
        (0x5A, true) if !page => REDO,
        (0x41, false) if !page => SELECT_ALL,
        (0x46, false) if !page => FIND,
        (0x47, false) if !page => FIND_NEXT,
        (0x47, true) if !page => FIND_PREV,
        _ => return None,
    };
    Some(Shortcut::Item(id))
}

#[cfg(target_os = "windows")]
pub(crate) fn install(webview: &tauri::Webview<tauri::Wry>, page: bool) {
    use webview2_com::AcceleratorKeyPressedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
        COREWEBVIEW2_PHYSICAL_KEY_STATUS,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    let app = webview.app_handle().clone();
    let label = webview.label().to_string();
    let result = webview.with_webview(move |platform| {
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            unsafe {
                let mut kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
                args.KeyEventKind(&mut kind)?;
                if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                    && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
                {
                    return Ok(());
                }
                let down = |key| GetKeyState(key as i32) < 0;
                if down(VK_LWIN) || down(VK_RWIN) {
                    return Ok(());
                }
                let mut key = 0;
                args.VirtualKey(&mut key)?;
                let Some(action) =
                    shortcut(key, down(VK_CONTROL), down(VK_SHIFT), down(VK_MENU), page)
                else {
                    return Ok(());
                };
                args.SetHandled(true)?;
                let mut status = COREWEBVIEW2_PHYSICAL_KEY_STATUS::default();
                args.PhysicalKeyStatus(&mut status)?;
                if status.WasKeyDown.as_bool() {
                    return Ok(());
                }
                // Windowed WebView2 blocks cross-process COM during this
                // callback. Queue the action behind it, including close.
                let app = app.clone();
                std::thread::spawn(move || {
                    app.clone()
                        .run_on_main_thread(move || {
                            // DOM autofocus cannot move focus between native
                            // WebView2 controllers. Focus the shell before its
                            // event; a destination browser reclaims it on place.
                            if action.focuses_shell(page) {
                                if let Some(main) = app.get_webview("main") {
                                    if let Err(error) = main.set_focus() {
                                        elog!("[oculus] could not focus the shell: {error}");
                                    }
                                }
                            }
                            let id = match action {
                                Shortcut::Item(id) => id.to_string(),
                                Shortcut::Slot(slot) => format!("{TAB_SLOT}{}", slot + 1),
                            };
                            handle(&app, MenuEvent { id: id.into() });
                        })
                        .ok();
                });
            }
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = unsafe {
            platform
                .controller()
                .add_AcceleratorKeyPressed(&handler, &mut token)
        } {
            elog!("[oculus] {label}: could not register Windows shortcuts: {error}");
        }
        // The controller owns the callback until the webview closes.
    });
    if let Err(error) = result {
        elog!("[oculus] could not access WebView2 for shortcuts: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_ctrl_keys_are_labels_without_native_accelerators() {
        for key in [
            "CmdOrCtrl+K",
            "CmdOrCtrl+T",
            "CmdOrCtrl+A",
            "CmdOrCtrl+Z",
            "CmdOrCtrl+F",
            "Shift+CmdOrCtrl+Z",
            "Ctrl+Tab",
            "Ctrl+Shift+Y",
        ] {
            let (label, native) = menu_label("Action", Some(key));
            assert!(label.starts_with("Action\t"));
            assert!(label.contains("Ctrl"));
            assert_eq!(native, None, "the native menu must not consume {key}");
        }
        assert_eq!(
            menu_label("Redo", Some("Shift+CmdOrCtrl+Z")),
            ("Redo\tCtrl+Shift+Z".into(), None)
        );
        assert_eq!(
            menu_label("Exit", Some("Alt+F4")),
            ("Exit".into(), Some("Alt+F4"))
        );
    }

    #[test]
    fn shell_shortcuts_work_in_app_and_browser_pages() {
        for page in [false, true] {
            assert_eq!(
                shortcut(0x4B, true, false, false, page),
                Some(Shortcut::Item(SEARCH))
            );
            assert_eq!(
                shortcut(0x54, true, false, false, page),
                Some(Shortcut::Item(NEW_TAB))
            );
            assert_eq!(
                shortcut(0x54, true, true, false, page),
                Some(Shortcut::Item(REOPEN_TAB))
            );
            assert_eq!(
                shortcut(0x59, true, true, false, page),
                Some(Shortcut::Item(SIDE_PANEL))
            );
            assert_eq!(
                shortcut(0x31, true, false, false, page),
                Some(Shortcut::Slot(0))
            );
            assert_eq!(
                shortcut(0x38, true, false, false, page),
                Some(Shortcut::Slot(7))
            );
            assert_eq!(
                shortcut(0x39, true, false, false, page),
                Some(Shortcut::Item(LAST_TAB))
            );
        }
    }

    #[test]
    fn browser_pages_keep_editing_and_find_keys() {
        for key in [0x5A, 0x41, 0x46, 0x47, 0x43, 0x56, 0x58] {
            for shift in [false, true] {
                assert_eq!(shortcut(key, true, shift, false, true), None);
            }
        }
        assert_eq!(
            shortcut(0x5A, true, false, false, false),
            Some(Shortcut::Item(UNDO))
        );
        assert_eq!(
            shortcut(0x5A, true, true, false, false),
            Some(Shortcut::Item(REDO))
        );
        assert_eq!(
            shortcut(0x41, true, false, false, false),
            Some(Shortcut::Item(SELECT_ALL))
        );
        assert_eq!(
            shortcut(0x46, true, false, false, false),
            Some(Shortcut::Item(FIND))
        );
    }

    #[test]
    fn browser_shell_actions_transfer_focus_without_taking_page_actions() {
        for action in [
            Shortcut::Item(SEARCH),
            Shortcut::Item(ADDRESS),
            Shortcut::Item(NEW_TAB),
            Shortcut::Item(CLOSE_TAB),
            Shortcut::Item(REOPEN_TAB),
            Shortcut::Item(LAST_TAB),
            Shortcut::Item(SIDE_PANEL),
            Shortcut::Item(SIDE_NEXT),
            Shortcut::Item(SIDE_PREV),
            Shortcut::Slot(0),
            Shortcut::Slot(7),
        ] {
            assert!(action.focuses_shell(true), "{action:?}");
            assert!(!action.focuses_shell(false), "{action:?}");
        }
        for id in [
            UNDO,
            REDO,
            SELECT_ALL,
            FIND,
            FIND_NEXT,
            FIND_PREV,
            RELOAD,
            HARD_RELOAD,
            BACK,
            FORWARD,
            ZOOM_IN,
            ZOOM_OUT,
            ZOOM_RESET,
            CLOSE_WINDOW,
        ] {
            assert!(!Shortcut::Item(id).focuses_shell(true), "{id}");
        }
    }

    #[test]
    fn bare_keys_altgr_and_unassigned_combinations_pass_through() {
        assert_eq!(shortcut(0x54, false, false, false, false), None);
        assert_eq!(shortcut(0x54, true, false, true, false), None);
        assert_eq!(shortcut(0x59, true, false, false, false), None);
        assert_eq!(shortcut(0x4B, true, true, false, false), None);
    }
}
