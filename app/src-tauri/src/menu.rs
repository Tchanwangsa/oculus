//! The application menu.
//!
//! It exists for the window-level shortcuts. macOS gives the menu bar first
//! refusal on every ⌘-key, before the key reaches any webview, so ⌘W, ⌘T and
//! ⌘K have to be menu items: a `keydown` listener in the frontend never sees
//! them, and while a browser tab's native page holds focus (`browser.rs`) the
//! app's own webview sees no keys at all. For the palette that is the point —
//! ⌘K is how you get *out* of a browser tab. Tauri's default menu spends ⌘W on
//! Close Window, which a tabbed window wants for the tab — so the whole menu is
//! built here instead, with Close Window moved to ⇧⌘W.
//!
//! Windows WebView2 consumes accelerator keys before the window menu sees
//! them. Each webview forwards the app's shortcuts to the same dispatcher.
//!
//! The items only emit; the frontend owns what they mean — the strip owns what
//! a tab is (`app/src/components/tabs/TopTabBar.tsx`), the palette owns what
//! search is (`app/src/components/palette/CommandPalette.tsx`).

#[cfg(target_os = "macos")]
use tauri::menu::AboutMetadata;
use tauri::menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Runtime};

const SEARCH: &str = "search";
const NEW_TAB: &str = "new-tab";
const SPLIT: &str = "split";
const CLOSE_TAB: &str = "close-tab";
const REOPEN_TAB: &str = "reopen-tab";
const LAST_TAB: &str = "last-tab";
const CLOSE_WINDOW: &str = "close-window";
const FULLSCREEN: &str = "toggle-fullscreen";
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

/// ⌘1…⌘8 each get an item of their own; their ids are this plus the number.
/// `strip_prefix` in `handle` is why nothing else may start with it.
const TAB_SLOT: &str = "tab-slot-";
/// How many tabs the keyboard reaches **by position**. Eight, because the
/// ninth key is spent on the last tab instead (`LAST_TAB`) — which is where
/// Chrome and Safari both stop counting, and for the same reason: past eight
/// you no longer know a tab's number without looking at the strip.
const TAB_SLOTS: usize = 8;

