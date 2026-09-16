use serde::Deserialize;
use sqlx::Row;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

/// Port the FastAPI sidecar listens on. Must match `sidecar/main.py` and the
/// frontend's SIDECAR constant.
pub const SIDECAR_PORT: u16 = 9547;

/// Handle to the spawned Python process so we can kill it on app exit.
pub struct SidecarProcess(pub Arc<Mutex<Option<Child>>>);

/// Set this to keep Oculus off the port entirely — for running the sidecar by
/// hand under a debugger or a profiler.
const EXTERNAL_SIDECAR_ENV: &str = "OCULUS_SIDECAR_EXTERNAL";

fn health_url() -> String {
    format!("http://127.0.0.1:{SIDECAR_PORT}/health")
}

/// True if something is already answering on the sidecar port.
fn is_healthy() -> bool {
    ureq::get(&health_url())
        .timeout(std::time::Duration::from_millis(500))
        .call()
        .is_ok()
}

/// The sidecar's own /health JSON (status, pid, parser version, what the
/// quality slot is doing), for the Sync page's status card. Errors mean
/// "down" — the frontend renders that state rather than treating it as a
/// failure.
#[tauri::command]
pub fn sidecar_health() -> Result<serde_json::Value, String> {
    let text = ureq::get(&health_url())
        .timeout(std::time::Duration::from_secs(2))
        .call()
        .map_err(|e| format!("sidecar unreachable: {e}"))?
        .into_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ParseSettings {
    memory_cap_mb: u64,
    backend: String,
}

impl Default for ParseSettings {
    fn default() -> Self {
        Self {
            memory_cap_mb: 8192,
            backend: "local".into(),
        }
    }
}

fn saved_parse_settings() -> ParseSettings {
    let database = crate::paths::db_path(&crate::paths::data_dir());
    if !database.is_file() {
        return ParseSettings::default();
    }
    let mut settings: ParseSettings = tauri::async_runtime::block_on(async move {
        let pool = crate::store::pool(&database).await.ok()?;
        let row = sqlx::query("SELECT value FROM settings WHERE key = 'parse'")
            .fetch_optional(&pool)
            .await
            .ok()??;
        serde_json::from_str(&row.get::<String, _>("value")).ok()
    })
    .unwrap_or_default();
    settings.memory_cap_mb = settings.memory_cap_mb.max(5120);
    if !matches!(settings.backend.as_str(), "local" | "cloud" | "auto") {
        settings.backend = "local".into();
    }
    settings
}

pub fn set_limits(memory_cap_mb: Option<u64>, backend: Option<&str>) -> Result<serde_json::Value, String> {
    let body = serde_json::json!({
        "memory_cap_mb": memory_cap_mb,
        "backend": backend,
    });
    let text = ureq::post(&format!("http://127.0.0.1:{SIDECAR_PORT}/limits"))
        .timeout(std::time::Duration::from_secs(5))
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
        .map_err(|e| format!("sidecar limits: {e}"))?
        .into_string()
        .map_err(|e| format!("sidecar limits: unreadable response: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("sidecar limits: bad response: {e}"))
}

#[tauri::command]
pub fn sidecar_set_limits(
    memory_cap_mb: Option<u64>,
    backend: Option<String>,
) -> Result<serde_json::Value, String> {
    set_limits(memory_cap_mb, backend.as_deref())
}

// ── Reclaiming the port ──────────────────────────────────────────────────────
//
// Adopting whatever already listens on 9547 sounds thrifty and is a trap. A
// sidecar that outlives its app keeps serving stale code — after a `bun run
// tauri dev` with parser changes, the app talks to the *old* parser and every
// result looks inexplicably unchanged. Worse, a wedged one (a quality parse
// that died holding the model lock) answers /health perfectly while accepting
// work it will never do.
//
// So: whatever is there, stop it, and start a sidecar this process owns and
// can kill on exit.

/// PIDs listening on the sidecar port.
#[cfg(unix)]
fn listeners() -> Vec<i32> {
    Command::new("lsof")
        .args(["-ti", &format!("tcp:{SIDECAR_PORT}"), "-sTCP:LISTEN"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.trim().parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(unix))]
fn listeners() -> Vec<i32> {
    // netstat's table is "proto local foreign state pid"; the PID is last.
    Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()
        .ok()
        .map(|o| {
            let needle = format!(":{SIDECAR_PORT}");
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| l.contains("LISTENING") && l.split_whitespace().nth(1).is_some_and(|a| a.ends_with(&needle)))
                .filter_map(|l| l.split_whitespace().last()?.parse().ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(unix)]
fn signal(pid: i32, sig: i32) {
    // Safe: `kill` only inspects the pid and signal, and a stale pid returns
    // ESRCH rather than doing anything.
    unsafe { libc::kill(pid, sig) };
}

/// Every descendant of `root`, deepest last.
///
/// MinerU renders pages in a process pool, and those children do not die with
/// their parent — signalling only the sidecar leaves them running, reparented
/// to init, still holding the model in memory. They have to be collected
/// *before* the parent exits, because after that the parent link is gone.
///
/// Process groups would be the tidier mechanism, but putting the sidecar in
/// its own group stops Ctrl-C in a `tauri dev` terminal from reaching it, and
/// signalling a group we did not create risks taking down the user's shell.
#[cfg(unix)]
fn descendants(root: i32) -> Vec<i32> {
    let Ok(out) = Command::new("ps").args(["-eo", "pid=,ppid="]).output() else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);

    let pairs: Vec<(i32, i32)> = text
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
        })
        .collect();

    let mut found = Vec::new();
    let mut frontier = vec![root];
    while let Some(parent) = frontier.pop() {
        for &(pid, ppid) in &pairs {
            if ppid == parent && pid != root && !found.contains(&pid) {
                found.push(pid);
                frontier.push(pid);
            }
        }
    }
    found
}

#[cfg(not(unix))]
fn descendants(_root: i32) -> Vec<i32> {
    // `taskkill /T` already walks the tree.
    Vec::new()
}

/// Ask a sidecar and everything it spawned to exit, escalating if they do not.
fn stop_tree(pid: i32) {
    let children = descendants(pid);

    signal(pid, libc_sigterm());
    for &c in &children {
        signal(c, libc_sigterm());
    }

    // Short grace period on purpose. An idle uvicorn exits in well under a
    // second; one in the middle of a quality parse is inside MinerU and will
    // not come back no matter how long we wait, so a longer window only adds
    // dead time to every app start. The children are signalled either way.
    for _ in 0..12 {
        if !alive(pid) && !children.iter().copied().any(alive) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    eprintln!("[oculus] sidecar: forcing shutdown of pid {pid}");
    signal(pid, libc_sigkill());
    for &c in &children {
        signal(c, libc_sigkill());
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
}

/// Signal 0 tests for existence without delivering anything.
#[cfg(unix)]
fn alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(not(unix))]
fn alive(_pid: i32) -> bool {
    false
}

#[cfg(not(unix))]
fn signal(pid: i32, _sig: i32) {
    let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/F", "/T"]).output();
}

/// Stop anything already on the port, so the sidecar we start is ours.
fn reclaim_port() {
    let mine = std::process::id() as i32;
    let pids: Vec<i32> = listeners().into_iter().filter(|&p| p != mine).collect();
    if pids.is_empty() {
        return;
    }

    eprintln!(
        "[oculus] sidecar: stopping {} stale process(es) on :{SIDECAR_PORT}",
        pids.len()
    );
    for pid in pids {
        stop_tree(pid);
    }
    if listeners().is_empty() {
        eprintln!("[oculus] sidecar: port :{SIDECAR_PORT} released");
    } else {
        eprintln!("[oculus] sidecar: warning — :{SIDECAR_PORT} is still held");
    }
}

#[cfg(unix)]
fn libc_sigterm() -> i32 {
    libc::SIGTERM
}
#[cfg(unix)]
fn libc_sigkill() -> i32 {
    libc::SIGKILL
}
#[cfg(not(unix))]
fn libc_sigterm() -> i32 {
    0
}
#[cfg(not(unix))]
fn libc_sigkill() -> i32 {
    0
}

/// Locate `sidecar/`. In dev it sits next to the Tauri crate in the repo; in a
/// bundled app it must be shipped as a resource.
fn sidecar_dir(app: &AppHandle) -> Option<PathBuf> {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // app/
        .and_then(|p| p.parent()) // repo root
        .map(|p| p.join("sidecar"));

    if let Some(d) = dev {
        if d.join("main.py").is_file() {
            return Some(d);
        }
    }

    let bundled = app.path().resource_dir().ok()?.join("sidecar");
    bundled.join("main.py").is_file().then_some(bundled)
}

/// Prefer the project venv — it has docling and the model deps. A bare `python3`
/// almost certainly cannot import them, so we do not fall back to one.
fn find_python(dir: &Path) -> Option<PathBuf> {
    let candidates = [
        dir.join(".venv/bin/python3"),
        dir.join(".venv/bin/python"),
        dir.join(".venv/Scripts/python.exe"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

/// Start a sidecar this process owns, replacing any that is already running.
/// Failure is non-fatal: PDFs simply stay unparsed and the parse call logs a
/// miss.
pub fn spawn(app: &AppHandle) {
    if std::env::var_os(EXTERNAL_SIDECAR_ENV).is_some() {
        eprintln!(
            "[oculus] {EXTERNAL_SIDECAR_ENV} set — using the sidecar on :{SIDECAR_PORT} as-is"
        );
        if !is_healthy() {
            eprintln!("[oculus] warning: nothing is answering there yet");
        }
        return;
    }

    reclaim_port();

    let Some(dir) = sidecar_dir(app) else {
        eprintln!("[oculus] sidecar/ not found — PDF parsing disabled");
        return;
    };

    let Some(python) = find_python(&dir) else {
        eprintln!(
            "[oculus] no venv at {}/.venv — run `uv sync` there; PDF parsing disabled",
            dir.display()
        );
        return;
    };

    let parse_settings = saved_parse_settings();
    eprintln!("[oculus] starting sidecar: {}", python.display());

    match Command::new(&python)
        .arg("main.py")
        .current_dir(&dir)
        // Python block-buffers stdout when it is a pipe, which swallows the
        // sidecar's progress lines until the buffer fills. Force unbuffered so
        // `[quality] …` shows up live.
        .env("PYTHONUNBUFFERED", "1")
        .env(
            "OCULUS_SIDECAR_MEMORY_CAP_MB",
            parse_settings.memory_cap_mb.to_string(),
        )
        .env("OCULUS_MINERU_BACKEND", &parse_settings.backend)
        .env("OCULUS_DATA_DIR", crate::paths::data_dir())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
    {
        Ok(child) => {
            app.state::<SidecarProcess>()
                .0
                .lock()
                .unwrap()
                .replace(child);

            // Uvicorn needs a moment to bind. Poll rather than sleep blindly so
            // a fast start is not penalised.
            std::thread::spawn(|| {
                for _ in 0..40 {
                    if is_healthy() {
                        eprintln!("[oculus] sidecar healthy on :{SIDECAR_PORT}");
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                eprintln!("[oculus] sidecar did not become healthy within 10s");
            });
        }
        Err(e) => eprintln!("[oculus] failed to spawn sidecar: {e}"),
    }
}

// ── Exit signals ─────────────────────────────────────────────────────────────

/// The signal that asked us to stop, or 0. Written from a signal handler, so
/// nothing here may allocate, lock, or print.
#[cfg(unix)]
static EXIT_SIGNAL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

#[cfg(unix)]
extern "C" fn on_exit_signal(sig: i32) {
    use std::sync::atomic::Ordering;
    // A second signal means the first cleanup is stuck or the user is out of
    // patience — leave immediately. `_exit` is one of the few calls that is
    // safe from a handler.
    if EXIT_SIGNAL.swap(sig, Ordering::SeqCst) != 0 {
        unsafe { libc::_exit(128 + sig) };
    }
}

/// Run the sidecar cleanup on Ctrl-C and `kill`, not just on a clean quit.
///
/// `RunEvent::Exit` only fires when Tauri's event loop ends — closing the
/// window, quitting the app. A signal kills the process out from under it, so
/// the loop never gets there and `shutdown` never runs. In a `tauri dev`
/// session that is the common case: Ctrl-C, and the SIGTERM the CLI sends on
/// every rebuild, both left a sidecar orphaned on port 9547 with MinerU's
/// render pool still under it.
///
/// The handler itself only records the signal; a watcher thread does the work,
/// because shutting down takes locks and spawns processes and none of that is
/// safe inside a handler.
pub fn install_exit_handlers(app: &AppHandle) {
    #[cfg(unix)]
    {
        use std::sync::atomic::Ordering;

        unsafe {
            libc::signal(libc::SIGINT, on_exit_signal as *const () as libc::sighandler_t);
            libc::signal(libc::SIGTERM, on_exit_signal as *const () as libc::sighandler_t);
            libc::signal(libc::SIGHUP, on_exit_signal as *const () as libc::sighandler_t);
        }

        let app = app.clone();
        std::thread::spawn(move || loop {
            let sig = EXIT_SIGNAL.load(Ordering::SeqCst);
            if sig != 0 {
                eprintln!("[oculus] signal {sig} — cleaning up before exit");
                shutdown(&app);
                // 128 + signal is the shell's own convention for "died by
                // signal", so `tauri dev` and CI read this the usual way.
                std::process::exit(128 + sig);
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        });
    }
    #[cfg(not(unix))]
    let _ = app;
}

/// Stop the child on app exit so uvicorn does not outlive the window.
///
/// SIGTERM first, for the same reason as `reclaim_port`: MinerU's render pool
/// is a set of child processes, and only an orderly exit reaps them. A straight
/// kill leaves them parented to init, holding memory and the model file.
pub fn shutdown(app: &AppHandle) {
    let Some(state) = app.try_state::<SidecarProcess>() else { return };
    let Some(mut child) = state.0.lock().unwrap().take() else { return };

    eprintln!("[oculus] stopping sidecar");
    stop_tree(child.id() as i32);
    let _ = child.try_wait();
}
