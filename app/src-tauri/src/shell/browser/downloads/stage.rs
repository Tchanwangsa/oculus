//! The page's download hook: every download is staged under `.incoming/`,
//! and its end is matched back to it. Both events arrive on the main thread
//! (WebKit's delegate, WebView2's event loop), so nothing here blocks.
//!
//! A tab that has never shown a page when it starts downloading was opened
//! only for that: a popup, or a link, typed address or attachment opened in
//! a new tab. It leaves the strip at once (`close_tab`), and its page stays,
//! hidden, until its downloads end — a closed page never reports the end.
//! Such a page has committed no navigation, so nothing may ask it for its URL
//! (`Webview::url()` panics there); nothing here or in `close_tab` does.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::webview::DownloadEvent;
use tauri::AppHandle;

use super::correlate::Ended;
use super::deliver::run;
use super::name::{is_stream_manifest, staged_name};
use super::state::Source;
use super::{broadcast, host, incoming_root, origin, url_key};
use crate::shell::browser::layout::{close_tab, webview};
use crate::shell::browser::teardown::destroy;
use crate::shell::browser::{with_state, Inner};

/// How often a receiving download's size is read, and so the most often the
/// list is re-sent for progress.
const POLL: Duration = Duration::from_millis(500);

/// What a refused streaming playlist's row says.
const MANIFEST: &str = "this is a video stream's playlist, not the video itself";

/// What a download's row says when its staging folder could not be made.
const NOT_STAGED: &str = "there was nowhere to put it while it arrived";

/// `on_download` for the page of tab `page`. Returning false cancels.
pub(in crate::shell::browser) fn on_download(
    app: &AppHandle,
    page: u32,
    event: DownloadEvent<'_>,
) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => requested(app, page, &url, destination),
        DownloadEvent::Finished { url, path, success } => {
            finished(app, page, &url, path.as_deref(), success);
            true
        }
        _ => true,
    }
}

/// Where a download starting in `page` came from, and whether that page is a
/// download-only tab's: one that has shown no page yet.
pub(super) fn source_of(s: &Inner, page: u32, url: &url::Url) -> (Source, bool) {
    let blank = s.blank.get(&page).copied();
    let source = Source {
        page,
        // A download-only tab's download is its opener's, if it has one.
        tab: blank.flatten().unwrap_or(page),
        url_key: url_key(url),
        host: host(url),
    };
    (source, blank.is_some())
}

fn requested(app: &AppHandle, page: u32, url: &url::Url, destination: &mut PathBuf) -> bool {
    // Only WebView2's proposal is numbered against the real folder (`name`).
    let numbered_in = if cfg!(windows) {
        dirs::download_dir()
    } else {
        None
    };
    let name = staged_name(destination, numbered_in.as_deref());
    let manifest = is_stream_manifest(&name);
    let root = incoming_root();
    let (id, staged, download_only) = with_state(app, |s| {
        let (source, download_only) = source_of(s, page, url);
        if manifest {
            let id = s.downloads.refuse(source, name, MANIFEST);
            return (id, None, download_only);
        }
        let (id, staged) = s.downloads.request(source, name, &root);
        (id, Some(staged), download_only)
    });
    let from = format!(
        "from {} in tab {page}{}",
        origin(url),
        if download_only { ", download-only" } else { "" }
    );
    if download_only {
        on_main_later(app, move |app| close_tab(app, page));
    }
    let Some(staged) = staged else {
        elog!("[oculus] browser: download {id} {from} refused: a streaming playlist");
        broadcast(app);
        return false;
    };
    // WebKit fails a download whose folder is missing, so it is made first.
    let made = staged
        .parent()
        .ok_or_else(|| "no staging folder".to_string())
        .and_then(|dir| std::fs::create_dir_all(dir).map_err(|e| e.to_string()));
    if let Err(e) = made {
        elog!("[oculus] browser: download {id} {from} could not be staged: {e}");
        with_state(app, |s| s.downloads.unstaged(id, NOT_STAGED));
        broadcast(app);
        return false;
    }
    *destination = staged;
    elog!("[oculus] browser: download {id} {from}");
    if with_state(app, |s| s.downloads.start_polling()) {
        start_poller(app);
    }
    broadcast(app);
    true
}

fn finished(app: &AppHandle, page: u32, url: &url::Url, path: Option<&Path>, success: bool) {
    if success && path.is_none() {
        elog!(
            "[oculus] browser: a download from {} finished WITHOUT ITS PATH: the vendored wry is \
             not in effect (`cargo tree -i wry` must print vendor/wry). Matching it by URL",
            origin(url)
        );
    }
    let size = |p: &Path| std::fs::metadata(p).ok().map(|m| m.len());
    let ended = Ended {
        page,
        url_key: url_key(url),
        path,
        success,
    };
    let (settled, release) = with_state(app, |s| {
        let settled = s.downloads.finish(ended, size);
        // A closed tab's page was kept only to hear these.
        let mut release: Vec<u32> = settled
            .iter()
            .map(|d| d.page)
            .filter(|page| s.parked.contains(page) && !s.downloads.unfinished_in(*page))
            .collect();
        release.sort_unstable();
        release.dedup();
        for page in &release {
            s.parked.remove(page);
        }
        (settled, release)
    });
    // Not ours (a refused download, WebKit's orphan failure), or one of a
    // URL group still waiting (`correlate`): expected, so nothing is logged.
    if settled.is_empty() {
        return;
    }
    for done in settled {
        elog!(
            "[oculus] browser: download {} {}",
            done.id,
            if done.success { "finished" } else { "failed" }
        );
        run(app, done.effects);
    }
    for closed in release {
        on_main_later(app, move |app| {
            if let Some(webview) = webview(app, closed) {
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