/// Events shared by menu clicks and native WebView2 accelerators.
pub const SEARCH_EVENT: &str = "menu-search";
pub const NEW_TAB_EVENT: &str = "menu-new-tab";
pub const SPLIT_EVENT: &str = "menu-split";
pub const CLOSE_TAB_EVENT: &str = "menu-close-tab";
pub const REOPEN_TAB_EVENT: &str = "menu-reopen-tab";
pub const LAST_TAB_EVENT: &str = "menu-last-tab";
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
            &MenuItem::with_id(app, SEARCH, "Search…", true, Some("CmdOrCtrl+K"))?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, NEW_TAB, "New Tab", true, Some("CmdOrCtrl+T"))?,
            // A second pane inside the tab in front, not a second tab. A menu
            // item for the same reason ⌘T is one, and more so: the half you
            // are reaching *from* is often a browser page, whose native
            // WebView has the keys and would never pass this on to the app.
            &MenuItem::with_id(
                app,
                SPLIT,
                "Split Tab",
                true,
                Some("Alt+CmdOrCtrl+T"),
            )?,
            &MenuItem::with_id(app, CLOSE_TAB, "Close Tab", true, Some("CmdOrCtrl+W"))?,
            // Beside Close Tab rather than up with New Tab: it undoes the
            // item above it. Always enabled — whether there is anything to
            // put back is the frontend's stack to know, and greying this out
            // would mean Rust holding a copy of it.
            &MenuItem::with_id(
                app,
                REOPEN_TAB,
                "Reopen Closed Tab",
                true,
                Some("Shift+CmdOrCtrl+T"),
            )?,
            // Where Chrome and Safari both keep ⌘L. It means nothing outside a
            // browser tab, and the frontend simply ignores it there rather
            // than the item being disabled — a menu item that greys out as you
            // switch tabs needs Rust to be told which tab is in front, which is
            // a second copy of a truth the frontend already owns.
            &MenuItem::with_id(
                app,
                ADDRESS,
                "Open Location…",
                true,
                Some("CmdOrCtrl+L"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            // Not the predefined item: muda nails ⌘W onto that one, which is
            // the key we just spent on the tab.
            &MenuItem::with_id(
                app,
                CLOSE_WINDOW,
                "Close Window",
                true,
                Some("Shift+CmdOrCtrl+W"),
            )?,
        ],
    )?;

    // Copy/paste on macOS are menu key equivalents like any other — without
    // this submenu ⌘C and ⌘V stop working in every text field in the app.
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
            // Find is a browser page's, and a browser page is a native
            // WebView that has the keys — ⌘F typed into a Canvas page never
            // reaches the app's own webview at all, so a `keydown` listener
            // could only ever open the bar from outside the page it searches.
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

    // Zoom has two scopes and one pair of keys: a browser tab in front zooms
    // its *page*, anything else zooms the window. The frontend picks between
    // them (`AppLayout`) because it is the side that knows what is in front —
    // the menu only says which way.
    //
    // These are ⌘= / ⌘− / ⌘0 rather than ⌘+ : muda names the physical key, and
    // ⌘+ is ⇧⌘= on this keyboard. `AppLayout` still listens for the shifted
    // one, so both land in the same place when the app has focus.
    let view = Submenu::with_items(
        app,
        "View",
        true,
        &[
            &MenuItem::with_id(app, RELOAD, "Reload Page", true, Some("CmdOrCtrl+R"))?,
            // `location.reload()` obeys the cache; this one does not. See
            // `browser_reload` for the twenty minutes that bought.
            &MenuItem::with_id(
                app,
                HARD_RELOAD,
                "Reload Ignoring Cache",
                true,
                Some("Shift+CmdOrCtrl+R"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            // The strip's arrows, reachable from the keyboard: on a browser
            // tab they walk the page's history, everywhere else the pane's.
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

    // ⌘1…⌘5 bring the nth tab forward. In the Window submenu because that is
    // the menu about what is in front, and listed rather than left as bare key
    // equivalents because a browser tab's native WebView has the keys — the
    // one place you most want a way back to the tab you came from.
    //
    // Numbered, not titled: the frontend owns what a tab is called, and a menu
    // that named them would need every title pushed into Rust and kept in step
    // with a strip that re-titles itself on a rename.
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

    // ⌘9 is the *last* tab, not the ninth. It is the one position that still
    // means something once the strip has outrun the numbers — and the strip
    // does, at a tab a lecture and a tab a PDF. Chrome and Safari both spend
    // the key this way.
    let last = MenuItem::with_id(app, LAST_TAB, "Last Tab", true, Some("CmdOrCtrl+9"))?;

    let minimize = PredefinedMenuItem::minimize(app, None)?;
    let maximize = PredefinedMenuItem::maximize(app, None)?;
    let slots_separator = PredefinedMenuItem::separator(app)?;
    #[cfg(target_os = "macos")]
    let fullscreen_separator = PredefinedMenuItem::separator(app)?;
    #[cfg(target_os = "macos")]
    let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;
    #[cfg(target_os = "windows")]
    let fullscreen = MenuItem::with_id(app, FULLSCREEN, "Toggle Full Screen", true, Some("F11"))?;

    let mut window_items: Vec<&dyn IsMenuItem<R>> = vec![&minimize, &maximize];
    #[cfg(target_os = "macos")]
    window_items.extend([&fullscreen_separator as &dyn IsMenuItem<R>, &fullscreen]);
    #[cfg(target_os = "windows")]
    window_items.push(&fullscreen);
    window_items.push(&slots_separator);
    window_items.extend(slots.iter().map(|i| i as &dyn IsMenuItem<R>));
    window_items.push(&last);

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
    #[cfg(target_os = "windows")]
    if std::env::var_os("OCULUS_TRACE_SHORTCUTS").is_some() {
        eprintln!("[oculus] menu action={}", event.id().as_ref());
    }
    dispatch(app, event.id().as_ref());
}

fn dispatch<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        SEARCH => {
            app.emit(SEARCH_EVENT, ()).ok();
        }
        NEW_TAB => {
            app.emit(NEW_TAB_EVENT, ()).ok();
        }
        SPLIT => {
            app.emit(SPLIT_EVENT, ()).ok();
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
        FULLSCREEN => {
            app.emit("menu-toggle-fullscreen", ()).ok();
        }
        // The eight numbered slots, which carry their position rather than
        // having eight events of their own. Zero-based on the way out: the
        // frontend indexes a list, it does not count on its fingers.
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

/// WebView2 owns the keyboard while either the main UI or a remote page has
/// focus, so Windows menu accelerators alone never reach the application.
/// Install once per controller; WebView2 releases the callback on close.
#[cfg(target_os = "windows")]
pub fn attach_windows_accelerators(webview: &tauri::Webview<tauri::Wry>) {
    use webview2_com::AcceleratorKeyPressedEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
        COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetKeyState, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    let app = webview.app_handle().clone();
    if let Err(error) = webview.with_webview(move |platform| unsafe {
        let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()); };
            let mut kind = Default::default();
            args.KeyEventKind(&mut kind)?;
            if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
                && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
            {
                return Ok(());
            }
            let mut key = 0;
            args.VirtualKey(&mut key)?;
            let pressed = |key: u16| GetKeyState(key as i32) < 0;
            let ctrl = pressed(VK_CONTROL);
            let shift = pressed(VK_SHIFT);
            let alt = pressed(VK_MENU);
            let win = pressed(VK_LWIN) || pressed(VK_RWIN);
            let Some(action) = windows_accelerator(
                key,
                ctrl,
                shift,
                alt,
                win,
            ) else { return Ok(()); };
            // Release the browser's synchronous input callback before any
            // app action. Repeated presses are consumed but not dispatched.
            args.SetHandled(true)?;
            let mut status = Default::default();
            args.PhysicalKeyStatus(&mut status)?;
            // Diagnostic opt-in logs only app shortcuts, never typed text.
            if std::env::var_os("OCULUS_TRACE_SHORTCUTS").is_some() {
                eprintln!(
                    "[oculus] webview shortcut={action} kind={} ctrl={ctrl} shift={shift} alt={alt} win={win} physical_alt={} repeat={}",
                    kind.0, status.IsMenuKeyDown.as_bool(), status.WasKeyDown.as_bool(),
                );
            }
            if !status.WasKeyDown.as_bool() {
                let app = app.clone();
                std::thread::spawn(move || dispatch(&app, action));
            }
            Ok(())
        }));
        let mut token = 0;
        if let Err(error) = platform.controller().add_AcceleratorKeyPressed(&handler, &mut token) {
            eprintln!("[oculus] cannot register WebView2 shortcuts: {error}");
        }
    }) {
        eprintln!("[oculus] cannot access WebView2 shortcuts: {error}");
    }
}

