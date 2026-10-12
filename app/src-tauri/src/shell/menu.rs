//! The application menu, which exists for the window-level shortcuts. While a
//! browser tab's native page holds focus (`shell/browser/`) the app's webview sees
//! no keys, so ⌘-shortcuts must be menu items to work everywhere. Built by hand
//! so ⌘W closes the tab (Close Window moves to ⇧⌘W).
//!
//! The items emit and the frontend decides what they mean, except that Undo,
//! Redo and Select All act natively while a browser page holds focus. A
//! focused page sees ⌘-keys before the menu, bar the chrome keys `keys.rs`
//! reserves.

#[cfg(target_os = "macos")]
use tauri::menu::AboutMetadata;
use tauri::menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Runtime};

#[cfg(any(target_os = "windows", test))]
pub(crate) mod windows;

const SEARCH: &str = "search";
const NEW_TAB: &str = "new-tab";
const SIDE_PANEL: &str = "side-panel";
const CLOSE_TAB: &str = "close-tab";
const REOPEN_TAB: &str = "reopen-tab";
const LAST_TAB: &str = "last-tab";
const SIDE_NEXT: &str = "side-next";
const SIDE_PREV: &str = "side-prev";
const CLOSE_WINDOW: &str = "close-window";
const UNDO: &str = "undo";
const REDO: &str = "redo";
const SELECT_ALL: &str = "select-all";
const FIND: &str = "find";
const FIND_NEXT: &str = "find-next";
const FIND_PREV: &str = "find-prev";
const ZOOM_IN: &str = "zoom-in";
const ZOOM_OUT: &str = "zoom-out";
const ZOOM_RESET: &str = "zoom-reset";
const RELOAD: &str = "reload";
const HARD_RELOAD: &str = "hard-reload";
const BACK: &str = "back";
const FORWARD: &str = "forward";
const ADDRESS: &str = "address";
#[cfg(target_os = "windows")]
const EXIT: &str = "exit";
#[cfg(target_os = "windows")]
const SIDE_PANEL_KEY: &str = "Ctrl+Shift+Y";
#[cfg(not(target_os = "windows"))]
const SIDE_PANEL_KEY: &str = "Alt+CmdOrCtrl+T";

/// Ids of the ⌘1…⌘8 items are this plus the number; no other id may start
/// with it (`handle` strips it).
const TAB_SLOT: &str = "tab-slot-";
/// ⌘9 is the last tab (`LAST_TAB`), as in Chrome and Safari.
const TAB_SLOTS: usize = 8;

/// Events the frontend listens for.
pub const SEARCH_EVENT: &str = "menu-search";
pub const NEW_TAB_EVENT: &str = "menu-new-tab";
pub const SIDE_PANEL_EVENT: &str = "menu-side-panel";
pub const CLOSE_TAB_EVENT: &str = "menu-close-tab";
pub const REOPEN_TAB_EVENT: &str = "menu-reopen-tab";
pub const LAST_TAB_EVENT: &str = "menu-last-tab";
pub const SIDE_NEXT_EVENT: &str = "menu-side-next";
pub const SIDE_PREV_EVENT: &str = "menu-side-prev";
/// Carries the **zero-based** index of the slot pressed.
pub const SELECT_TAB_EVENT: &str = "menu-select-tab";
pub const UNDO_EVENT: &str = "menu-undo";
pub const REDO_EVENT: &str = "menu-redo";
pub const SELECT_ALL_EVENT: &str = "menu-select-all";
pub const FIND_EVENT: &str = "menu-find";
pub const FIND_NEXT_EVENT: &str = "menu-find-next";
pub const FIND_PREV_EVENT: &str = "menu-find-prev";
pub const ZOOM_IN_EVENT: &str = "menu-zoom-in";
pub const ZOOM_OUT_EVENT: &str = "menu-zoom-out";
pub const ZOOM_RESET_EVENT: &str = "menu-zoom-reset";
pub const RELOAD_EVENT: &str = "menu-reload";
pub const HARD_RELOAD_EVENT: &str = "menu-hard-reload";
pub const BACK_EVENT: &str = "menu-back";
pub const FORWARD_EVENT: &str = "menu-forward";
pub const ADDRESS_EVENT: &str = "menu-address";

