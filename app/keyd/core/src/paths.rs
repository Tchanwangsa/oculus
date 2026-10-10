//! Where keyd's files live. The data dir is defined here once, and both keyd
//! and the app's `paths::data_dir` use it. What sits in it is named here too;
//! where the OS keeps its own registration is the registrar's business.

use std::path::{Path, PathBuf};

/// The app's bundle identifier (`identifier` in tauri.conf.json; the app's
/// tests hold the two together).
pub const IDENTIFIER: &str = "com.tchan.oculus";

/// Canvas's origin, without a trailing slash: where the sign-in lands and
/// where the app's requests go.
pub const CANVAS_BASE: &str = "https://canvas.lms.unimelb.edu.au";

/// The name cargo builds keyd under, and of the bare binary an earlier
/// install left in `bin/`.
pub const BINARY: &str = "oculus-keyd";

/// The helper app keyd runs as, and its executable's name inside it: the
/// name macOS shows for keyd in a keychain prompt, the background-activity
/// notice and Login Items, which some read from the executable rather than the
/// bundle. Its bundle identifier is `<IDENTIFIER>.keyd`.
pub const HELPER: &str = "Oculus Helper";

const STAMP: &str = "oculus-keyd.stamp";

/// The OS's per-user data dir plus the identifier: Tauri's `app_data_dir()`.
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(IDENTIFIER)
}

/// keyd's endpoint. Short on purpose: a macOS `sun_path` holds 104 bytes.
pub fn socket(data_dir: &Path) -> PathBuf {
    data_dir.join("keyd.sock")
}

/// Every secret, sealed under the master key only keyd reads.
pub fn vault(data_dir: &Path) -> PathBuf {
    data_dir.join("vault.bin")
}

/// The lock the vault is read and replaced under, beside it.
pub fn vault_lock(vault_file: &Path) -> PathBuf {
    let mut name = vault_file.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}

/// Where the Canvas session's files sit: the sign-in's attempt record and
/// its signed-out marker, and the app's authenticated flag.
pub fn session_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("canvas-session")
}

/// The Canvas session cookie as an earlier version kept it, a bare
/// `name=value; …` header. keyd imports it once (`ops/legacy.rs`).
pub fn cookie(data_dir: &Path) -> PathBuf {
    data_dir.join("canvas-session.cookie")
}

/// Okta's cookies for the SSO host as an earlier version kept them, the same
/// bare header as the Canvas one.
pub fn sso_cookie(data_dir: &Path) -> PathBuf {
    data_dir.join("sso-session.cookie")
}

/// Ed's `x-token`, as an earlier version kept it. keyd imports it once.
pub fn ed_token(data_dir: &Path) -> PathBuf {
    data_dir.join("ed-session.token")
}

/// Present while a session Canvas has accepted is believed held. The app's
/// startup probe ignores the stored session without it.
pub fn authenticated(data_dir: &Path) -> PathBuf {
    session_dir(data_dir).join("authenticated")
}

/// Present from a sign-out until the next session. While it is, automatic
/// sign-ins stand down.
pub fn signed_out(data_dir: &Path) -> PathBuf {
    session_dir(data_dir).join("signed-out")
}

/// The attempt record every process checks before an automatic sign-in.
pub fn sign_in_record(data_dir: &Path) -> PathBuf {
    session_dir(data_dir).join("sign-in.json")
}

/// The lock `sign_in_record` is read and replaced under. A file of its own,
/// because a save replaces the record's inode and a lock on it would go with it.
pub fn sign_in_lock(data_dir: &Path) -> PathBuf {
    session_dir(data_dir).join("sign-in.json.lock")
}

/// Every headless Okta sign-in attempt, whoever made it.
pub fn sign_in_log(data_dir: &Path) -> PathBuf {
    data_dir.join("okta-sign-in.log")
}

/// Writes a session file readable by this user only.
pub fn write_private(path: &Path, body: &str) -> std::io::Result<()> {
    crate::platform::files::write_private(path, body.as_bytes())
}

/// Appends one sign-in attempt, stamped `now`, to the log.
pub fn append_sign_in_log(data_dir: &Path, message: &str, now: u64) {
    append_bounded_log(&sign_in_log(data_dir), message, now);
}

