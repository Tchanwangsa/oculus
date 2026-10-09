//! Who is on the other end of the socket, and whether keyd serves them.
//!
//! The uid comes from `getpeereid`. The code comes from the audit token
//! (`LOCAL_PEERTOKEN`, which names a process instance, not a reusable pid)
//! through `SecCodeCopyGuestWithAttributes`. A bundled keyd admits only
//! executables inside its own app bundle, and only while that bundle's seal
//! verifies strictly; a `dev` build admits any same-user caller, because
//! dev builds have no bundle (docs/architecture.md).

use std::ffi::c_void;
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::ptr;

use core_foundation::base::TCFType;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_foundation_sys::base::{CFGetTypeID, CFRelease, CFTypeRef, OSStatus};
use core_foundation_sys::dictionary::{CFDictionaryGetValue, CFDictionaryRef};
use core_foundation_sys::string::{CFStringGetTypeID, CFStringRef};
use core_foundation_sys::url::CFURLRef;

type SecCodeRef = *const c_void;
type SecStaticCodeRef = *const c_void;

// SecCode.h / SecStaticCode.h
const K_SEC_CS_DEFAULT_FLAGS: u32 = 0;
const K_SEC_CS_SIGNING_INFORMATION: u32 = 1 << 1;
const K_SEC_CS_CHECK_ALL_ARCHITECTURES: u32 = 1 << 0;
const K_SEC_CS_CHECK_NESTED_CODE: u32 = 1 << 3;
const K_SEC_CS_STRICT_VALIDATE: u32 = 1 << 4;

// security-framework lacks the guest, static-code and signing-information calls.
#[link(name = "Security", kind = "framework")]
extern "C" {
    static kSecGuestAttributeAudit: CFStringRef;
    static kSecCodeInfoIdentifier: CFStringRef;

    fn SecCodeCopyGuestWithAttributes(host: SecCodeRef, attributes: CFDictionaryRef, flags: u32, guest: *mut SecCodeRef) -> OSStatus;
    fn SecCodeCopyStaticCode(code: SecCodeRef, flags: u32, static_code: *mut SecStaticCodeRef) -> OSStatus;
    fn SecCodeCopyPath(static_code: SecStaticCodeRef, flags: u32, path: *mut CFURLRef) -> OSStatus;
    fn SecCodeCopySigningInformation(code: SecStaticCodeRef, flags: u32, information: *mut CFDictionaryRef) -> OSStatus;
    fn SecCodeCheckValidity(code: SecCodeRef, flags: u32, requirement: *const c_void) -> OSStatus;
    fn SecStaticCodeCreateWithPath(path: CFURLRef, flags: u32, static_code: *mut SecStaticCodeRef) -> OSStatus;
    fn SecStaticCodeCheckValidity(static_code: SecStaticCodeRef, flags: u32, requirement: *const c_void) -> OSStatus;
}

/// What keyd learned about a peer. Each lookup that failed leaves its field
/// empty and says why in `problems`.
#[derive(Debug, Default)]
pub struct Caller {
    pub uid: Option<u32>,
    pub pid: Option<u32>,
    pub path: Option<PathBuf>,
    /// The signing identifier (`com.tchan.oculus`, `com.tchan.oculus.cli`):
    /// what later ops are tiered by.
    pub identifier: Option<String>,
    pub valid: bool,
    pub problems: Vec<String>,
}

