//! The LaunchAgent `com.tchan.oculus.keyd`: launchd owns keyd's socket and
//! starts keyd on the first connect. The plist names a fixed program, never
//! a build tree, and has no `KeepAlive`: keyd exits when idle. It names the
//! app as its owner (`AssociatedBundleIdentifiers`), so Login Items and the
//! background-activity notice show it under Oculus.

use std::path::{Path, PathBuf};

use crate::paths;
use crate::platform::files::replace_file;
use crate::platform::{Registrar, Registration};

mod launchd;
use launchd::{bootout, gui_domain, is_loaded, launchctl};

const LABEL: &str = "com.tchan.oculus.keyd";

/// `sun_path` holds 104 bytes, NUL included.
const SUN_PATH: usize = 104;

pub(crate) struct Launchd;

impl Registrar for Launchd {
    fn check(&self, data_dir: &Path) -> Result<(), String> {
        let socket = paths::socket(data_dir);
        if socket.as_os_str().len() >= SUN_PATH {
            return Err(format!(
                "the socket path is too long for a unix socket: {}",
                socket.display()
            ));
        }
        Ok(())
    }

    /// A keyd whose helper app is nested in another app must run in place:
    /// its caller check admits only that app's executables. A helper on its
    /// own (a dev build's) is copied.
    fn runs_in_place(&self, program: &Path) -> bool {
        paths::helper_of(program).is_some_and(|helper| {
            super::outermost_app(&helper).is_some_and(|outer| outer != helper)
        })
    }

    fn install(&self, program: &Path, data_dir: &Path) -> Result<PathBuf, String> {
        self.check(data_dir)?;
        let socket = paths::socket(data_dir);
        let plist = plist_path()?;
        let log = log_path()?;
        if let Some(dir) = log.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        let body = plist_body(program, &socket, &log);
        replace_file(&plist, |tmp| std::fs::write(tmp, &body))?;

        bootout()?;
        std::fs::remove_file(&socket).ok();
        launchctl(&["bootstrap", &gui_domain(), &plist.to_string_lossy()])
            .map_err(|e| format!("launchctl bootstrap refused {}: {e}", plist.display()))?;
        Ok(plist)
    }

    fn status(&self) -> Result<Registration, String> {
        let plist = plist_path()?;
        Ok(Registration {
            program: program_from_plist(&plist),
            loaded: is_loaded(LABEL),
            path: plist,
        })
    }

