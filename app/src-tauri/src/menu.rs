//! The application menu, which exists for the window-level shortcuts. While a
//! browser tab's native page holds focus (`browser.rs`) the app's webview sees
//! no keys, so ⌘-shortcuts must be menu items to work everywhere. Built by hand
//! so ⌘W closes the tab (Close Window moves to ⇧⌘W).
//!
//! The items only emit; the frontend decides what they mean. A focused page
//! sees ⌘-keys before the menu, bar the chrome keys `keys.rs` reserves.

use tauri::menu::{
    AboutMetadata, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use tauri::{AppHandle, Emitter, Manager, Runtime};

const SEARCH: &str = "search";
const NEW_TAB: &str = "new-tab";
const SIDE_PANEL: &str = "side-panel";
const CLOSE_TAB: &str = "close-tab";
const REOPEN_TAB: &str = "reopen-tab";
const LAST_TAB: &str = "last-tab";
const SIDE_NEXT: &str = "side-next";
const SIDE_PREV: &str = "side-prev";
const CLOSE_WINDOW: &str = "close-window";
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

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let pkg = app.package_info();
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
            &MenuItem::with_id(app, SEARCH, "Search…", true, Some("CmdOrCtrl+K"))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, NEW_TAB, "New Tab", true, Some("CmdOrCtrl+T"))?,
            // The side panel of the tab in front, not a second tab.
            &MenuItem::with_id(
                app,
                SIDE_PANEL,
                "Side Panel",
                true,
                Some("Alt+CmdOrCtrl+T"),
            )?,
            &MenuItem::with_id(app, CLOSE_TAB, "Close Tab", true, Some("CmdOrCtrl+W"))?,
            // Always enabled: only the frontend knows if there is anything
            // to reopen.
            &MenuItem::with_id(
                app,
                REOPEN_TAB,
                "Reopen Closed Tab",
                true,
                Some("Shift+CmdOrCtrl+T"),
            )?,
            // Ignored by the frontend outside a browser tab, rather than
            // disabled, so Rust needn't track which tab is in front.
            &MenuItem::with_id(
                app,
                ADDRESS,
                "Open Location…",
                true,
                Some("CmdOrCtrl+L"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            // Not the predefined item: muda hard-wires ⌘W onto that one.
            &MenuItem::with_id(
                app,
                CLOSE_WINDOW,
                "Close Window",
                true,
                Some("Shift+CmdOrCtrl+W"),
            )?,
        ],
    )?;

    // Without this submenu ⌘C/⌘V stop working in every text field on macOS.
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, FIND, "Find…", true, Some("CmdOrCtrl+F"))?,
            &MenuItem::with_id(app, FIND_NEXT, "Find Next", true, Some("CmdOrCtrl+G"))?,
            &MenuItem::with_id(
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
            &MenuItem::with_id(app, RELOAD, "Reload Page", true, Some("CmdOrCtrl+R"))?,
            &MenuItem::with_id(
                app,
                HARD_RELOAD,
                "Reload Ignoring Cache",
                true,
                Some("Shift+CmdOrCtrl+R"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            // Page history on a browser tab, the pane's elsewhere.
            &MenuItem::with_id(app, BACK, "Back", true, Some("CmdOrCtrl+BracketLeft"))?,
            &MenuItem::with_id(
                app,
                FORWARD,
                "Forward",
                true,
                Some("CmdOrCtrl+BracketRight"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ZOOM_IN, "Zoom In", true, Some("CmdOrCtrl+Equal"))?,
            &MenuItem::with_id(app, ZOOM_OUT, "Zoom Out", true, Some("CmdOrCtrl+Minus"))?,
            &MenuItem::with_id(app, ZOOM_RESET, "Actual Size", true, Some("CmdOrCtrl+0"))?,
        ],
    )?;

    // ⌘1…⌘8 bring the nth tab forward. Numbered, not titled: titles belong
    // to the frontend.
    let slots = (1..=TAB_SLOTS)
        .map(|n| {
            MenuItem::with_id(
                app,
                format!("{TAB_SLOT}{n}"),
                format!("Tab {n}"),
                true,
                Some(format!("CmdOrCtrl+{n}")),
            )
        })
        .collect::<tauri::Result<Vec<_>>>()?;

    let last = MenuItem::with_id(app, LAST_TAB, "Last Tab", true, Some("CmdOrCtrl+9"))?;

    // ⌃, not ⌘: these walk the side panel's items, not the strip. The
    // frontend ignores them unless the side panel has focus.
    let side_separator = PredefinedMenuItem::separator(app)?;
    let side_next = MenuItem::with_id(
        app,
        SIDE_NEXT,
        "Next Side Panel Item",
        true,
        Some("Ctrl+Tab"),
    )?;
    let side_prev = MenuItem::with_id(
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
