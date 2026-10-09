use std::path::PathBuf;

pub fn cookie_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("canvas-session.cookie")
}

/// Okta's cookies for `sso.unimelb.edu.au`, the same bare `name=value; …`
/// header as the Canvas one. Only the in-app browser replays it.
pub fn sso_cookie_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("sso-session.cookie")
}

/// Writes a session file readable by this user only. A new file is created
/// 0600 and an existing one narrowed before the body lands, so the secret is
/// never on disk world-readable.
pub fn write_private(path: &std::path::Path, body: &str) -> std::io::Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(body.as_bytes())
}

/// Ed's `x-token`, minted from the Canvas session (`sources/ed/`).
pub fn ed_token_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("ed-session.token")
}

pub fn auth_flag_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("canvas-session").join("authenticated")
}

/// Present from a sign-out until the next session (`mark_authenticated`).
/// While it is, automatic sign-ins stand down (`okta::sign_in`).
pub fn signed_out_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("canvas-session").join("signed-out")
}

/// Drops the saved Canvas, Okta and Ed sessions and the auth flag, and marks
/// the app signed out; returns whether there was anything to drop. The attempt
/// record beside the flag stays: forgetting a lockout pause would let
/// automatic sign-in resume.
pub fn sign_out(data_dir: &std::path::Path) -> std::io::Result<bool> {
    let mut had = false;
    for path in [
        cookie_path(data_dir),
        sso_cookie_path(data_dir),
        ed_token_path(data_dir),
        auth_flag_path(data_dir),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => had = true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    let marker = signed_out_path(data_dir);
    if let Some(parent) = marker.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&marker, b"1")?;
    Ok(had)
}

/// Record that we hold a session Canvas has accepted.
///
/// The app's startup probe ignores the cookie without this flag, so every path
/// that establishes a session must write it, the CLI included.
pub fn mark_authenticated(data_dir: &std::path::Path) {
    let flag = auth_flag_path(data_dir);
    if let Some(parent) = flag.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&flag, b"1").ok();
    std::fs::remove_file(signed_out_path(data_dir)).ok();
}
