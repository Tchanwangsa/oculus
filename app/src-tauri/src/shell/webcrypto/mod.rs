//! WebKit's WebCrypto master key from oculus-keyd (`docs/auth.md`), handed
//! to WebKit by a website data store delegate (`delegate.rs`) installed on
//! the default store shared by the app's webviews, at setup. `fetch` blocks on
//! keyd, which can wait on a keychain prompt, so the delegate calls it off the
//! main thread, once per process (`cache.rs`).

mod cache;
mod delegate;

use keyd_core::client::ServedKey;
use objc2::MainThreadMarker;
use objc2_foundation::{NSBundle, NSProcessInfo};
use objc2_web_kit::WKWebsiteDataStore;
use tauri::AppHandle;

use crate::providers::credentials::{Credentialed, KeydError};

pub use delegate::install;

/// Installs the delegate on WebKit's default store, which the main webview,
/// browser tabs and the Canvas sign-in window share. On the main thread it
/// runs inline: setup calls it before the run loop first turns, and WebKit
/// asks the delegate only when a web process asks, over the run loop.
pub fn install_default(app: &AppHandle) {
    let handle = app.clone();
    let queued = app.run_on_main_thread(move || {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let store = unsafe { WKWebsiteDataStore::defaultDataStore(mtm) };
        install(&handle, &store, mtm);
    });
    if let Err(e) = queued {
        elog!("[oculus] webcrypto: could not reach the main thread: {e}; WebKit reads its own keychain item");
    }
}

/// The name WebKit files this process's key under, computed as WebKit does:
/// the main bundle's identifier, or else the process name (`app` in dev,
/// whose embedded Info.plist has no identifier).
pub fn webkit_application() -> String {
    NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|id| id.to_string())
        .unwrap_or_else(|| NSProcessInfo::processInfo().processName().to_string())
}

/// The 16 bytes, or `None` when WebKit should read its own keychain item:
/// keyd absent, a refused or failed read, or any other error. Never logs the
/// key.
pub fn fetch() -> Option<[u8; 16]> {
    let app = webkit_application();
    let broker = Credentialed::at(&crate::library::paths::data_dir());
    answer(&app, broker.webcrypto_key(&app))
}

fn answer(app: &str, served: Result<ServedKey, KeydError>) -> Option<[u8; 16]> {
    match served {
        Ok(served) => {
            if served.source != "vault" {
                elog!(
                    "[oculus] webcrypto: keyd now holds {app}'s master key (source={})",
                    served.source
                );
            }
            Some(*served.key.bytes())
        }
        Err(KeydError::Absent) => {
            elog!("[oculus] webcrypto: oculus-keyd is not running; WebKit reads its own keychain item");
            None
        }
        Err(e) => {
            elog!("[oculus] webcrypto: {e}; WebKit reads its own keychain item");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use keyd_core::webcrypto::WebCryptoKey;

    use super::*;

    #[test]
    fn a_served_key_is_its_16_bytes_and_every_error_is_none() {
        let key = WebCryptoKey::from_bytes(*b"foobarfoobarfoob");
        for source in ["vault", "keychain", "minted"] {
            let served = ServedKey {
                key: key.clone(),
                source: source.into(),
            };
            assert_eq!(answer("app", Ok(served)), Some(*b"foobarfoobarfoob"));
        }
        for error in [
            KeydError::Absent,
            KeydError::Keychain("reading x: OSStatus -128".into()),
            KeydError::Caller("not that application".into()),
            KeydError::Broken("webcrypto_key: no 16-byte key in the reply".into()),
        ] {
            assert_eq!(answer("app", Err(error)), None);
        }
    }

    #[test]
    fn the_test_binary_is_named_by_its_process() {
        // A test binary has no bundle identifier, like the dev app.
        let name = webkit_application();
        assert!(!name.is_empty());
        assert_eq!(
            Some(name.as_str()),
            std::env::current_exe()
                .ok()
                .as_deref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
        );
    }
}