fn item<R: Runtime>(
    app: &AppHandle<R>,
    id: impl Into<tauri::menu::MenuId>,
    text: impl AsRef<str>,
    enabled: bool,
    accelerator: Option<impl AsRef<str>>,
) -> tauri::Result<MenuItem<R>> {
    #[cfg(target_os = "windows")]
    {
        let (text, accelerator) =
            windows::menu_label(text.as_ref(), accelerator.as_ref().map(AsRef::as_ref));
        MenuItem::with_id(app, id, text, enabled, accelerator)
    }
    #[cfg(not(target_os = "windows"))]
    MenuItem::with_id(app, id, text, enabled, accelerator)
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    #[cfg(target_os = "macos")]
    let pkg = app.package_info();
    #[cfg(target_os = "macos")]
    let about = AboutMetadata {
        name: Some(pkg.name.clone()),
        version: Some(pkg.version.to_string()),
        ..Default::default()
    };

    let file = Submenu::with_items(
        app,
        "File",
        true,
        &[
            &item(app, SEARCH, "Search…", true, Some("CmdOrCtrl+K"))?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, NEW_TAB, "New Tab", true, Some("CmdOrCtrl+T"))?,
            // The side panel of the tab in front, not a second tab.
            &item(app, SIDE_PANEL, "Side Panel", true, Some(SIDE_PANEL_KEY))?,
            &item(app, CLOSE_TAB, "Close Tab", true, Some("CmdOrCtrl+W"))?,
            // Always enabled: only the frontend knows if there is anything
            // to reopen.
            &item(
                app,
                REOPEN_TAB,
                "Reopen Closed Tab",
                true,
                Some("Shift+CmdOrCtrl+T"),
            )?,
            // Ignored by the frontend outside a browser tab, rather than
            // disabled, so Rust needn't track which tab is in front.
            &item(app, ADDRESS, "Open Location…", true, Some("CmdOrCtrl+L"))?,
            &PredefinedMenuItem::separator(app)?,
            // Not the predefined item: muda hard-wires ⌘W onto that one.
            &item(
                app,
                CLOSE_WINDOW,
                "Close Window",
                true,
                Some("Shift+CmdOrCtrl+W"),
            )?,
            #[cfg(target_os = "windows")]
            &PredefinedMenuItem::separator(app)?,
            #[cfg(target_os = "windows")]
            &item(app, EXIT, "Exit", true, Some("Alt+F4"))?,
        ],
    )?;

    // Without this submenu ⌘C/⌘V stop working in every text field on macOS.
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            // Undo, Redo and Select All are not the predefined items: those
            // hand ⌘Z to WebKit's own undo stack, which a note's history never
            // fills. The frontend routes them, bar a focused browser page (`edit`).
            &item(app, UNDO, "Undo", true, Some("CmdOrCtrl+Z"))?,
            &item(app, REDO, "Redo", true, Some("Shift+CmdOrCtrl+Z"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &item(app, SELECT_ALL, "Select All", true, Some("CmdOrCtrl+A"))?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, FIND, "Find…", true, Some("CmdOrCtrl+F"))?,
            &item(app, FIND_NEXT, "Find Next", true, Some("CmdOrCtrl+G"))?,
            &item(
                app,
                FIND_PREV,
                "Find Previous",
                true,
                Some("Shift+CmdOrCtrl+G"),
            )?,
        ],
    )?;

    // Zoom: `AppLayout` picks page or window zoom by what is in front.
    // ⌘= not ⌘+: muda names the physical key, and ⌘+ is ⇧⌘=.
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &item(app, RELOAD, "Reload Page", true, Some("CmdOrCtrl+R"))?,
            &item(
                app,
                HARD_RELOAD,
                "Reload Ignoring Cache",
                true,
                Some("Shift+CmdOrCtrl+R"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            // Page history on a browser tab, the pane's elsewhere.
            &item(app, BACK, "Back", true, Some("CmdOrCtrl+BracketLeft"))?,
            &item(
                app,
                FORWARD,
                "Forward",
                true,
                Some("CmdOrCtrl+BracketRight"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &item(app, ZOOM_IN, "Zoom In", true, Some("CmdOrCtrl+Equal"))?,
            &item(app, ZOOM_OUT, "Zoom Out", true, Some("CmdOrCtrl+Minus"))?,
            &item(app, ZOOM_RESET, "Actual Size", true, Some("CmdOrCtrl+0"))?,
        ],
    )?;

    // ⌘1…⌘8 bring the nth tab forward. Numbered, not titled: titles belong
    // to the frontend.
    let slots = (1..=TAB_SLOTS)
        .map(|n| {
            item(
                app,
                format!("{TAB_SLOT}{n}"),
                format!("Tab {n}"),
                true,
                Some(format!("CmdOrCtrl+{n}")),
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;

    let last = item(app, LAST_TAB, "Last Tab", true, Some("CmdOrCtrl+9"))?;

    // ⌃, not ⌘: these walk the side panel's items, not the strip. The
    // frontend ignores them unless the side panel has focus.
    let side_separator = PredefinedMenuItem::separator(app)?;
    let side_next = item(
        app,
        SIDE_NEXT,
        "Next Side Panel Item",
        true,
        Some("Ctrl+Tab"),
    )?;
    let side_prev = item(
        app,
        SIDE_PREV,
        "Previous Side Panel Item",
        true,
        Some("Ctrl+Shift+Tab"),
    )?;

    let minimize = PredefinedMenuItem::minimize(app, None)?;
    let maximize = PredefinedMenuItem::maximize(app, None)?;
    let slots_separator = PredefinedMenuItem::separator(app)?;
    #[cfg(target_os = "macos")]
    let fullscreen_separator = PredefinedMenuItem::separator(app)?;
    #[cfg(target_os = "macos")]
    let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;

    let mut window_items: Vec<&dyn IsMenuItem<R>> = vec![&minimize, &maximize];
    #[cfg(target_os = "macos")]
    window_items.extend([&fullscreen_separator as &dyn IsMenuItem<R>, &fullscreen]);
    window_items.push(&slots_separator);
    window_items.extend(slots.iter().map(|i| i as &dyn IsMenuItem<R>));
    window_items.push(&last);
    window_items.push(&side_separator);
    window_items.push(&side_next);
    window_items.push(&side_prev);

    let window = Submenu::with_items(app, "Window", true, &window_items)?;

    Menu::with_items(
        app,
        &[
            #[cfg(target_os = "macos")]
            &Submenu::with_items(
                app,
                pkg.name.clone(),
                true,
                &[
                    &PredefinedMenuItem::about(app, None, Some(about))?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::services(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::hide(app, None)?,
                    &PredefinedMenuItem::hide_others(app, None)?,
                    &PredefinedMenuItem::show_all(app, None)?,
                    &PredefinedMenuItem::separator(app)?,
                    &PredefinedMenuItem::quit(app, None)?,
                ],
            )?,
            &file,
            &edit,
            &view,
            &window,
        ],
    )
}

pub fn handle<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        #[cfg(target_os = "windows")]
        EXIT => app.exit(0),
        SEARCH => {
            app.emit(SEARCH_EVENT, ()).ok();
        }
        NEW_TAB => {
            app.emit(NEW_TAB_EVENT, ()).ok();
        }
        SIDE_PANEL => {
            app.emit(SIDE_PANEL_EVENT, ()).ok();
        }
        CLOSE_TAB => {
            app.emit(CLOSE_TAB_EVENT, ()).ok();
        }
        REOPEN_TAB => {
            app.emit(REOPEN_TAB_EVENT, ()).ok();
        }
        LAST_TAB => {
            app.emit(LAST_TAB_EVENT, ()).ok();
        }
        SIDE_NEXT => {
            app.emit(SIDE_NEXT_EVENT, ()).ok();
        }
        SIDE_PREV => {
            app.emit(SIDE_PREV_EVENT, ()).ok();
        }
        UNDO => edit(app, UNDO_EVENT, c"undo:"),
        REDO => edit(app, REDO_EVENT, c"redo:"),
        SELECT_ALL => edit(app, SELECT_ALL_EVENT, c"selectAll:"),
        FIND => {
            app.emit(FIND_EVENT, ()).ok();
        }
        FIND_NEXT => {
            app.emit(FIND_NEXT_EVENT, ()).ok();
        }
        FIND_PREV => {
            app.emit(FIND_PREV_EVENT, ()).ok();
        }
        ZOOM_IN => {
            app.emit(ZOOM_IN_EVENT, ()).ok();
        }
        ZOOM_OUT => {
            app.emit(ZOOM_OUT_EVENT, ()).ok();
        }
        ZOOM_RESET => {
            app.emit(ZOOM_RESET_EVENT, ()).ok();
        }
        RELOAD => {
            app.emit(RELOAD_EVENT, ()).ok();
        }
        HARD_RELOAD => {
            app.emit(HARD_RELOAD_EVENT, ()).ok();
        }
        BACK => {
            app.emit(BACK_EVENT, ()).ok();
        }
        FORWARD => {
            app.emit(FORWARD_EVENT, ()).ok();
        }
        ADDRESS => {
            app.emit(ADDRESS_EVENT, ()).ok();
        }
        CLOSE_WINDOW => {
            if let Some(window) = app.get_focused_window() {
                window.close().ok();
            }
        }
        // Numbered slots emit a zero-based index.
        other => {
            if let Some(n) = other
                .strip_prefix(TAB_SLOT)
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|n| *n >= 1)
            {
                app.emit(SELECT_TAB_EVENT, n - 1).ok();
            }
        }
    }
}

/// Sends `action` down the responder chain when a browser page holds focus,
/// as the predefined item would (the page already saw the key, `keys.rs`);
/// otherwise emits `event` for the frontend.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn edit<R: Runtime>(app: &AppHandle<R>, event: &str, action: &std::ffi::CStr) {
    #[cfg(target_os = "macos")]
    if let Some(mtm) = objc2::MainThreadMarker::new() {
        if crate::shell::keys::focused_page(mtm).is_some() {
            let ns_app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            let sel = objc2::runtime::Sel::register(action);
            unsafe { ns_app.sendAction_to_from(sel, None, None) };
            return;
        }
    }
    app.emit(event, ()).ok();
}
