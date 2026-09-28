//! Small OS boundaries shared by the application and its headless CLI.

use std::ffi::OsStr;
use std::process::Command;

/// Console helper programs must not flash a terminal over the desktop app.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

/// Owning this handle ties the whole process tree to the app, including crashes
/// and forced termination where Rust destructors never run.
#[cfg(windows)]
pub struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for ProcessJob {}

#[cfg(windows)]
impl ProcessJob {
    pub fn assign(child: &std::process::Child) -> std::io::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() { return Err(std::io::Error::last_os_error()); }
        let job = Self(handle);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe { SetInformationJobObject(handle, JobObjectExtendedLimitInformation, &limits as *const _ as _, std::mem::size_of_val(&limits) as u32) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        if unsafe { AssignProcessToJobObject(handle, child.as_raw_handle() as _) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(job)
    }
}

#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0); }
    }
}
