//! The LaunchAgent `com.tchan.oculus.keyd`: launchd owns keyd's socket and
//! starts keyd on the first connect. The plist names a fixed program, never
//! a build tree, and has no `KeepAlive`: keyd exits when idle.

use std::path::{Path, PathBuf};

use crate::paths;
use crate::platform::files::replace_file;
use crate::platform::{Registrar, Registration};

const LABEL: &str = "com.tchan.oculus.keyd";

/// `sun_path` holds 104 bytes, NUL included.
const SUN_PATH: usize = 104;

pub(crate) struct Launchd;

impl Registrar for Launchd {
    fn check(&self, data_dir: &Path) -> Result<(), String> {
        let socket = paths::socket(data_dir);
        if socket.as_os_str().len() >= SUN_PATH {
            return Err(format!("the socket path is too long for a unix socket: {}", socket.display()));
        }
        Ok(())
    }

    /// A bundled keyd must run in place: its caller check admits only its own
    /// bundle.
    fn runs_in_place(&self, program: &Path) -> bool {
        bundle_of(program).is_some()
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
        Ok(Registration { program: program_from_plist(&plist), loaded: is_loaded(), path: plist })
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
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| "HOME is not set".to_string())
}

fn plist_path() -> Result<PathBuf, String> {
    Ok(home()?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

fn log_path() -> Result<PathBuf, String> {
    Ok(home()?.join("Library/Logs/oculus-keyd.log"))
}

/// The `.app` that `bin` sits in, if any.
fn bundle_of(bin: &Path) -> Option<PathBuf> {
    bin.ancestors()
        .find(|a| a.extension().is_some_and(|x| x == "app") && a.join("Contents").is_dir())
        .map(Path::to_path_buf)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// No `KeepAlive`: launchd starts keyd per connection and keyd exits idle.
/// `SockPathMode` 384 is 0600.
fn plist_body(program: &Path, socket: &Path, log: &Path) -> String {
    let [program, socket, log] = [program, socket, log].map(|p| xml_escape(&p.to_string_lossy()));
    let sockets_key = super::SOCKETS_KEY;
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
    Some(raw.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
}

// ── launchctl ────────────────────────────────────────────────────────────────

fn gui_domain() -> String {
    format!("gui/{}", unsafe { libc::getuid() })
}

fn service() -> String {
    format!("{}/{LABEL}", gui_domain())
}

fn launchctl(args: &[&str]) -> Result<std::process::Output, String> {
    let out = std::process::Command::new("/bin/launchctl")
        .args(args)
        .output()
        .map_err(|e| format!("launchctl: {e}"))?;
    if out.status.success() {
        return Ok(out);
    }
    let mut msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if msg.is_empty() {
        msg = format!("exit {}", out.status);
    }
    Err(msg)
}

fn is_loaded() -> bool {
    launchctl(&["print", &service()]).is_ok()
}

/// `bootout` returns before launchd has let go of the job, and a `bootstrap`
/// that comes too soon fails and leaves it unloaded. So wait for `print` to
/// stop finding the label.
fn bootout() -> Result<(), String> {
    if !is_loaded() {
        return Ok(());
    }
    // An error here is usually "not loaded" racing the check above; the wait decides.
    let _ = launchctl(&["bootout", &service()]);
    let started = std::time::Instant::now();
    while is_loaded() {
        if started.elapsed() > std::time::Duration::from_secs(30) {
            return Err(format!("launchd still has {LABEL} loaded 30 s after bootout"));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Ok(())
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
        let program = Path::new("/Users/a&b/Library/Application Support/com.tchan.oculus/bin/oculus-keyd");
        let body = plist_body(program, Path::new("/x/<keyd>.sock"), Path::new("/x/\"log\""));
        assert!(body.contains("/Users/a&amp;b/Library/Application Support/"));
        assert!(body.contains("/x/&lt;keyd&gt;.sock"));
        assert!(body.contains("/x/&quot;log&quot;"));
        assert!(!body.contains("a&b"));
        let p = dir.join("k.plist");
        std::fs::write(&p, &body).unwrap();
        assert_eq!(program_from_plist(&p).as_deref(), Some(program.to_str().unwrap()));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// launchd execs ProgramArguments directly, owns the 0600 socket, and
    /// must not keep keyd resident.
    #[test]
    fn the_agent_is_socket_activated_and_not_kept_alive() {
        let body = plist_body(Path::new("/d/bin/oculus-keyd"), Path::new("/d/keyd.sock"), Path::new("/l.log"));
        assert!(body.contains("<string>/d/bin/oculus-keyd</string>\n        <string>serve</string>"));
        assert!(body.contains("<key>Listeners</key>\n        <dict>\n            <key>SockPathName</key>\n            <string>/d/keyd.sock</string>"));
        assert!(body.contains("<integer>384</integer>"));
        assert!(body.contains(&format!("<string>{LABEL}</string>")));
        assert!(!body.contains("KeepAlive"));
        assert!(!body.contains("RunAtLoad"));
    }

    #[test]
    fn a_bundled_keyd_is_recognised_by_its_app() {
        let dir = scratch("bundle");
        let macos = dir.join("Oculus.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        assert_eq!(bundle_of(&macos.join(paths::BINARY)), Some(dir.join("Oculus.app")));
        assert!(Launchd.runs_in_place(&macos.join(paths::BINARY)));
        assert_eq!(bundle_of(&dir.join("bin").join(paths::BINARY)), None);
        assert!(!Launchd.runs_in_place(&dir.join("bin").join(paths::BINARY)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_data_dir_too_deep_for_a_socket_is_refused() {
        assert!(Launchd.check(Path::new("/Users/x/Library/Application Support/com.tchan.oculus")).is_ok());
        let deep = PathBuf::from("/").join("d".repeat(SUN_PATH));
        assert!(Launchd.check(&deep).unwrap_err().contains("too long"));
    }
}
