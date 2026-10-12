//! Admit the current user's process only while its executable belongs to MSIX.
use super::security;
use crate::platform::{Caller, Conn, PeerCheck, Policy, Role, Stream};
use std::path::PathBuf;
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;

pub(crate) struct WindowsPeers {
    policy: Policy,
    sid: Result<String, String>,
    root: Result<PathBuf, String>,
}
impl WindowsPeers {
    pub fn new(policy: Policy) -> Self {
        Self {
            policy,
            sid: security::current_sid().map_err(|e| e.to_string()),
            root: security::package_root().map_err(|e| e.to_string()),
        }
    }
}
impl PeerCheck for WindowsPeers {
    fn inspect(&self, conn: &Conn) -> Caller {
        let mut caller = Caller::default();
        let stream = match &conn.0 {
            Stream::Os(stream) => stream,
            #[cfg(test)]
            _ => {
                caller.problems.push("not a Windows pipe".into());
                return caller;
            }
        };
        let inspect = || -> Result<(u32, PathBuf, String), String> {
            unsafe {
                let mut pid = 0;
                if GetNamedPipeClientProcessId(stream.handle.0, &mut pid) == 0 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                let process = security::open_process(pid, false).map_err(|e| e.to_string())?;
                let sid = security::process_sid(process.0).map_err(|e| e.to_string())?;
                let path = security::process_image(process.0).map_err(|e| e.to_string())?;
                let mut still_pid = 0;
                if GetNamedPipeClientProcessId(stream.handle.0, &mut still_pid) == 0
                    || still_pid != pid
                {
                    return Err("pipe client changed while checking it".into());
                }
                security::confirm_process(pid, process.0).map_err(|e| e.to_string())?;
                Ok((pid, path, sid))
            }
        };
        match inspect() {
            Ok((pid, path, sid)) => {
                caller.pid = Some(pid);
                caller.identifier = Some(sid.clone());
                caller.valid = self.sid.as_ref().is_ok_and(|own| own == &sid);
                caller.role = match path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref()
                {
                    Some("oculus.exe") => Role::Cli,
                    Some("app.exe" | "oculus-app.exe") => Role::App,
                    _ => Role::Unknown,
                };
                // Tauri's main binary is Oculus.exe; case cannot distinguish
                // it from oculus.exe, so packaging names the GUI oculus-app.exe.
                caller.path = Some(path);
            }
            Err(error) => caller.problems.push(error),
        }
        caller
    }
    fn admit(&self, caller: &Caller) -> Result<(), String> {
        if !caller.problems.is_empty() {
            return Err(caller.problems.join("; "));
        }
        if !caller.valid {
            return Err("pipe caller does not run as keyd's user".into());
        }
        if self.policy == Policy::SameUser {
            return Ok(());
        }
        let root = self.root.as_ref().map_err(Clone::clone)?;
        let path = caller
            .path
            .as_ref()
            .ok_or("pipe caller has no image path")?;
        if !path.starts_with(root) {
            return Err("pipe caller is outside the installed MSIX package".into());
        }
        Ok(())
    }
}