    fn uninstall(&self) -> Result<Vec<PathBuf>, String> {
        bootout()?;
        let plist = plist_path()?;
        match std::fs::remove_file(&plist) {
            Ok(()) => Ok(vec![plist]),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(format!("removing {}: {e}", plist.display())),
        }
    }
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn agents_dir() -> Result<PathBuf, String> {
    Ok(home()?.join("Library/LaunchAgents"))
}

fn plist_path() -> Result<PathBuf, String> {
    Ok(agents_dir()?.join(format!("{LABEL}.plist")))
}

fn log_path() -> Result<PathBuf, String> {
    Ok(home()?.join("Library/Logs/oculus-keyd.log"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// No `KeepAlive`: launchd starts keyd per connection and keyd exits idle.
/// `SockPathMode` 384 is 0600.
fn plist_body(program: &Path, socket: &Path, log: &Path) -> String {
    let [program, socket, log] = [program, socket, log].map(|p| xml_escape(&p.to_string_lossy()));
    let sockets_key = super::SOCKETS_KEY;
    let owner = paths::IDENTIFIER;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{program}</string>
        <string>serve</string>
    </array>
    <key>Sockets</key>
    <dict>
        <key>{sockets_key}</key>
        <dict>
            <key>SockPathName</key>
            <string>{socket}</string>
            <key>SockPathMode</key>
            <integer>384</integer>
        </dict>
    </dict>
    <key>StandardErrorPath</key>
    <string>{log}</string>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>AssociatedBundleIdentifiers</key>
    <array>
        <string>{owner}</string>
    </array>
</dict>
</plist>
"#
    )
}

/// The first `<string>` inside `ProgramArguments`, unescaped. Scanned, not parsed.
fn program_from_plist(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let after = text.split("<key>ProgramArguments</key>").nth(1)?;
    let rest = &after[after.find("<string>")? + "<string>".len()..];
    let raw = rest[..rest.find("</string>")?].trim();
    Some(
        raw.replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("keyd-launchd-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_plist_escapes_paths_and_round_trips_the_program() {
        let dir = scratch("plist");
        let program = Path::new(
            "/Users/a&b/Library/Application Support/com.tchan.oculus/bin/Oculus Helper.app/Contents/MacOS/Oculus Helper",
        );
        let body = plist_body(
            program,
            Path::new("/x/<keyd>.sock"),
            Path::new("/x/\"log\""),
        );
        assert!(body.contains("/Users/a&amp;b/Library/Application Support/"));
        assert!(body.contains("/x/&lt;keyd&gt;.sock"));
        assert!(body.contains("/x/&quot;log&quot;"));
        assert!(!body.contains("a&b"));
        let p = dir.join("k.plist");
        std::fs::write(&p, &body).unwrap();
        assert_eq!(
            program_from_plist(&p).as_deref(),
            Some(program.to_str().unwrap())
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// launchd execs ProgramArguments directly, owns the 0600 socket, and
    /// must not keep keyd resident.
    #[test]
    fn the_agent_is_socket_activated_and_not_kept_alive() {
        let body = plist_body(
            Path::new("/d/bin/oculus-keyd"),
            Path::new("/d/keyd.sock"),
            Path::new("/l.log"),
        );
        assert!(
            body.contains("<string>/d/bin/oculus-keyd</string>\n        <string>serve</string>")
        );
        assert!(body.contains("<key>Listeners</key>\n        <dict>\n            <key>SockPathName</key>\n            <string>/d/keyd.sock</string>"));
        assert!(body.contains("<integer>384</integer>"));
        assert!(body.contains(&format!("<string>{LABEL}</string>")));
        assert!(!body.contains("KeepAlive"));
        assert!(!body.contains("RunAtLoad"));
    }

    /// Login Items and the background-activity notice attribute the agent
    /// to the app whose bundle identifier it names.
    #[test]
    fn the_agent_names_the_app_as_its_owner() {
        let body = plist_body(
            Path::new("/d/bin/oculus-keyd"),
            Path::new("/d/keyd.sock"),
            Path::new("/l.log"),
        );
        assert!(body.contains(&format!(
            "<key>AssociatedBundleIdentifiers</key>\n    <array>\n        <string>{}</string>\n    </array>",
            paths::IDENTIFIER
        )));
    }

    #[test]
    fn only_a_helper_nested_in_an_app_runs_in_place() {
        let dir = scratch("bundle");
        let nested = dir.join("Oculus.app/Contents/Helpers/Oculus Helper.app");
        std::fs::create_dir_all(nested.join("Contents/MacOS")).unwrap();
        assert!(Launchd.runs_in_place(&paths::helper_program(&nested)));

        // The dev install and the build tree hold the helper on its own.
        let alone = paths::installed_helper(&dir);
        std::fs::create_dir_all(alone.join("Contents/MacOS")).unwrap();
        assert!(!Launchd.runs_in_place(&paths::helper_program(&alone)));
        assert!(!Launchd.runs_in_place(&dir.join("bin").join(paths::BINARY)));

        // Not keyd's own executable, even inside the app.
        let macos = dir.join("Oculus.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        assert!(!Launchd.runs_in_place(&macos.join(paths::BINARY)));
        assert!(!Launchd.runs_in_place(&nested.join("Contents/MacOS/other")));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_data_dir_too_deep_for_a_socket_is_refused() {
        assert!(Launchd
            .check(Path::new(
                "/Users/x/Library/Application Support/com.tchan.oculus"
            ))
            .is_ok());
        let deep = PathBuf::from("/").join("d".repeat(SUN_PATH));
        assert!(Launchd.check(&deep).unwrap_err().contains("too long"));
    }
}
