//! `oculus-keyd`: the one process that reads the keychain. It holds the
//! master key for `vault.bin` and answers the app and the CLI over its
//! endpoint, which the OS owns and hands over on the first connect. It exits
//! after a minute idle. Everything but this argv handling is `keyd_core`;
//! see docs/architecture.md.
//!
//!   oculus-keyd serve          run by the OS (the registration the app or
//!                              `oculus keyd install` writes)
//!   oculus-keyd source-hash    print the source hash this binary was built from
//!   oculus-keyd --version
//!
//! Debug builds add `serve-local <endpoint>`, which binds the endpoint itself,
//! and read OCULUS_KEYD_DATA_DIR, OCULUS_KEYD_TEST_KEY (64 hex characters, in
//! place of the keychain; old keychain items are then never read either),
//! OCULUS_KEYD_IDLE_SECS, and OCULUS_KEYD_VOYAGE_ORIGIN, OCULUS_KEYD_MINERU_ORIGIN
//! and OCULUS_KEYD_GROQ_ORIGIN (each `http://127.0.0.1:<port>`, a fake of that
//! service), and OCULUS_KEYD_CANVAS_ORIGIN and OCULUS_KEYD_SSO_ORIGIN (the
//! sign-in's fake Canvas and Okta: `http://127.0.0.1:<port>` or
//! `http://localhost:<port>`, on different hosts), and OCULUS_KEYD_NOW (Unix
//! seconds the sign-in and its attempt guard see for good, so a test never
//! waits on a TOTP window), and OCULUS_KEYD_TEST_ROLE (`app` or `cli`: the role
//! every caller is given, so a test process need not be named `app` or
//! `oculus`). Release builds have none.

use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use keyd_core::log;
use keyd_core::ops::{Build, State};
#[cfg(debug_assertions)]
use keyd_core::platform::Role;
use keyd_core::platform::{self, Listener, Policy};
use keyd_core::server::Server;
use keyd_core::vault::{KeySource, LegacySource};

const IDLE: Duration = Duration::from_secs(60);

const BUILD: Build = Build {
    version: env!("CARGO_PKG_VERSION"),
    source_hash: env!("KEYD_SOURCE_HASH"),
};

