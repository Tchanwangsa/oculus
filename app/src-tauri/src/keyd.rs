//! Installs and inspects `oculus-keyd`, the credential broker (its own crate
//! in `app/keyd/`; see docs/architecture.md and docs/development.md).
//!
//! An install points the LaunchAgent `com.tchan.oculus.keyd` at a fixed
//! binary — `<data_dir>/bin/oculus-keyd` for a dev build, copied there, or a
//! bundle's own `Contents/MacOS/oculus-keyd`, which must run in place for its
//! caller check — never at `target/` or a worktree. launchd owns the socket
//! and starts keyd on the first connect. The source-hash stamp in
//! `<data_dir>/bin/` records what the agent runs, so a rebuild with unchanged
//! source never reinstalls.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::paths;

pub const LABEL: &str = "com.tchan.oculus.keyd";
pub const BINARY: &str = "oculus-keyd";
const STAMP: &str = "oculus-keyd.stamp";

pub fn installed_bin(data_dir: &Path) -> PathBuf {
    paths::keyd_bin_dir(data_dir).join(BINARY)
}

pub fn stamp_path(data_dir: &Path) -> PathBuf {
    paths::keyd_bin_dir(data_dir).join(STAMP)
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| "HOME is not set".to_string())
}

pub fn plist_path() -> Result<PathBuf, String> {
    Ok(home()?.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

pub fn log_path() -> Result<PathBuf, String> {
    Ok(home()?.join("Library/Logs/oculus-keyd.log"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// No `KeepAlive`: launchd starts keyd per connection and keyd exits idle.
/// `SockPathMode` 384 is 0600.
fn plist_body(program: &Path, socket: &Path, log: &Path) -> String {
    let [program, socket, log] = [program, socket, log].map(|p| xml_escape(&p.to_string_lossy()));
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
        <key>Listeners</key>
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
pub fn program_from_plist(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let after = text.split("<key>ProgramArguments</key>").nth(1)?;
    let rest = &after[after.find("<string>")? + "<string>".len()..];
    let raw = rest[..rest.find("</string>")?].trim();
    Some(raw.replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
}

/// What `<bin> source-hash` prints. Running it also proves the file is a keyd
/// that starts.
pub fn source_hash_of(bin: &Path) -> Result<String, String> {
    let out = std::process::Command::new(bin)
        .arg("source-hash")
        .output()
        .map_err(|e| format!("running {}: {e}", bin.display()))?;
    let hash = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{} is not an oculus-keyd (source-hash gave {:?})", bin.display(), hash));
    }
    Ok(hash)
}

pub fn installed_stamp(data_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(stamp_path(data_dir)).ok()?;
    Some(text.trim().to_string()).filter(|s| !s.is_empty())
}

/// The `.app` that `bin` sits in, if any.
fn bundle_of(bin: &Path) -> Option<PathBuf> {
    bin.ancestors()
        .find(|a| a.extension().is_some_and(|x| x == "app") && a.join("Contents").is_dir())
        .map(Path::to_path_buf)
}

/// The keyd this build of the app or CLI would install: the one beside it in
/// the bundle, else (debug builds) the signed output of `bun run keyd` in the
/// checkout it was built from.
pub fn candidate() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let sibling = exe.parent()?.join(BINARY);
    if sibling.is_file() {
        return Some(sibling);
    }
    if cfg!(debug_assertions) {
        let built = Path::new(env!("CARGO_MANIFEST_DIR")).join("../keyd/target/signed").join(BINARY);
        return built.canonicalize().ok().filter(|p| p.is_file());
    }
    None
}

#[derive(Debug, serde::Serialize)]
pub struct Installed {
    pub program: PathBuf,
    pub source_hash: String,
    pub plist: PathBuf,
}

/// Installs `from` and (re)loads the LaunchAgent. A keyd inside an app bundle
/// is registered where it is; any other is copied to `<data_dir>/bin` first.
pub fn install(data_dir: &Path, from: &Path) -> Result<Installed, String> {
    if !cfg!(target_os = "macos") {
        return Err("oculus-keyd runs under launchd, so it is macOS-only".to_string());
    }
    let from = from.canonicalize().map_err(|e| format!("{}: {e}", from.display()))?;
    let source_hash = source_hash_of(&from)?;
    let socket = paths::keyd_socket_path(data_dir);
    if socket.as_os_str().len() >= 104 {
        return Err(format!("the socket path is too long for a unix socket: {}", socket.display()));
    }

    let program = if bundle_of(&from).is_some() {
        from.clone()
    } else {
        let dest = installed_bin(data_dir);
        // A new file renamed over the old one: overwriting a signed binary in
        // place leaves the kernel's cached signature stale and the next exec
        // is killed.
        replace_file(&dest, |tmp| {
            std::fs::copy(&from, tmp)?;
            set_mode(tmp, 0o755)
        })?;
        dest
    };

    let plist = plist_path()?;
    let log = log_path()?;
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    }
    let body = plist_body(&program, &socket, &log);
    replace_file(&plist, |tmp| std::fs::write(tmp, &body))?;

    bootout()?;
    std::fs::remove_file(&socket).ok();
    launchctl(&["bootstrap", &gui_domain(), &plist.to_string_lossy()])
        .map_err(|e| format!("launchctl bootstrap refused {}: {e}", plist.display()))?;

    // Last, so a failed load is retried by the next preflight or launch.
    let stamp = stamp_path(data_dir);
    replace_file(&stamp, |tmp| std::fs::write(tmp, format!("{source_hash}\n")))?;
    Ok(Installed { program, source_hash, plist })
}

/// Unloads keyd and removes its plist, dev binary and stamp. The vault and the
/// keychain's master key stay, so a reinstall reads the same secrets.
pub fn uninstall(data_dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !cfg!(target_os = "macos") {
        return Ok(Vec::new());
    }
    bootout()?;
    let mut removed = Vec::new();
    for p in [plist_path()?, installed_bin(data_dir), stamp_path(data_dir), paths::keyd_socket_path(data_dir)] {
        match std::fs::remove_file(&p) {
            Ok(()) => removed.push(p),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("removing {}: {e}", p.display())),
        }
    }
    Ok(removed)
}

/// Startup check. Dev builds do nothing: the preflight installs from the main
/// checkout only, and an app built in a worktree must not take keyd over.
/// A release reinstalls its bundled keyd when the stamp or the plist's
/// program differs from it.
pub fn ensure_installed() {
    if cfg!(debug_assertions) || !cfg!(target_os = "macos") {
        return;
    }
    std::thread::spawn(|| match ensure_bundled(&paths::data_dir()) {
        Ok(Some(i)) => eprintln!("[oculus] keyd installed from {} ({})", i.program.display(), &i.source_hash[..12]),
        Ok(None) => {}
        Err(e) => eprintln!("[oculus] could not install keyd: {e}"),
    });
}

fn ensure_bundled(data_dir: &Path) -> Result<Option<Installed>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundled = exe.with_file_name(BINARY);
    if !bundled.is_file() {
        return Ok(None);
    }
    let hash = source_hash_of(&bundled)?;
    let program = program_from_plist(&plist_path()?);
    let current = installed_stamp(data_dir).as_deref() == Some(hash.as_str())
        && program.as_deref() == Some(bundled.to_string_lossy().as_ref());
    if current {
        return Ok(None);
    }
    install(data_dir, &bundled).map(Some)
}

// ── Status ───────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct Status {
    pub plist: PathBuf,
    /// The binary the LaunchAgent runs, when a plist is installed.
    pub program: Option<String>,
    pub loaded: bool,
    pub installed_hash: Option<String>,
    /// What this build would install, and its source hash or why it has none.
    pub candidate: Option<PathBuf>,
    pub candidate_hash: Option<String>,
    pub candidate_error: Option<String>,
    pub socket: PathBuf,
    /// keyd's `ping` reply: version, source hash, pid.
    pub ping: Option<serde_json::Value>,
    pub ping_error: Option<String>,
    pub vault: PathBuf,
    pub vault_bytes: Option<u64>,
}

/// Never reads a secret: `ping` is the one op it sends, and keyd answers it
/// without opening the vault or the keychain.
pub fn status(data_dir: &Path) -> Result<Status, String> {
    let plist = plist_path()?;
    let program = program_from_plist(&plist);
    let candidate = candidate();
    let (candidate_hash, candidate_error) = match &candidate {
        Some(c) => match source_hash_of(c) {
            Ok(h) => (Some(h), None),
            Err(e) => (None, Some(e)),
        },
        None => (None, None),
    };
    let socket = paths::keyd_socket_path(data_dir);
    let (ping, ping_error) = match ping(&socket) {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    let vault = paths::vault_path(data_dir);
    Ok(Status {
        loaded: cfg!(target_os = "macos") && is_loaded(),
        plist,
        program,
        installed_hash: installed_stamp(data_dir),
        candidate,
        candidate_hash,
        candidate_error,
        vault_bytes: std::fs::metadata(&vault).ok().map(|m| m.len()),
        vault,
        socket,
        ping,
        ping_error,
    })
}

/// One `ping` over the socket. Connecting starts keyd if launchd has the
/// agent loaded.
pub fn ping(socket: &Path) -> Result<serde_json::Value, String> {
    let stream = std::os::unix::net::UnixStream::connect(socket).map_err(|e| format!("{}: {e}", socket.display()))?;
    // A probe, not a parse: ping does no work, so a reply this late means
    // keyd is wedged. launchd's cold start is about a second and a half.
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10))).ok();
    (&stream).write_all(b"{\"op\":\"ping\"}\n").map_err(|e| format!("write: {e}"))?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).map_err(|e| format!("read: {e}"))?;
    let reply: serde_json::Value = serde_json::from_str(&line).map_err(|_| format!("not a keyd reply: {line:?}"))?;
    match reply.get("error").and_then(|e| e.as_str()) {
        Some(kind) => Err(format!("keyd refused ping ({kind}): {}", reply["detail"].as_str().unwrap_or(""))),
        None => Ok(reply),
    }
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

