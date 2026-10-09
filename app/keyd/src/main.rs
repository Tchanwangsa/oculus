//! `oculus-keyd`: the one process that reads the keychain. It holds the
//! master key for `vault.bin` and answers the app and the CLI over
//! `<data_dir>/keyd.sock`, a socket launchd owns and hands over on the first
//! connect. It exits after a minute idle. See docs/architecture.md.
//!
//!   oculus-keyd serve          run by launchd (the LaunchAgent the app or
//!                              `oculus keyd install` writes)
//!   oculus-keyd source-hash    print the source hash this binary was built from
//!   oculus-keyd --version
//!
//! Debug builds add `serve-local <sock>`, which binds the socket itself, and
//! read OCULUS_KEYD_DATA_DIR, OCULUS_KEYD_TEST_KEY (64 hex characters, in
//! place of the keychain; old keychain items are then never read either),
//! OCULUS_KEYD_IDLE_SECS and OCULUS_KEYD_VOYAGE_ORIGIN (`http://127.0.0.1:<port>`,
//! a fake Voyage). Release builds have none.

mod caller;
mod forward;
mod ops;
mod server;
#[cfg(test)]
mod test_support;

use std::ffi::{c_char, c_int, CString};
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use vault::KeySource;

const IDLE: Duration = Duration::from_secs(60);

// launch.h, in libSystem. Returns 0 or an errno; the caller frees the array.
extern "C" {
    fn launch_activate_socket(name: *const c_char, fds: *mut *mut c_int, cnt: *mut libc::size_t) -> c_int;
}

pub fn log(msg: &str) {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    eprintln!("[{t:.3}] pid={} {msg}", std::process::id());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("serve") => serve(launchd_listeners()),
        Some("source-hash") => println!("{}", ops::SOURCE_HASH),
        Some("--version") => println!("oculus-keyd {} ({})", ops::VERSION, &ops::SOURCE_HASH[..12]),
        #[cfg(debug_assertions)]
        Some("serve-local") => match args.get(2) {
            Some(sock) => serve(vec![bind_local(&PathBuf::from(sock))]),
            None => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("usage: oculus-keyd serve | source-hash | --version");
    exit(64)
}

fn serve(listeners: Vec<RawFd>) {
    let policy = caller::Policy::compiled();
    log(&format!("started, source {}, {policy:?} policy, listeners {listeners:?}", &ops::SOURCE_HASH[..12]));
    let (keys, legacy) = key_sources();
    let state = ops::State::new(data_dir(), keys, legacy);
    #[cfg(debug_assertions)]
    let state = match std::env::var("OCULUS_KEYD_VOYAGE_ORIGIN") {
        Ok(origin) => match forward::Routes::compiled().with_origin(vault::names::VOYAGE, &origin) {
            Ok(routes) => state.with_routes(routes),
            Err(e) => {
                log(&format!("OCULUS_KEYD_VOYAGE_ORIGIN: {e}"));
                exit(64);
            }
        },
        Err(_) => state,
    };
    let server = Arc::new(server::Server::new(state, policy, idle()));
    server.run(&listeners);
}

/// The listener launchd made from the plist's `Sockets.Listeners`.
fn launchd_listeners() -> Vec<RawFd> {
    let name = CString::new("Listeners").unwrap();
    let mut fds: *mut c_int = std::ptr::null_mut();
    let mut cnt: libc::size_t = 0;
    let rc = unsafe { launch_activate_socket(name.as_ptr(), &mut fds, &mut cnt) };
    if rc != 0 {
        log(&format!("launch_activate_socket failed: {}", std::io::Error::from_raw_os_error(rc)));
        exit(1);
    }
    if cnt == 0 || fds.is_null() {
        log("launch_activate_socket returned no sockets");
        exit(1);
    }
    let out = unsafe { std::slice::from_raw_parts(fds, cnt) }.to_vec();
    unsafe { libc::free(fds as *mut libc::c_void) };
    out
}

/// `~/Library/Application Support/com.tchan.oculus`, as the app computes it.
fn data_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("OCULUS_KEYD_DATA_DIR") {
        return PathBuf::from(dir);
    }
    home().join("Library/Application Support/com.tchan.oculus")
}

/// launchd sets HOME for a LaunchAgent; the password database is the fallback.
fn home() -> PathBuf {
    if let Some(h) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        return PathBuf::from(h);
    }
    let pw = unsafe { libc::getpwuid(libc::geteuid()) };
    if pw.is_null() {
        log("no HOME and no password entry");
        exit(1);
    }
    let dir = unsafe { std::ffi::CStr::from_ptr((*pw).pw_dir) };
    PathBuf::from(std::ffi::OsStr::new(&*dir.to_string_lossy()))
}

/// The master key's source, and where old items are imported from.
fn key_sources() -> (Box<dyn KeySource>, Box<dyn ops::LegacySource>) {
    #[cfg(debug_assertions)]
    if let Ok(hex) = std::env::var("OCULUS_KEYD_TEST_KEY") {
        match vault::MasterKey::from_hex(&hex) {
            Some(key) => return (Box::new(vault::StaticKey(key)), Box::new(ops::NoLegacy)),
            None => {
                log("OCULUS_KEYD_TEST_KEY is not 64 hex characters");
                exit(64);
            }
        }
    }
    (Box::new(vault::keychain::MasterItem), Box::new(ops::LegacyKeychain))
}

fn idle() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(secs) = std::env::var("OCULUS_KEYD_IDLE_SECS").ok().and_then(|s| s.parse().ok()) {
        return Duration::from_secs(secs);
    }
    IDLE
}

/// serve-local only: non-blocking like launchd's listener, so the accept loop
/// is the same.
#[cfg(debug_assertions)]
fn bind_local(path: &std::path::Path) -> RawFd {
    use std::os::fd::IntoRawFd;
    std::fs::remove_file(path).ok();
    let listener = std::os::unix::net::UnixListener::bind(path).unwrap_or_else(|e| {
        eprintln!("bind {}: {e}", path.display());
        exit(1)
    });
    listener.set_nonblocking(true).expect("set_nonblocking");
    listener.into_raw_fd()
}
