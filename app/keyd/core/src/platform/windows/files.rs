//! Owner-only DACLs, kernel file locks, and atomic sibling-file replacement.
use super::security::{wide, Descriptor};
use std::fs::File;
use std::io::{self, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::*;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE};
use windows_sys::Win32::System::IO::OVERLAPPED;

pub struct FileLock(#[allow(dead_code)] File);
fn private_file(path: &Path, creation: u32) -> io::Result<File> {
    let descriptor = Descriptor::owner_only()?;
    let attributes = descriptor.attributes();
    let name = wide(path);
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | READ_CONTROL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            &attributes,
            creation,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_handle(handle) };
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 || !owner_only(&file)? {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "keyd private file has a reparse point or a non-owner ACL",
        ));
    }
    Ok(file)
}
pub fn lock(path: &Path) -> io::Result<FileLock> {
    let file = private_file(path, OPEN_ALWAYS)?;
    let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
    if unsafe {
        LockFileEx(
            file.as_raw_handle(),
            LOCKFILE_EXCLUSIVE_LOCK,
            0,
            u32::MAX,
            u32::MAX,
            &mut ov,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(FileLock(file))
}
pub fn create_private(path: &Path) -> io::Result<File> {
    private_file(path, CREATE_NEW)
}
pub fn write_private(path: &Path, body: &[u8]) -> io::Result<()> {
    let mut file = private_file(path, OPEN_ALWAYS)?;
    file.set_len(0)?;
    file.write_all(body)?;
    file.sync_all()
}
fn owner_only(file: &File) -> io::Result<bool> {
    unsafe {
        let mut owner = std::ptr::null_mut();
        let mut acl: *mut ACL = std::ptr::null_mut();
        let mut descriptor = std::ptr::null_mut();
        let error = GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            &mut acl,
            std::ptr::null_mut(),
            &mut descriptor,
        );
        if error != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        let result = (|| {
            let own = super::security::current_sid()?;
            if owner.is_null() || acl.is_null() || super::security::sid_string(owner)? != own {
                return Ok(false);
            }
            let mut grants_owner = false;
            for index in 0..(*acl).AceCount {
                let mut ace = std::ptr::null_mut();
                if GetAce(acl, index as u32, &mut ace) == 0 {
                    return Err(io::Error::last_os_error());
                }
                let header = &*ace.cast::<ACE_HEADER>();
                if header.AceFlags & INHERIT_ONLY_ACE as u8 != 0 {
                    continue;
                }
                if header.AceType == ACCESS_DENIED_ACE_TYPE as u8 {
                    continue;
                }
                if header.AceType != ACCESS_ALLOWED_ACE_TYPE as u8 {
                    return Ok(false);
                }
                let entry = &*ace.cast::<ACCESS_ALLOWED_ACE>();
                if super::security::sid_string((&entry.SidStart as *const u32).cast_mut().cast())?
                    != own
                {
                    return Ok(false);
                }
                grants_owner |= entry.Mask != 0;
            }
            Ok(grants_owner)
        })();
        LocalFree(descriptor);
        result
    }
}
pub fn is_owner_only(path: &Path) -> io::Result<bool> {
    owner_only(&File::open(path)?)
}
// File contents are flushed before the MOVEFILE_WRITE_THROUGH rename. Windows
// does not provide POSIX directory fsync on an ordinary directory handle.
pub fn sync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}
pub fn set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}
pub fn replace_file(dest: &Path, fill: impl FnOnce(&Path) -> io::Result<()>) -> Result<(), String> {
    let parent = dest
        .parent()
        .ok_or("replacement needs a parent directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = dest.with_extension(format!("{}.{}.tmp", std::process::id(), sequence()));
    let result = (|| {
        create_private(&tmp).map_err(|e| e.to_string())?;
        fill(&tmp).map_err(|e| e.to_string())?;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&tmp)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        let (from, to) = (wide(&tmp), wide(dest));
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error().to_string());
        }
        Ok(())
    })();
    std::fs::remove_file(&tmp).ok();
    result
}
fn sequence() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
pub fn replace_dir(
    _dest: &Path,
    _fill: impl FnOnce(&Path) -> io::Result<()>,
) -> Result<(), String> {
    Err("Windows keyd runs in place; copying helper bundles is unsupported".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_replacements_are_atomic_and_keep_the_owner_acl() {
        let directory =
            std::env::temp_dir().join(format!("keyd-private-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("private");
        replace_file(&path, |tmp| write_private(tmp, b"first")).unwrap();
        assert!(is_owner_only(&path).unwrap());
        replace_file(&path, |tmp| write_private(tmp, b"second")).unwrap();
        assert!(is_owner_only(&path).unwrap());
        assert!(replace_file(&path, |_| Err(io::Error::other("fill failed"))).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(&directory).unwrap();
    }
    #[test]
    fn file_locks_serialize_other_threads() {
        let directory = std::env::temp_dir().join(format!("keyd-lock-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("lock");
        let first = lock(&path).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _guard = lock(&path).unwrap();
            tx.send(()).unwrap();
        });
        assert!(rx
            .recv_timeout(std::time::Duration::from_millis(50))
            .is_err());
        drop(first);
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        worker.join().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
    }
}
