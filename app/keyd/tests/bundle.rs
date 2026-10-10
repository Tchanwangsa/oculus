//! keyd's release policy against a real, signed app bundle. The build here has
//! no `dev` feature, so keyd admits only executables inside the app its helper
//! is nested in, and only while that app's seal verifies. The test lays out
//! `Oculus.app` the way the release does (`Contents/MacOS/{app,oculus}`, keyd
//! in `Contents/Helpers/Oculus Helper.app`), signs it inside-out with the
//! hardened runtime, starts the bundled keyd through its debug-only
//! `serve-local`, and connects from copies of this very test binary placed
//! inside and outside it.

#![cfg(all(target_os = "macos", debug_assertions, not(feature = "dev")))]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use keyd_core::paths;
use serde_json::{json, Value};

const SOCK_VAR: &str = "OCULUS_BUNDLE_TEST_SOCK";
const REQ_VAR: &str = "OCULUS_BUNDLE_TEST_REQ";

/// Not a test of its own: a copy of this binary runs it as the client. It
/// sends one request and prints the reply after a marker (the harness puts
/// its own `test client ...` text before it on the same line).
#[test]
fn client() {
    let (Ok(sock), Ok(req)) = (std::env::var(SOCK_VAR), std::env::var(REQ_VAR)) else {
        return;
    };
    let reply = (|| -> std::io::Result<String> {
        let stream = UnixStream::connect(sock)?;
        (&stream).write_all(format!("{req}\n").as_bytes())?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        Ok(line)
    })()
    .unwrap_or_else(|e| json!({"error": "io", "detail": e.to_string()}).to_string());
    println!("REPLY:{}", reply.trim());
}

struct Scratch {
    root: PathBuf,
    app: PathBuf,
    keyd: Option<Child>,
}