/// Appends one line stamped `now` (UTC), rewriting the file to its last 200
/// lines once it passes 64 KiB.
pub fn append_bounded_log(path: &Path, message: &str, now: u64) {
    use std::io::Write;

    let line = format!("{}Z {message}\n", crate::clock::iso8601_utc(now));

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        f.write_all(line.as_bytes()).ok();
    }

    // Only rewrite once the file has grown past the cap.
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > 64 * 1024 {
            if let Ok(text) = std::fs::read_to_string(path) {
                let lines: Vec<&str> = text.lines().collect();
                let keep = lines[lines.len().saturating_sub(200)..].join("\n");
                std::fs::write(path, format!("{keep}\n")).ok();
            }
        }
    }
}

/// `<HELPER>.app`, the directory name of keyd's helper app.
pub fn helper_app_name() -> String {
    format!("{HELPER}.app")
}

/// keyd's executable inside the helper app `helper`.
pub fn helper_program(helper: &Path) -> PathBuf {
    helper.join("Contents/MacOS").join(HELPER)
}

/// The helper app `program` is the executable of, when it sits at
/// `<x>.app/Contents/MacOS/<HELPER>`.
pub fn helper_of(program: &Path) -> Option<PathBuf> {
    let macos = program.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let shaped = program.file_name()? == HELPER
        && macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app";
    shaped.then(|| app.to_path_buf())
}

/// The keyd an app bundle ships for `exe`, one of the bundle's
/// `Contents/MacOS` executables: its `Contents/Helpers/<HELPER>.app`.
pub fn bundled_program(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let shaped = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app";
    shaped.then(|| helper_program(&contents.join("Helpers").join(helper_app_name())))
}

/// Where an install that does not run in place puts keyd's helper app, and
/// its stamp.
pub fn bin_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("bin")
}

pub fn installed_helper(data_dir: &Path) -> PathBuf {
    bin_dir(data_dir).join(helper_app_name())
}

pub fn installed_bin(data_dir: &Path) -> PathBuf {
    helper_program(&installed_helper(data_dir))
}

/// Where an install put keyd before it ran as a helper app: a bare binary,
/// removed by the next install.
pub fn bare_installed_bin(data_dir: &Path) -> PathBuf {
    bin_dir(data_dir).join(BINARY)
}

/// The source hash of the keyd the agent runs, written after a good install.
pub fn stamp(data_dir: &Path) -> PathBuf {
    bin_dir(data_dir).join(STAMP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_sits_in_the_data_dir() {
        let d = Path::new("/d");
        assert_eq!(socket(d), Path::new("/d/keyd.sock"));
        assert_eq!(vault(d), Path::new("/d/vault.bin"));
        assert_eq!(vault_lock(&vault(d)), Path::new("/d/vault.bin.lock"));
        assert_eq!(
            installed_bin(d),
            Path::new("/d/bin/Oculus Helper.app/Contents/MacOS/Oculus Helper")
        );
        assert_eq!(bare_installed_bin(d), Path::new("/d/bin/oculus-keyd"));
        assert_eq!(stamp(d), Path::new("/d/bin/oculus-keyd.stamp"));
        assert!(data_dir().ends_with(IDENTIFIER));
    }

    #[test]
    fn keyd_is_the_executable_of_its_helper_app() {
        let helper = Path::new("/d/bin/Oculus Helper.app");
        assert_eq!(helper_of(&helper_program(helper)).as_deref(), Some(helper));
        assert_eq!(helper_of(Path::new("/d/bin/oculus-keyd")), None);
        assert_eq!(
            helper_of(Path::new("/d/X.app/Contents/MacOS/other")),
            None,
            "another executable is not keyd"
        );
        assert_eq!(
            helper_of(Path::new("/d/X/Contents/MacOS/Oculus Helper")),
            None
        );
    }

    #[test]
    fn a_bundle_ships_keyd_as_a_nested_helper() {
        assert_eq!(
            bundled_program(Path::new("/A/Oculus.app/Contents/MacOS/app")).unwrap(),
            Path::new(
                "/A/Oculus.app/Contents/Helpers/Oculus Helper.app/Contents/MacOS/Oculus Helper"
            )
        );
        assert_eq!(bundled_program(Path::new("/x/target/release/oculus")), None);
        assert_eq!(bundled_program(Path::new("/x/Contents/MacOS/oculus")), None);
    }
}
