//! Removes the session keep-alive an earlier Oculus installed: its LaunchAgent
//! (unloaded and deleted through keyd's registrar, which owns every OS call)
//! and the files it left in the data dir. Runs once per start, off the main
//! thread because unloading can wait on launchd, and says nothing when there
//! is nothing to remove.

use std::path::{Path, PathBuf};

use keyd_core::platform;

const LABEL: &str = "com.tchan.oculus.session-keepalive";

/// What the agent and the app wrote in the data dir.
const FILES: [&str; 3] = [
    "session-keepalive.sh",
    "session-keepalive.log",
    "keepalive-disabled",
];

pub fn retire_in_background() {
    std::thread::spawn(|| {
        let data_dir = crate::paths::data_dir();
        match retire(&data_dir, |label| platform::registrar().retire(label)) {
            Ok(removed) if removed.is_empty() => {}
            Ok(removed) => eprintln!(
                "[oculus] removed the old session keep-alive ({} file(s))",
                removed.len()
            ),
            Err(e) => eprintln!("[oculus] could not remove the old session keep-alive: {e}"),
        }
    });
}

/// Retires the agent, then deletes its files whatever the agent's outcome;
/// returns everything removed. A failed unload is the error, and the agent's
/// plist is still there for the next start to try again.
fn retire(
    data_dir: &Path,
    retire_agent: impl FnOnce(&str) -> Result<Vec<PathBuf>, String>,
) -> Result<Vec<PathBuf>, String> {
    let agent = retire_agent(LABEL);
    let mut removed = remove_files(data_dir);
    removed.extend(agent?);
    Ok(removed)
}

fn remove_files(data_dir: &Path) -> Vec<PathBuf> {
    FILES
        .iter()
        .map(|name| data_dir.join(name))
        .filter(|path| std::fs::remove_file(path).is_ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn nothing_installed_removes_nothing_and_asks_for_the_right_label() {
        let dir = Scratch::new("legacy-agent-none");
        let asked = std::cell::RefCell::new(Vec::new());
        let removed = retire(&dir, |label| {
            asked.borrow_mut().push(label.to_string());
            Ok(Vec::new())
        })
        .unwrap();
        assert!(removed.is_empty());
        assert_eq!(*asked.borrow(), [LABEL]);
    }

    #[test]
    fn the_leftover_files_go_once_and_other_files_stay() {
        let dir = Scratch::new("legacy-agent-files");
        for name in FILES.iter().chain(&["okta-sign-in.log", "oculus.db"]) {
            std::fs::write(dir.join(name), "x").unwrap();
        }
        let plist = dir.join("agent.plist");
        let removed = retire(&dir, |_| Ok(vec![plist.clone()])).unwrap();
        assert_eq!(removed.len(), FILES.len() + 1);
        for name in FILES {
            assert!(!dir.join(name).exists(), "{name}");
        }
        assert!(dir.join("okta-sign-in.log").exists());
        assert!(dir.join("oculus.db").exists());

        assert!(retire(&dir, |_| Ok(Vec::new())).unwrap().is_empty());
    }

    #[test]
    fn a_failed_unload_is_reported_and_the_files_still_go() {
        let dir = Scratch::new("legacy-agent-stuck");
        std::fs::write(dir.join("session-keepalive.log"), "x").unwrap();
        let err = retire(&dir, |_| Err("still loaded".into())).unwrap_err();
        assert_eq!(err, "still loaded");
        assert!(!dir.join("session-keepalive.log").exists());
    }
}
