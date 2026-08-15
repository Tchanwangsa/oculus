use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

/// Port the FastAPI sidecar listens on. Must match `sidecar/main.py` and the
/// frontend's SIDECAR constant.
pub const SIDECAR_PORT: u16 = 9547;

/// Handle to the spawned Python process so we can kill it on app exit.
pub struct SidecarProcess(pub Arc<Mutex<Option<Child>>>);

fn health_url() -> String {
    format!("http://127.0.0.1:{SIDECAR_PORT}/health")
}

/// True if something is already answering on the sidecar port — either a
/// previous run of ours or a sidecar the user started by hand.
fn is_healthy() -> bool {
    ureq::get(&health_url())
        .timeout(std::time::Duration::from_millis(500))
        .call()
        .is_ok()
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

/// Spawn the parsing sidecar if one is not already running. Failure is
/// non-fatal: PDFs simply stay unparsed and `trigger_pdf_parse` logs a miss.
pub fn spawn(app: &AppHandle) {
    if is_healthy() {
        eprintln!("[oculus] sidecar already running on :{SIDECAR_PORT}");
        return;
    }

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

    eprintln!("[oculus] starting sidecar: {}", python.display());

    match Command::new(&python)
        .arg("main.py")
        .current_dir(&dir)
        // Python block-buffers stdout when it is a pipe, which swallows the
        // sidecar's progress lines until the buffer fills. Force unbuffered so
        // `[quality] …` shows up live.
        .env("PYTHONUNBUFFERED", "1")
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

/// Kill the child on app exit so uvicorn does not outlive the window.
pub fn shutdown(app: &AppHandle) {
    if let Some(state) = app.try_state::<SidecarProcess>() {
        if let Some(mut child) = state.0.lock().unwrap().take() {
            eprintln!("[oculus] stopping sidecar");
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