// ── Files ────────────────────────────────────────────────────────────────────

/// Writes through `fill` into a sibling temp file, then renames it over `dest`.
fn replace_file(dest: &Path, fill: impl FnOnce(&Path) -> std::io::Result<()>) -> Result<(), String> {
    let dir = dest.parent().ok_or_else(|| format!("{} has no parent", dest.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let name = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let done = fill(&tmp).and_then(|()| std::fs::rename(&tmp, dest));
    if let Err(e) = done {
        std::fs::remove_file(&tmp).ok();
        return Err(format!("writing {}: {e}", dest.display()));
    }
    Ok(())
}

fn set_mode(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plist_escapes_paths_and_round_trips_the_program() {
        let dir = crate::test_support::Scratch::new("keyd-plist");
        let program = Path::new("/Users/a&b/Library/Application Support/com.tchan.oculus/bin/oculus-keyd");
        let body = plist_body(program, Path::new("/x/<keyd>.sock"), Path::new("/x/\"log\""));
        assert!(body.contains("/Users/a&amp;b/Library/Application Support/"));
        assert!(body.contains("/x/&lt;keyd&gt;.sock"));
        assert!(body.contains("/x/&quot;log&quot;"));
        assert!(!body.contains("a&b"));
        let p = dir.join("k.plist");
        std::fs::write(&p, &body).unwrap();
        assert_eq!(program_from_plist(&p).as_deref(), Some(program.to_str().unwrap()));
    }

    /// launchd execs ProgramArguments directly, owns the 0600 socket, and
    /// must not keep keyd resident.
    #[test]
    fn the_agent_is_socket_activated_and_not_kept_alive() {
        let body = plist_body(Path::new("/d/bin/oculus-keyd"), Path::new("/d/keyd.sock"), Path::new("/l.log"));
        assert!(body.contains("<string>/d/bin/oculus-keyd</string>\n        <string>serve</string>"));
        assert!(body.contains("<key>SockPathName</key>\n            <string>/d/keyd.sock</string>"));
        assert!(body.contains("<integer>384</integer>"));
        assert!(body.contains(&format!("<string>{LABEL}</string>")));
        assert!(!body.contains("KeepAlive"));
        assert!(!body.contains("RunAtLoad"));
    }

    #[test]
    fn a_bundled_keyd_is_recognised_by_its_app() {
        let dir = crate::test_support::Scratch::new("keyd-bundle");
        let macos = dir.join("Oculus.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        assert_eq!(bundle_of(&macos.join(BINARY)), Some(dir.join("Oculus.app")));
        assert_eq!(bundle_of(&dir.join("bin").join(BINARY)), None);
    }

    #[test]
    fn a_file_that_is_not_keyd_is_refused_before_anything_is_written() {
        let dir = crate::test_support::Scratch::new("keyd-notkeyd");
        let err = source_hash_of(Path::new("/bin/echo")).unwrap_err();
        assert!(err.contains("not an oculus-keyd"), "{err}");
        assert!(install(&dir.join("data"), Path::new("/bin/echo")).is_err());
        assert!(!dir.join("data").exists());
    }

    /// A stand-in keyd that answers one request with `reply`.
    fn fake_keyd(dir: &Path, reply: &'static str) -> PathBuf {
        let sock = dir.join("k.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            assert_eq!(line, "{\"op\":\"ping\"}\n");
            (&stream).write_all(reply.as_bytes()).unwrap();
        });
        sock
    }

    #[test]
    fn ping_returns_keyd_reply_or_its_error() {
        let dir = crate::test_support::Scratch::new("keyd-ping");
        let sock = fake_keyd(&dir, "{\"version\":\"0.1.0\",\"source_hash\":\"ab\",\"pid\":7}\n");
        assert_eq!(ping(&sock).unwrap()["pid"], 7);

        let dir = crate::test_support::Scratch::new("keyd-ping-refused");
        let sock = fake_keyd(&dir, "{\"error\":\"caller\",\"detail\":\"outside the bundle\"}\n");
        let err = ping(&sock).unwrap_err();
        assert!(err.contains("caller") && err.contains("outside the bundle"), "{err}");

        assert!(ping(&dir.join("absent.sock")).is_err());
    }

    #[test]
    fn replace_file_leaves_no_temp_behind() {
        let dir = crate::test_support::Scratch::new("keyd-replace");
        let dest = dir.join("sub/stamp");
        replace_file(&dest, |tmp| std::fs::write(tmp, "a\n")).unwrap();
        replace_file(&dest, |tmp| std::fs::write(tmp, "b\n")).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "b\n");
        assert!(replace_file(&dest, |_| Err(std::io::Error::other("no"))).is_err());
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "b\n");
        assert_eq!(std::fs::read_dir(dir.join("sub")).unwrap().count(), 1);
    }
}
