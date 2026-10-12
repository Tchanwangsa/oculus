//! Start over: clears the library so the app can be pointed at another
//! university. The running app only signs out and leaves a marker, then
//! restarts; the next launch deletes the files before any thread or database
//! handle exists, so nothing is still writing into what is being deleted.

use std::path::Path;

/// Kept: each cloud's quota and spend guard belong to the student's API
/// accounts, not to the library; and oculus-keyd's vault (the cloud keys),
/// socket and installed helper are its install. Sign-out has already dropped
/// the sessions and the Okta credentials from the vault.
fn kept(data_dir: &Path) -> Vec<std::path::PathBuf> {
    use keyd_core::paths;

    let vault = paths::vault(data_dir);
    vec![
        data_dir.join("mineru-usage.json"),
        data_dir.join("voyage-usage.json"),
        paths::vault_lock(&vault),
        vault,
        paths::socket(data_dir),
        paths::bin_dir(data_dir),
    ]
}

fn start_over_marker(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("start-over")
}

/// Empty the data dir if Start over asked for it, keeping what [`kept`]
/// names. The first thing the app and the CLI do. The marker goes last, so an
/// interrupted run finishes on the next launch.
pub fn finish_start_over(data_dir: &Path) {
    let marker = start_over_marker(data_dir);
    if !marker.exists() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(data_dir) else {
        return;
    };
    let kept = kept(data_dir);
    let mut failed = false;
    for entry in entries.flatten() {
        let path = entry.path();
        if path == marker || kept.contains(&path) {
            continue;
        }
        let removed = match entry.file_type() {
            Ok(t) if t.is_dir() => std::fs::remove_dir_all(&path),
            _ => std::fs::remove_file(&path),
        };
        if let Err(e) = removed {
            elog!(
                "[oculus] start over: could not remove {}: {e}",
                path.display()
            );
            failed = true;
        }
    }
    if !failed {
        std::fs::remove_file(&marker).ok();
        elog!("[oculus] start over: library cleared");
    }
}

/// Sign out everywhere and forget the university's credentials, then restart
/// into [`finish_start_over`]. The marker is written last: a sign-out that
/// fails leaves the library as it was, not queued for deletion. The frontend
/// clears its own library-scoped storage first.
#[tauri::command]
pub async fn storage_start_over(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;

    if let Some(cancel) = app.try_state::<crate::sync::scrape::ScrapeCancel>() {
        cancel.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    crate::harness::app::shutdown(&app);
    crate::auth::commands::end_session(&app).await?;
    crate::auth::okta::clear_credentials().ok();
    let dir = crate::library::paths::data_dir();
    std::fs::write(start_over_marker(&dir), b"1").map_err(|e| e.to_string())?;
    app.restart()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn start_over_empties_the_library_but_keeps_cloud_usage_and_keyd() {
        let dir = Scratch::new("start-over");
        std::fs::create_dir_all(dir.join("courses/COMP30026/files")).unwrap();
        std::fs::write(dir.join("courses/COMP30026/files/a.pdf"), b"%PDF").unwrap();
        std::fs::write(dir.join("oculus.db"), b"").unwrap();
        std::fs::write(dir.join("institution.json"), b"{}").unwrap();
        std::fs::write(dir.join("voyage-usage.json"), b"{}").unwrap();
        std::fs::write(dir.join("mineru-usage.json"), b"{}").unwrap();
        std::fs::write(keyd_core::paths::vault(&dir), b"sealed").unwrap();
        std::fs::create_dir_all(keyd_core::paths::bin_dir(&dir)).unwrap();
        std::fs::create_dir_all(keyd_core::paths::session_dir(&dir)).unwrap();

        finish_start_over(&dir);
        assert!(
            dir.join("oculus.db").exists(),
            "nothing happens without the marker"
        );

        std::fs::write(start_over_marker(&dir), b"1").unwrap();
        finish_start_over(&dir);
        let mut left: Vec<_> = std::fs::read_dir(&*dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            ["bin", "mineru-usage.json", "vault.bin", "voyage-usage.json"]
        );
    }
}