impl Scratch {
    fn macos(&self) -> PathBuf {
        self.app.join("Contents/MacOS")
    }
    fn helper(&self) -> PathBuf {
        self.app
            .join("Contents/Helpers")
            .join(paths::helper_app_name())
    }
    fn sock(&self) -> PathBuf {
        self.root.join("k.sock")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Some(mut keyd) = self.keyd.take() {
            keyd.kill().ok();
            keyd.wait().ok();
        }
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn codesign(args: &[&str], path: &Path) {
    let out = Command::new("codesign")
        .args(["--force", "-s", "-"])
        .args(args)
        .arg(path)
        .output()
        .expect("codesign runs");
    assert!(
        out.status.success(),
        "codesign {}: {}",
        path.display(),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// keyd's helper app at `helper`, laid out and signed as keyd-build.mjs does.
fn helper_app(helper: &Path) {
    std::fs::create_dir_all(helper.join("Contents/MacOS")).unwrap();
    std::fs::create_dir_all(helper.join("Contents/Resources")).unwrap();
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::copy(
        crate_dir.join("bundle/Info.plist"),
        helper.join("Contents/Info.plist"),
    )
    .unwrap();
    std::fs::copy(
        crate_dir.join("../src-tauri/icons/icon.icns"),
        helper.join("Contents/Resources/icon.icns"),
    )
    .unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_oculus-keyd"),
        paths::helper_program(helper),
    )
    .unwrap();
    codesign(&["-o", "runtime", "-i", "com.tchan.oculus.keyd"], helper);
}

/// A signed `Oculus.app` with keyd's helper app, this binary as the app and as
/// `oculus`, and a third executable the app has no role for. `app/` is the
/// main executable.
fn bundle() -> Scratch {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("keyd-bundle-{}-{n}", std::process::id()));
    let app = root.join("Oculus.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    std::fs::create_dir_all(app.join("Contents/Resources")).unwrap();
    std::fs::write(app.join("Contents/Resources/sealed.txt"), "as shipped").unwrap();
    std::fs::write(
        app.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>app</string>
<key>CFBundleIdentifier</key><string>com.tchan.oculus.bundle-test</string>
<key>CFBundleName</key><string>Oculus</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>"#,
    )
    .unwrap();

    let me = std::env::current_exe().unwrap();
    for name in ["app", "oculus", "helper"] {
        std::fs::copy(&me, macos.join(name)).unwrap();
    }
    // Inside-out, as the release is signed: the helper app (signed by the
    // build, left alone by Tauri), each executable, then the bundle.
    let mut scratch = Scratch {
        root,
        app,
        keyd: None,
    };
    helper_app(&scratch.helper());
    for name in ["helper", "oculus", "app"] {
        codesign(&["-o", "runtime"], &macos.join(name));
    }
    codesign(&["-o", "runtime"], &scratch.app);
    let verified = Command::new("codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&scratch.app)
        .status()
        .unwrap();
    assert!(verified.success(), "the scratch bundle verifies as shipped");

    let keyd = paths::helper_program(&scratch.helper());
    scratch.keyd = Some(serve(&scratch, &keyd));
    scratch
}

/// keyd at `keyd`, serving the scratch socket.
fn serve(scratch: &Scratch, keyd: &Path) -> Child {
    let sock = scratch.sock();
    let child = Command::new(keyd)
        .arg("serve-local")
        .arg(&sock)
        .env("OCULUS_KEYD_DATA_DIR", &scratch.root)
        .env("OCULUS_KEYD_TEST_KEY", "11".repeat(32))
        .env("OCULUS_KEYD_IDLE_SECS", "120")
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while !sock.exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "keyd never bound its socket"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    child
}

/// `client` run from `exe`, which decides who keyd sees at the other end.
fn ask(scratch: &Scratch, exe: &Path, req: Value) -> Value {
    let out = Command::new(exe)
        .args(["--exact", "client", "--nocapture", "--test-threads=1"])
        .env(SOCK_VAR, scratch.sock())
        .env(REQ_VAR, req.to_string())
        .output()
        .unwrap_or_else(|e| panic!("running {}: {e}", exe.display()));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find_map(|l| l.split_once("REPLY:").map(|(_, reply)| reply))
        .unwrap_or_else(|| {
            panic!(
                "{} printed no reply (exit {:?}): {stdout}{}",
                exe.display(),
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
            )
        });
    serde_json::from_str(line).unwrap_or_else(|e| panic!("{line:?}: {e}"))
}

fn ping() -> Value {
    json!({"op": "ping"})
}

fn okta_status() -> Value {
    json!({"op": "okta_status"})
}

#[test]
fn only_the_bundles_own_executables_are_admitted_and_each_gets_its_role() {
    let b = bundle();
    let macos = b.macos();

    for name in ["app", "oculus", "helper"] {
        let reply = ask(&b, &macos.join(name), ping());
        assert_eq!(
            reply["source_hash"].as_str().map(str::len),
            Some(64),
            "{name} is inside the bundle: {reply}"
        );
    }

    // The two roles that may touch the Okta credentials: the app (the bundle
    // itself as the caller path) and the CLI.
    for name in ["app", "oculus"] {
        let reply = ask(&b, &macos.join(name), okta_status());
        assert_ne!(reply["error"], "caller", "{name}: {reply}");
        assert_eq!(reply["has_password"], false, "{name}: {reply}");
    }
    let reply = ask(&b, &macos.join("helper"), okta_status());
    assert_eq!(reply["error"], "caller", "{reply}");
    assert!(
        reply["detail"].as_str().unwrap().contains("app and CLI"),
        "{reply}"
    );

    // The same signed binary, one directory out of the bundle.
    let outside = b.root.join("outsider");
    std::fs::copy(std::env::current_exe().unwrap(), &outside).unwrap();
    codesign(&["-o", "runtime"], &outside);
    let reply = ask(&b, &outside, ping());
    assert_eq!(reply["error"], "caller", "{reply}");
    assert!(
        reply["detail"].as_str().unwrap().contains("outside"),
        "{reply}"
    );
}

#[test]
fn a_bundle_edited_after_signing_is_refused_for_everyone_in_it() {
    let b = bundle();
    assert_eq!(
        ask(&b, &b.macos().join("oculus"), ping())["source_hash"]
            .as_str()
            .map(str::len),
        Some(64)
    );

    std::fs::write(
        b.app.join("Contents/Resources/sealed.txt"),
        "edited on disk",
    )
    .unwrap();
    for name in ["app", "oculus"] {
        let reply = ask(&b, &b.macos().join(name), ping());
        assert_eq!(reply["error"], "caller", "{name}: {reply}");
        assert!(
            reply["detail"].as_str().unwrap().contains("seal"),
            "{name}: {reply}"
        );
    }
}

#[test]
fn a_helper_edited_after_signing_breaks_the_apps_seal() {
    let b = bundle();
    std::fs::write(
        b.helper().join("Contents/Resources/icon.icns"),
        "not the icon",
    )
    .unwrap();
    for name in ["app", "oculus"] {
        let reply = ask(&b, &b.macos().join(name), ping());
        assert_eq!(reply["error"], "caller", "{name}: {reply}");
        assert!(
            reply["detail"].as_str().unwrap().contains("seal"),
            "{name}: {reply}"
        );
    }
}

/// The helper copied out of the app, as an install that does not run in
/// place keeps it, trusts only itself: the app and CLI beside it are outside.
#[test]
fn a_helper_outside_the_app_admits_neither_the_app_nor_the_cli() {
    let mut b = bundle();
    if let Some(mut keyd) = b.keyd.take() {
        keyd.kill().ok();
        keyd.wait().ok();
    }
    std::fs::remove_file(b.sock()).ok();
    let alone = b.root.join("bin").join(paths::helper_app_name());
    std::fs::create_dir_all(alone.parent().unwrap()).unwrap();
    let copied = Command::new("cp")
        .arg("-R")
        .arg(b.helper())
        .arg(&alone)
        .status()
        .unwrap();
    assert!(copied.success());
    b.keyd = Some(serve(&b, &paths::helper_program(&alone)));

    for name in ["app", "oculus"] {
        let reply = ask(&b, &b.macos().join(name), ping());
        assert_eq!(reply["error"], "caller", "{name}: {reply}");
        assert!(
            reply["detail"].as_str().unwrap().contains("outside"),
            "{name}: {reply}"
        );
    }
}
