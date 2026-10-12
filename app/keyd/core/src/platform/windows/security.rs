//! Current-user security descriptors shared by pipes and private files.
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::Packaging::Appx::*;
use windows_sys::Win32::System::Threading::*;

pub fn wide(value: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

pub struct Handle(pub HANDLE);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn handle(value: HANDLE) -> io::Result<Handle> {
    if value.is_null() || value == INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else {
        Ok(Handle(value))
    }
}

pub fn process_sid(process: HANDLE) -> io::Result<String> {
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token) == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = handle(token)?;
        let mut size = 0;
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut size);
        let mut data = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        if GetTokenInformation(
            token.0,
            TokenUser,
            data.as_mut_ptr().cast(),
            size,
            &mut size,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        sid_string((*(data.as_ptr().cast::<TOKEN_USER>())).User.Sid)
    }
}
pub fn current_sid() -> io::Result<String> {
    process_sid(unsafe { GetCurrentProcess() })
}
pub unsafe fn sid_string(sid: PSID) -> io::Result<String> {
    let mut text = std::ptr::null_mut();
    if ConvertSidToStringSidW(sid, &mut text) == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut len = 0;
    while *text.add(len) != 0 {
        len += 1;
    }
    let result = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
    LocalFree(text.cast());
    Ok(result)
}

pub struct Descriptor(PSECURITY_DESCRIPTOR);
impl Descriptor {
    pub fn owner_only() -> io::Result<Self> {
        let sid = current_sid()?;
        let text = wide(format!("O:{sid}D:P(A;;GA;;;{sid})"));
        let mut descriptor = std::ptr::null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
    pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0,
            bInheritHandle: 0,
        }
    }
}
impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0);
        }
    }
}

/// A release broker serves only its installed MSIX package, whose files the
/// current user cannot replace. An unsigned user-writable NSIS tree is refused.
pub fn package_root() -> io::Result<PathBuf> {
    unsafe {
        let mut length = 0;
        let result = GetCurrentPackageFullName(&mut length, std::ptr::null_mut());
        if result != ERROR_INSUFFICIENT_BUFFER {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, "Windows credentials require the MSIX installation; unsigned NSIS builds cannot run keyd"));
        }
        let mut name = vec![0u16; length as usize];
        let result = GetCurrentPackageFullName(&mut length, name.as_mut_ptr());
        if result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        length = 0;
        let result = GetPackagePathByFullName(name.as_ptr(), &mut length, std::ptr::null_mut());
        if result != ERROR_INSUFFICIENT_BUFFER {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        let mut path = vec![0u16; length as usize];
        let result = GetPackagePathByFullName(name.as_ptr(), &mut length, path.as_mut_ptr());
        if result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(result as i32));
        }
        dunce::canonicalize(Path::new(&String::from_utf16_lossy(
            &path[..length.saturating_sub(1) as usize],
        )))
    }
}

/// The image and creation time come from a pinned process object; the second
/// open confirms that the pipe's numeric PID still refers to that object.
pub fn open_process(pid: u32, terminate: bool) -> io::Result<Handle> {
    let access = PROCESS_QUERY_LIMITED_INFORMATION
        | PROCESS_SYNCHRONIZE
        | if terminate { PROCESS_TERMINATE } else { 0 };
    let process = handle(unsafe { OpenProcess(access, 0, pid) })?;
    if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
        return Err(io::Error::other("pipe process has exited"));
    }
    Ok(process)
}

pub fn process_image(process: HANDLE) -> io::Result<PathBuf> {
    let mut path = vec![0u16; 32768];
    let mut size = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) } == 0 {
        return Err(io::Error::last_os_error());
    }
    dunce::canonicalize(PathBuf::from(std::ffi::OsString::from_wide(
        &path[..size as usize],
    )))
}

pub fn process_created(process: HANDLE) -> io::Result<(u32, u32)> {
    let mut creation: FILETIME = unsafe { std::mem::zeroed() };
    let mut exit = creation;
    let mut kernel = creation;
    let mut user = creation;
    if unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((creation.dwHighDateTime, creation.dwLowDateTime))
}

pub fn confirm_process(pid: u32, process: HANDLE) -> io::Result<()> {
    let current = open_process(pid, false)?;
    if process_created(current.0)? != process_created(process)?
        || unsafe { WaitForSingleObject(process, 0) } != WAIT_TIMEOUT
    {
        return Err(io::Error::other("pipe PID changed while checking it"));
    }
    Ok(())
}

#[cfg(feature = "client")]
fn package_text(query: impl Fn(*mut u32, *mut u16) -> u32) -> io::Result<Vec<u16>> {
    let mut length = 0;
    let result = query(&mut length, std::ptr::null_mut());
    if result != ERROR_INSUFFICIENT_BUFFER {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    let mut text = vec![0u16; length as usize];
    let result = query(&mut length, text.as_mut_ptr());
    if result != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(result as i32));
    }
    text.truncate(length as usize);
    Ok(text)
}

#[cfg(feature = "client")]
pub fn same_package_family(process: HANDLE) -> io::Result<bool> {
    let own = package_text(|length, text| unsafe { GetCurrentPackageFamilyName(length, text) })?;
    let theirs =
        package_text(|length, text| unsafe { GetPackageFamilyName(process, length, text) })?;
    Ok(own == theirs)
}

#[cfg(feature = "client")]
pub fn process_package_root(process: HANDLE) -> io::Result<PathBuf> {
    let name = package_text(|length, text| unsafe { GetPackageFullName(process, length, text) })?;
    let path = package_text(|length, text| unsafe {
        GetPackagePathByFullName(name.as_ptr(), length, text)
    })?;
    dunce::canonicalize(PathBuf::from(std::ffi::OsString::from_wide(
        &path[..path.len().saturating_sub(1)],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_identity_is_pinned_and_the_current_pid_is_rechecked() {
        let pid = std::process::id();
        let process = open_process(pid, false).unwrap();
        assert_eq!(process_sid(process.0).unwrap(), current_sid().unwrap());
        assert_eq!(
            process_image(process.0).unwrap(),
            dunce::canonicalize(std::env::current_exe().unwrap()).unwrap()
        );
        let before = process_created(process.0).unwrap();
        confirm_process(pid, process.0).unwrap();
        assert_eq!(process_created(process.0).unwrap(), before);
        assert!(
            confirm_process(0, process.0).is_err(),
            "another PID cannot stand in for the pinned process"
        );
    }

    #[cfg(feature = "client")]
    #[test]
    fn a_release_package_root_cannot_be_invented_by_an_unpacked_executable() {
        // Unit tests run unpackaged in CI. A packaged VM runner instead
        // confirms that Windows supplies an existing canonical root.
        if let Ok(root) = package_root() {
            assert!(root.is_dir());
            assert!(dunce::canonicalize(std::env::current_exe().unwrap())
                .unwrap()
                .starts_with(root));
        } else {
            assert!(process_package_root(unsafe { GetCurrentProcess() }).is_err());
        }
    }
}
