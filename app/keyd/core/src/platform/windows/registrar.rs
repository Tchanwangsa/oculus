//! The GUI starts a detached resident broker; a CLI never activates it.
use super::security::{self, Handle};
use crate::platform::{ConnectError, Registrar, Registration};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Threading::*;

pub(crate) struct Resident;
struct Broker {
    pid: u32,
    process: Handle,
    image: PathBuf,
}
fn record(data_dir: &Path) -> PathBuf {
    data_dir.join("bin/keyd-program")
}
fn recorded(data_dir: &Path) -> Result<Option<PathBuf>, String> {
    match std::fs::read_to_string(record(data_dir)) {
        Ok(value) => Ok(Some(PathBuf::from(value.trim()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
fn lifecycle_lock(data_dir: &Path) -> Result<super::files::FileLock, String> {
    let dir = data_dir.join("bin");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    super::files::lock(&dir.join("keyd-lifecycle.lock")).map_err(|e| e.to_string())
}

/// Trust the pipe's OS server identity, never a PID supplied in a ping reply.
/// Release updates can stop an older version from the same package family.
fn connected_broker(data_dir: &Path, expected: &[PathBuf]) -> Result<Option<Broker>, String> {
    let pipe = match super::endpoint::connect_for_lifecycle(&crate::paths::socket(data_dir)) {
        Ok(pipe) => pipe,
        Err(ConnectError::Absent(_)) => return Ok(None),
        Err(ConnectError::Broken(e)) => return Err(e),
    };
    let mut pid = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe.handle.0, &mut pid) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let process = security::open_process(pid, true).map_err(|e| e.to_string())?;
    let image = security::process_image(process.0).map_err(|e| e.to_string())?;
    let sid = security::process_sid(process.0).map_err(|e| e.to_string())?;
    if sid != security::current_sid().map_err(|e| e.to_string())?
        || !image
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("oculus-keyd.exe"))
    {
        return Err("the pipe server is not this user's oculus-keyd executable".into());
    }
    if cfg!(debug_assertions) {
        if !expected
            .iter()
            .any(|path| dunce::canonicalize(path).is_ok_and(|p| p == image))
        {
            return Err("the pipe server is not the recorded or bundled development keyd".into());
        }
    } else {
        let root = security::process_package_root(process.0).map_err(|e| e.to_string())?;
        if !security::same_package_family(process.0).map_err(|e| e.to_string())?
            || !image.starts_with(root)
        {
            return Err("the pipe server is outside this Oculus MSIX package family".into());
        }
    }
    let mut still_pid = 0;
    if unsafe { GetNamedPipeServerProcessId(pipe.handle.0, &mut still_pid) } == 0
        || still_pid != pid
    {
        return Err("pipe server changed while checking it".into());
    }
    security::confirm_process(pid, process.0).map_err(|e| e.to_string())?;
    Ok(Some(Broker {
        pid,
        process,
        image,
    }))
}

fn stop(broker: Broker) -> Result<(), String> {
    if unsafe { TerminateProcess(broker.process.0, 0) } == 0 {
        let error = std::io::Error::last_os_error();
        // The pinned process may have exited after its identity was checked.
        if unsafe { WaitForSingleObject(broker.process.0, 0) } != WAIT_OBJECT_0 {
            return Err(format!("stopping resident keyd: {error}"));
        }
    }
    if unsafe { WaitForSingleObject(broker.process.0, 10_000) } != WAIT_OBJECT_0 {
        return Err("resident keyd did not stop; its registration is preserved".into());
    }
    Ok(())
}

fn expected(data_dir: &Path, bundled: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    let mut paths = recorded(data_dir)?.into_iter().collect::<Vec<_>>();
    if let Some(path) = bundled {
        paths.push(path.to_path_buf());
    }
    Ok(paths)
}
fn source_hash(program: &Path) -> Result<String, String> {
    let out = Command::new(program)
        .arg("source-hash")
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    let hash = String::from_utf8(out.stdout).map_err(|e| e.to_string())?;
    let hash = hash.trim();
    if !out.status.success() || hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the bundled keyd did not return a source hash".into());
    }
    Ok(hash.to_string())
}
fn remember(data_dir: &Path, program: &Path) -> Result<PathBuf, String> {
    let path = record(data_dir);
    super::files::replace_file(&path, |tmp| {
        super::files::write_private(tmp, program.to_string_lossy().as_bytes())
    })?;
    Ok(path)
}

impl Registrar for Resident {
    fn check(&self, _data_dir: &Path) -> Result<(), String> {
        if cfg!(debug_assertions) {
            return Ok(());
        }
        security::package_root()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    fn runs_in_place(&self, _program: &Path) -> bool {
        true
    }
    fn install(&self, program: &Path, data_dir: &Path) -> Result<PathBuf, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let name = exe.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !["app.exe", "oculus-app.exe"]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
        {
            return Err("Open Oculus to start keyd; the CLI cannot spawn a credential broker inside an agent sandbox".into());
        }
        self.check(data_dir)?;
        let program = dunce::canonicalize(program).map_err(|e| e.to_string())?;
        if !program
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("oculus-keyd.exe"))
            || (!cfg!(debug_assertions)
                && !program.starts_with(security::package_root().map_err(|e| e.to_string())?))
        {
            return Err("keyd must be the bundled Oculus credential broker".into());
        }
        let _lock = lifecycle_lock(data_dir)?;
        let wanted_hash = source_hash(&program)?;
        let client = crate::client::Client::at(data_dir);
        if let Some(broker) = connected_broker(data_dir, &expected(data_dir, Some(&program))?)? {
            if broker.image == program
                && client
                    .ping()
                    .is_ok_and(|reply| reply["source_hash"].as_str() == Some(wanted_hash.as_str()))
            {
                return remember(data_dir, &program);
            }
            stop(broker)?;
        }
        let mut child = Command::new(&program)
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_BREAKAWAY_FROM_JOB | DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("starting resident keyd outside the app job: {e}"))?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if client.ping().is_ok_and(|reply| {
                reply["pid"].as_u64() == Some(child.id() as u64)
                    && reply["source_hash"].as_str() == Some(wanted_hash.as_str())
            }) {
                let broker = connected_broker(data_dir, &expected(data_dir, Some(&program))?)?;
                if broker.is_some_and(|b| b.pid == child.id() && b.image == program) {
                    return remember(data_dir, &program);
                }
                child.kill().ok();
                child.wait().ok();
                return Err("the ready pipe is not the broker Oculus just started".into());
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                return Err(format!(
                    "resident keyd exited before opening its pipe: {status}"
                ));
            }
            if Instant::now() >= deadline {
                child.kill().ok();
                child.wait().ok();
                return Err("resident keyd did not open its named pipe".into());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn status(&self) -> Result<Registration, String> {
        let data_dir = crate::paths::data_dir();
        let registered = recorded(&data_dir)?;
        let broker = connected_broker(&data_dir, &expected(&data_dir, None)?)?;
        let loaded = broker.is_some() && crate::client::Client::at(&data_dir).ping().is_ok();
        // The actual running path detects an older MSIX version even if a
        // record or stamp already points at the newly installed package.
        let program = broker
            .map(|b| b.image)
            .or(registered)
            .map(|p| p.to_string_lossy().into_owned());
        Ok(Registration {
            path: record(&data_dir),
            program,
            loaded,
        })
    }
    fn uninstall(&self) -> Result<Vec<PathBuf>, String> {
        let data_dir = crate::paths::data_dir();
        let _lock = lifecycle_lock(&data_dir)?;
        if let Some(broker) = connected_broker(&data_dir, &expected(&data_dir, None)?)? {
            stop(broker)?;
        }
        let path = record(&data_dir);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(vec![path]),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(e.to_string()),
        }
    }
    fn retire(&self, _label: &str) -> Result<Vec<PathBuf>, String> {
        Ok(vec![])
    }
}
