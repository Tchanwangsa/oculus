use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::*;
use windows_sys::Win32::System::Threading::{
    OpenThread, ResumeThread, CREATE_NO_WINDOW, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME,
};

pub struct ProcessJob(HANDLE);
// An owned kernel handle can be closed or used from another thread.
unsafe impl Send for ProcessJob {}
unsafe impl Sync for ProcessJob {}
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
impl ProcessJob {
    pub fn terminate(&self) {
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

pub fn spawn(cmd: &mut Command) -> Result<(Child, ProcessJob), String> {
    unsafe {
        let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if handle.is_null() {
            return Err(format!(
                "cannot create agent job: {}",
                std::io::Error::last_os_error()
            ));
        }
        let job = ProcessJob(handle);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        ) == 0
        {
            return Err(format!(
                "cannot configure agent job: {}",
                std::io::Error::last_os_error()
            ));
        }
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("cannot start agent: {e}"))?;
        let result = (|| {
            if AssignProcessToJobObject(handle, child.as_raw_handle() as HANDLE) == 0 {
                return Err(format!(
                    "cannot contain agent in job: {}",
                    std::io::Error::last_os_error()
                ));
            }
            resume(child.id())
        })();
        if let Err(error) = result {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok((child, job))
    }
}

unsafe fn resume(pid: u32) -> Result<(), String> {
    let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(format!(
            "cannot enumerate suspended agent thread: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut entry: THREADENTRY32 = std::mem::zeroed();
    entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
    let mut found = Thread32First(snapshot, &mut entry);
    let mut result = Err("suspended agent thread was not found".to_string());
    while found != 0 {
        if entry.th32OwnerProcessID == pid {
            let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
            if thread.is_null() {
                result = Err(format!(
                    "cannot open suspended agent thread: {}",
                    std::io::Error::last_os_error()
                ));
            } else {
                let resumed = ResumeThread(thread);
                CloseHandle(thread);
                result = if resumed == u32::MAX {
                    Err(format!(
                        "cannot resume contained agent: {}",
                        std::io::Error::last_os_error()
                    ))
                } else {
                    Ok(())
                };
            }
            break;
        }
        found = Thread32Next(snapshot, &mut entry);
    }
    CloseHandle(snapshot);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    #[test]
    fn closing_a_job_terminates_descendant_processes() {
        let mut command = super::super::command("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-Command",
            "$child = Start-Process powershell.exe -ArgumentList '-NoProfile -NonInteractive -Command Start-Sleep 60' -WindowStyle Hidden -PassThru; Write-Output $child.Id; Start-Sleep 60"])
            .stdout(Stdio::piped()).stderr(Stdio::null());
        let (mut child, job) = spawn(&mut command).unwrap();
        let mut pid = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut pid)
            .unwrap();
        let descendant =
            unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid.trim().parse().unwrap()) };
        assert!(!descendant.is_null());
        drop(job);
        let exited = unsafe { WaitForSingleObject(descendant, 5000) };
        unsafe {
            CloseHandle(descendant);
        }
        let _ = child.wait();
        assert_eq!(
            exited, WAIT_OBJECT_0,
            "job close must terminate the descendant"
        );
    }
}