impl Caller {
    /// For the log: identifier, then path, then pid.
    pub fn label(&self) -> String {
        let path = self.path.as_ref().map(|p| p.display().to_string());
        match (&self.identifier, path) {
            (Some(id), Some(p)) => format!("{id} ({p})"),
            (Some(id), None) => id.clone(),
            (None, Some(p)) => p,
            (None, None) => format!("pid {}", self.pid.map_or("?".to_string(), |p| p.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Same uid is enough. Dev builds only.
    SameUser,
    /// Same uid, and an executable inside keyd's own sealed bundle.
    Bundle,
}

impl Policy {
    pub const fn compiled() -> Policy {
        if cfg!(feature = "dev") {
            Policy::SameUser
        } else {
            Policy::Bundle
        }
    }
}

pub fn inspect(fd: RawFd) -> Caller {
    let mut c = Caller::default();

    let (mut uid, mut gid) = (0 as libc::uid_t, 0 as libc::gid_t);
    if unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } == 0 {
        c.uid = Some(uid);
    } else {
        c.problems.push(format!("getpeereid: {}", std::io::Error::last_os_error()));
    }

    // audit_token_t is 8 x u32; val[5] is the pid.
    let mut token = [0u32; 8];
    let mut len = std::mem::size_of_val(&token) as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(fd, libc::SOL_LOCAL, libc::LOCAL_PEERTOKEN, token.as_mut_ptr() as *mut c_void, &mut len)
    };
    if rc != 0 || len as usize != std::mem::size_of_val(&token) {
        c.problems.push(format!("LOCAL_PEERTOKEN: {}", std::io::Error::last_os_error()));
        return c;
    }
    c.pid = Some(token[5]);
    let token_bytes: Vec<u8> = token.iter().flat_map(|v| v.to_ne_bytes()).collect();

    let attrs = unsafe {
        CFDictionary::from_CFType_pairs(&[(
            CFString::wrap_under_get_rule(kSecGuestAttributeAudit).as_CFType(),
            CFData::from_buffer(&token_bytes).as_CFType(),
        )])
    };
    let mut code: SecCodeRef = ptr::null();
    let st = unsafe { SecCodeCopyGuestWithAttributes(ptr::null(), attrs.as_concrete_TypeRef(), K_SEC_CS_DEFAULT_FLAGS, &mut code) };
    if st != 0 || code.is_null() {
        c.problems.push(format!("SecCodeCopyGuestWithAttributes: OSStatus {st}"));
        return c;
    }

    let mut static_code: SecStaticCodeRef = ptr::null();
    let st = unsafe { SecCodeCopyStaticCode(code, K_SEC_CS_DEFAULT_FLAGS, &mut static_code) };
    if st != 0 || static_code.is_null() {
        c.problems.push(format!("SecCodeCopyStaticCode: OSStatus {st}"));
    } else {
        let mut url: CFURLRef = ptr::null();
        let st = unsafe { SecCodeCopyPath(static_code, K_SEC_CS_DEFAULT_FLAGS, &mut url) };
        if st != 0 || url.is_null() {
            c.problems.push(format!("SecCodeCopyPath: OSStatus {st}"));
        } else {
            c.path = unsafe { CFURL::wrap_under_create_rule(url) }.to_path();
        }

        let mut info: CFDictionaryRef = ptr::null();
        let st = unsafe { SecCodeCopySigningInformation(static_code, K_SEC_CS_SIGNING_INFORMATION, &mut info) };
        if st != 0 || info.is_null() {
            c.problems.push(format!("SecCodeCopySigningInformation: OSStatus {st}"));
        } else {
            c.identifier = unsafe { dict_string(info, kSecCodeInfoIdentifier) };
            unsafe { CFRelease(info as CFTypeRef) };
        }
        unsafe { CFRelease(static_code) };
    }

    let st = unsafe { SecCodeCheckValidity(code, K_SEC_CS_DEFAULT_FLAGS, ptr::null()) };
    c.valid = st == 0;
    if st != 0 {
        c.problems.push(format!("SecCodeCheckValidity: OSStatus {st}"));
    }
    unsafe { CFRelease(code) };
    c
}

unsafe fn dict_string(dict: CFDictionaryRef, key: CFStringRef) -> Option<String> {
    let v = CFDictionaryGetValue(dict, key as *const c_void) as CFTypeRef;
    if v.is_null() || CFGetTypeID(v) != CFStringGetTypeID() {
        return None;
    }
    Some(CFString::wrap_under_get_rule(v as CFStringRef).to_string())
}

/// `Ok` to serve the caller, or why not. Runs before any request is read.
pub fn admit(policy: Policy, caller: &Caller) -> Result<(), String> {
    let me = unsafe { libc::geteuid() };
    match caller.uid {
        Some(uid) if uid == me => {}
        Some(uid) => return Err(format!("uid {uid} is not keyd's uid {me}")),
        None => return Err("the caller's uid is unknown".to_string()),
    }
    if policy == Policy::SameUser {
        return Ok(());
    }

    let bundle = my_bundle()?;
    let path = caller.path.as_ref().ok_or("the caller's path is unknown")?;
    let path = path.canonicalize().map_err(|e| format!("canonicalizing {}: {e}", path.display()))?;
    if !in_bundle(&bundle, &path) {
        return Err(format!("{} is outside {}", path.display(), bundle.display()));
    }
    if !caller.valid {
        return Err("the caller's running code is not valid".to_string());
    }
    seal_check(&bundle)
}

/// The nearest `*.app` above keyd's own executable.
fn my_bundle() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe = exe.canonicalize().map_err(|e| format!("canonicalizing {}: {e}", exe.display()))?;
    exe.ancestors()
        .find(|a| a.extension().is_some_and(|x| x == "app") && a.join("Contents").is_dir())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("keyd is not inside an app bundle ({})", exe.display()))
}

