//! The page's download hook: every download is staged under `.incoming/`,
//! and its end is matched back to it. Both events arrive on the main thread
//! (WebKit's delegate, WebView2's event loop), so nothing here blocks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::webview::DownloadEvent;
use tauri::AppHandle;

use super::deliver::run;
use super::name::staged_name;
use super::{broadcast, incoming_root, origin};
use crate::shell::browser::layout::{close_tab, page as webview_of};
use crate::shell::browser::teardown::destroy;
use crate::shell::browser::with_state;

/// How often a receiving download's size is read, and so the most often the
/// list is re-sent for progress.
const POLL: Duration = Duration::from_millis(500);

/// `on_download` for the page of tab `page`. Returning false cancels.
pub(in crate::shell::browser) fn on_download(
    app: &AppHandle,
    page: u32,
    event: DownloadEvent<'_>,
) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => requested(app, page, &url, destination),
        DownloadEvent::Finished { url, path, success } => {
            finished(app, &url, path.as_deref(), success);
            true
        }
        _ => true,
    }
}

fn requested(app: &AppHandle, page: u32, url: &url::Url, destination: &mut PathBuf) -> bool {
    let name = staged_name(destination, dirs::download_dir().as_deref());
    let root = incoming_root();
    let (id, staged, blank_popup) = with_state(app, |s| {
        // A popup that never showed a page opened only to download this.
        let opener = s.popups.get(&page).copied();
        let (id, staged) = s
            .downloads
            .request(page, opener.unwrap_or(page), url, name, &root);
        (id, staged, opener.is_some())
    });
    let made = staged
        .parent()
        .ok_or_else(|| "no staging folder".to_string())
        .and_then(|dir| std::fs::create_dir_all(dir).map_err(|e| e.to_string()));
    if let Err(e) = made {
        elog!(
            "[oculus] browser: could not stage a download from {}: {e}",
            origin(url)
        );
        with_state(app, |s| s.downloads.forget(id));
        return false;
    }
    *destination = staged;
    elog!(
        "[oculus] browser: download {id} from {} in tab {page}",
        origin(url)
    );
    if with_state(app, |s| s.downloads.start_polling()) {
        start_poller(app);
    }
    if blank_popup {
        on_main_later(app, move |app| close_tab(app, page));
    }
    broadcast(app);
    true
}

fn finished(app: &AppHandle, url: &url::Url, path: Option<&Path>, success: bool) {
    let size = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    let settled = with_state(app, |s| {
        let (id, effects) = s.downloads.finish(url.as_str(), path, success, size)?;
        // A closed tab's page was kept only to hear this.
        let page = s.downloads.page_of(id)?;
        let release = s.parked.contains(&page) && !s.downloads.unfinished_in(page);
        if release {
            s.parked.remove(&page);
        }
        Some((id, effects, release.then_some(page)))
    });
    let Some((id, effects, release)) = settled else {
        elog!(
            "[oculus] browser: a download from {} ended that was not staged here",
            origin(url)
        );
        return;
    };
    elog!(
        "[oculus] browser: download {id} {}",
        if success { "finished" } else { "failed" }
    );
    run(app, effects);
    if let Some(closed) = release {
        on_main_later(app, move |app| {
            if let Some(webview) = webview_of(app, closed) {
                destroy(&webview);
            }
        });
    }
    broadcast(app);
}

/// Runs `f` on the main thread after the current callback returns: the
/// download hook runs inside the page's own delegate, which must not see its
/// page closed under it. Queued through a helper thread, as popups are.
fn on_main_later(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let app = app.clone();
    std::thread::spawn(move || {
        let main = app.clone();
        main.run_on_main_thread(move || f(&app)).ok();
    });
}

/// Reads each receiving download's size every `POLL` and re-sends the list
/// when one grew; ends once nothing is receiving.
fn start_poller(app: &AppHandle) {
    let app = app.clone();
    let poll_app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("browser-downloads".into())
        .spawn(move || loop {
            std::thread::sleep(POLL);
            let Some(receiving) = with_state(&poll_app, |s| s.downloads.poll()) else {
                return;
            };
            let sizes: Vec<(u32, u64)> = receiving
                .into_iter()
                .filter_map(|(id, path)| std::fs::metadata(path).ok().map(|m| (id, m.len())))
                .collect();
            if with_state(&poll_app, |s| s.downloads.set_received(&sizes)) {
                broadcast(&poll_app);
            }
        });
    if let Err(e) = spawned {
        elog!("[oculus] browser: no download progress: {e}");
        with_state(&app, |s| s.downloads.stop_polling());
    }
}

/// Empties `.incoming/` at startup: nothing staged survives a session, so
/// anything there was left by a crash.
pub(in crate::shell::browser) fn sweep_incoming(root: &Path) {
    match std::fs::remove_dir_all(root) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => elog!("[oculus] browser: could not clear the download staging folder: {e}"),
    }
}