#[cfg(any(target_os = "windows", test))]
fn windows_accelerator(key: u32, ctrl: bool, shift: bool, alt: bool, win: bool) -> Option<&'static str> {
    if win { return None; }
    if alt { return (key == 0x54 && ctrl && !shift).then_some(SPLIT); }
    match (key, ctrl, shift) {
        (0x7A, false, false) => Some(FULLSCREEN), // F11
        (0x4B, true, false) => Some(SEARCH),
        (0x54, true, false) => Some(NEW_TAB),
        (0x54, true, true) => Some(REOPEN_TAB),
        (0x57, true, false) => Some(CLOSE_TAB),
        (0x57, true, true) => Some(CLOSE_WINDOW),
        (0x4C, true, false) => Some(ADDRESS),
        (0x46, true, false) => Some(FIND),
        (0x47, true, false) => Some(FIND_NEXT),
        (0x47, true, true) => Some(FIND_PREV),
        (0x52, true, false) => Some(RELOAD),
        (0x52, true, true) => Some(HARD_RELOAD),
        (0xDB, true, false) => Some(BACK),
        (0xDD, true, false) => Some(FORWARD),
        (0xBB, true, _) => Some(ZOOM_IN),
        (0xBD, true, false) => Some(ZOOM_OUT),
        (0x30, true, false) => Some(ZOOM_RESET),
        (0x31, true, false) => Some("tab-slot-1"),
        (0x32, true, false) => Some("tab-slot-2"),
        (0x33, true, false) => Some("tab-slot-3"),
        (0x34, true, false) => Some("tab-slot-4"),
        (0x35, true, false) => Some("tab-slot-5"),
        (0x36, true, false) => Some("tab-slot-6"),
        (0x37, true, false) => Some("tab-slot-7"),
        (0x38, true, false) => Some("tab-slot-8"),
        (0x39, true, false) => Some(LAST_TAB),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_shortcuts_match_menu_and_leave_other_keys_to_the_page() {
        assert_eq!(windows_accelerator(0x7A, false, false, false, false), Some(FULLSCREEN));
        assert_eq!(windows_accelerator(0x4B, true, false, false, false), Some(SEARCH));
        assert_eq!(windows_accelerator(0x54, true, false, false, false), Some(NEW_TAB));
        assert_eq!(windows_accelerator(0x57, true, false, false, false), Some(CLOSE_TAB));
        assert_eq!(windows_accelerator(0x57, true, true, false, false), Some(CLOSE_WINDOW));
        for (key, ctrl, shift, alt, win) in [
            (0x4B, false, false, false, false),
            (0x4B, true, true, false, false),
            (0x4B, true, false, true, false),
            (0x4B, true, false, false, true),
            (0x7A, true, false, false, false),
            (0x43, true, false, false, false), // Copy remains native.
            (0x1B, false, false, false, false), // Escape remains page/dialog owned.
        ] {
            assert_eq!(windows_accelerator(key, ctrl, shift, alt, win), None);
        }
    }

    #[test]
    fn windows_browser_and_tab_shortcuts_reach_the_application() {
        for (key, shift, action) in [
            (0x54, true, REOPEN_TAB), (0x4C, false, ADDRESS),
            (0x46, false, FIND), (0x47, false, FIND_NEXT), (0x47, true, FIND_PREV),
            (0x52, false, RELOAD), (0x52, true, HARD_RELOAD),
            (0xDB, false, BACK), (0xDD, false, FORWARD),
            (0xBB, false, ZOOM_IN), (0xBB, true, ZOOM_IN),
            (0xBD, false, ZOOM_OUT), (0x30, false, ZOOM_RESET),
            (0x31, false, "tab-slot-1"), (0x38, false, "tab-slot-8"), (0x39, false, LAST_TAB),
        ] {
            assert_eq!(windows_accelerator(key, true, shift, false, false), Some(action));
        }
        assert_eq!(windows_accelerator(0x54, true, false, true, false), Some(SPLIT));
        assert_eq!(windows_accelerator(0x54, true, true, true, false), None);
    }
}