/// The `dev` feature admits any caller running as this user; a release
/// admits only its own install.
const POLICY: Policy = if cfg!(feature = "dev") {
    Policy::SameUser
} else {
    Policy::Install
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("serve") => match platform::activated() {
            Ok(listeners) => serve(listeners),
            Err(e) => {
                log(&e.to_string());
                exit(1);
            }
        },
        Some("source-hash") => println!("{}", BUILD.source_hash),
        Some("--version") => println!(
            "oculus-keyd {} ({})",
            BUILD.version,
            &BUILD.source_hash[..12]
        ),
        #[cfg(debug_assertions)]
        Some("serve-local") => match args.get(2) {
            Some(endpoint) => match Listener::bind(&PathBuf::from(endpoint)) {
                Ok(listener) => serve(vec![listener]),
                Err(e) => {
                    eprintln!("bind {endpoint}: {e}");
                    exit(1);
                }
            },
            None => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ! {
    eprintln!("usage: oculus-keyd serve | source-hash | --version");
    exit(64)
}

fn serve(listeners: Vec<Listener>) {
    log(&format!(
        "started, source {}, {POLICY:?} policy, {} listener(s)",
        &BUILD.source_hash[..12],
        listeners.len()
    ));
    let (keys, legacy) = key_sources();
    let state = State::new(BUILD, data_dir(), keys, legacy);
    #[cfg(debug_assertions)]
    let state = state.with_routes(test_routes());
    #[cfg(debug_assertions)]
    let state = with_test_origins(state);
    #[cfg(debug_assertions)]
    let state = with_test_clock(state);
    let server = Arc::new(Server::new(state, peer_check(), idle()));
    server.run(&listeners);
}

/// The compiled routes, with any secret whose `OCULUS_KEYD_<NAME>_ORIGIN` is
/// set pointed at that loopback origin instead.
#[cfg(debug_assertions)]
fn test_routes() -> keyd_core::forward::Routes {
    use keyd_core::names::{GROQ, MINERU, VOYAGE};
    let mut routes = keyd_core::forward::Routes::compiled();
    for secret in [VOYAGE, MINERU, GROQ] {
        let var = format!("OCULUS_KEYD_{}_ORIGIN", secret.to_uppercase());
        if let Ok(origin) = std::env::var(&var) {
            routes = match routes.with_origin(secret, &origin) {
                Ok(routes) => routes,
                Err(e) => {
                    log(&format!("{var}: {e}"));
                    exit(64);
                }
            };
        }
    }
    routes
}

/// The sign-in's Canvas and Okta, pointed at the loopback origins in
/// `OCULUS_KEYD_CANVAS_ORIGIN` and `OCULUS_KEYD_SSO_ORIGIN` when they are set.
#[cfg(debug_assertions)]
fn with_test_origins(state: State) -> State {
    let canvas = std::env::var("OCULUS_KEYD_CANVAS_ORIGIN").ok();
    let sso = std::env::var("OCULUS_KEYD_SSO_ORIGIN").ok();
    match state.with_origins(canvas.as_deref(), sso.as_deref()) {
        Ok(state) => state,
        Err(e) => {
            log(&format!(
                "OCULUS_KEYD_CANVAS_ORIGIN / OCULUS_KEYD_SSO_ORIGIN: {e}"
            ));
            exit(64);
        }
    }
}

/// The caller check for `POLICY`. A debug build gives every caller the role in
/// `OCULUS_KEYD_TEST_ROLE` when it is set.
fn peer_check() -> Box<dyn platform::PeerCheck> {
    let check = platform::peer_check(POLICY);
    #[cfg(debug_assertions)]
    if let Ok(name) = std::env::var("OCULUS_KEYD_TEST_ROLE") {
        let role = match name.as_str() {
            "app" => Role::App,
            "cli" => Role::Cli,
            _ => {
                log("OCULUS_KEYD_TEST_ROLE is not app or cli");
                exit(64);
            }
        };
        return Box::new(TestRole { check, role });
    }
    check
}

/// The caller check, with every caller's role replaced.
#[cfg(debug_assertions)]
struct TestRole {
    check: Box<dyn platform::PeerCheck>,
    role: Role,
}

#[cfg(debug_assertions)]
impl platform::PeerCheck for TestRole {
    fn inspect(&self, conn: &platform::Conn) -> platform::Caller {
        platform::Caller {
            role: self.role,
            ..self.check.inspect(conn)
        }
    }

    fn admit(&self, caller: &platform::Caller) -> Result<(), String> {
        self.check.admit(caller)
    }
}

/// The sign-in's clock, fixed at `OCULUS_KEYD_NOW` when it is set.
#[cfg(debug_assertions)]
fn with_test_clock(state: State) -> State {
    let Ok(text) = std::env::var("OCULUS_KEYD_NOW") else {
        return state;
    };
    match text.parse::<u64>() {
        Ok(secs) => state.with_clock(Arc::new(move || secs)),
        Err(_) => {
            log("OCULUS_KEYD_NOW is not a number of seconds");
            exit(64);
        }
    }
}

/// `keyd_core::paths::data_dir`, the app's own definition.
fn data_dir() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("OCULUS_KEYD_DATA_DIR") {
        return PathBuf::from(dir);
    }
    keyd_core::paths::data_dir()
}

/// The master key's source, and where old items are imported from.
fn key_sources() -> (Box<dyn KeySource>, Box<dyn LegacySource>) {
    #[cfg(debug_assertions)]
    if let Ok(hex) = std::env::var("OCULUS_KEYD_TEST_KEY") {
        use keyd_core::vault::{MasterKey, NoLegacy, StaticKey};
        match MasterKey::from_hex(&hex) {
            Some(key) => return (Box::new(StaticKey(key)), Box::new(NoLegacy)),
            None => {
                log("OCULUS_KEYD_TEST_KEY is not 64 hex characters");
                exit(64);
            }
        }
    }
    (platform::master_key(), platform::legacy_items())
}

fn idle() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(secs) = std::env::var("OCULUS_KEYD_IDLE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return Duration::from_secs(secs);
    }
    IDLE
}