/// SecCodeCopyPath reports a main executable as the bundle itself, and any
/// helper by its own path under `Contents/`.
fn in_bundle(bundle: &Path, caller: &Path) -> bool {
    caller == bundle || caller.starts_with(bundle.join("Contents"))
}

/// Strict validation of the whole bundle on disk, nested code included.
fn seal_check(bundle: &Path) -> Result<(), String> {
    let url = CFURL::from_path(bundle, true).ok_or_else(|| format!("no URL for {}", bundle.display()))?;
    let mut sc: SecStaticCodeRef = ptr::null();
    let st = unsafe { SecStaticCodeCreateWithPath(url.as_concrete_TypeRef(), K_SEC_CS_DEFAULT_FLAGS, &mut sc) };
    if st != 0 || sc.is_null() {
        return Err(format!("SecStaticCodeCreateWithPath: OSStatus {st}"));
    }
    let flags = K_SEC_CS_STRICT_VALIDATE | K_SEC_CS_CHECK_ALL_ARCHITECTURES | K_SEC_CS_CHECK_NESTED_CODE;
    let st = unsafe { SecStaticCodeCheckValidity(sc, flags, ptr::null()) };
    unsafe { CFRelease(sc as CFTypeRef) };
    if st != 0 {
        return Err(format!("the bundle's seal does not verify: OSStatus {st}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_membership_is_component_wise() {
        let b = Path::new("/Applications/Oculus.app");
        assert!(in_bundle(b, b));
        assert!(in_bundle(b, Path::new("/Applications/Oculus.app/Contents/MacOS/oculus")));
        assert!(!in_bundle(b, Path::new("/Applications/Oculus.app.evil/Contents/MacOS/x")));
        assert!(!in_bundle(b, Path::new("/Applications/Oculus.appx")));
        assert!(!in_bundle(b, Path::new("/Applications/Oculus.app/Resources/x")));
    }

    #[test]
    fn another_uid_is_refused_under_either_policy() {
        let me = unsafe { libc::geteuid() };
        let other = Caller { uid: Some(me + 1), valid: true, ..Caller::default() };
        let unknown = Caller { uid: None, valid: true, ..Caller::default() };
        for policy in [Policy::SameUser, Policy::Bundle] {
            assert!(admit(policy, &other).is_err());
            assert!(admit(policy, &unknown).is_err());
        }
        assert!(admit(Policy::SameUser, &Caller { uid: Some(me), ..Caller::default() }).is_ok());
    }

    #[test]
    fn outside_a_bundle_the_bundle_policy_admits_no_one() {
        let me = unsafe { libc::geteuid() };
        let exe = std::env::current_exe().unwrap();
        let caller = Caller { uid: Some(me), path: Some(exe), valid: true, ..Caller::default() };
        let err = admit(Policy::Bundle, &caller).unwrap_err();
        assert!(err.contains("not inside an app bundle"), "{err}");
    }
}
