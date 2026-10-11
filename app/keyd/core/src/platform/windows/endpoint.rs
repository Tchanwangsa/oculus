//! Overlapped named-pipe I/O keeps accept ticks and client read deadlines real.
use super::security::{handle, wide, Descriptor, Handle};
use crate::platform::ConnectError;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Pipes::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::IO::*;

pub(crate) struct Stream {
    pub handle: Handle,
    timeout: Mutex<Option<Duration>>,
}
impl Stream {
    fn new(handle: Handle) -> Self {
        Self {
            handle,
            timeout: Mutex::new(None),
        }
    }
    fn io(&self, buf: *mut u8, len: usize, write: bool) -> io::Result<usize> {
        if len == 0 {
            return Ok(0);
        }
        let event = handle(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) })?;
        let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
        ov.hEvent = event.0;
        let mut count = 0;
        let ok = unsafe {
            if write {
                WriteFile(
                    self.handle.0,
                    buf,
                    len.min(u32::MAX as usize) as u32,
                    &mut count,
                    &mut ov,
                )
            } else {
                ReadFile(
                    self.handle.0,
                    buf,
                    len.min(u32::MAX as usize) as u32,
                    &mut count,
                    &mut ov,
                )
            }
        };
        if ok == 0 {
            let error = unsafe { GetLastError() };
            if !write && error == ERROR_BROKEN_PIPE {
                return Ok(0);
            }
            if error != ERROR_IO_PENDING {
                return Err(io::Error::from_raw_os_error(error as i32));
            }
            let timeout = *self
                .timeout
                .lock()
                .map_err(|_| io::Error::other("pipe timeout lock poisoned"))?;
            let wait = unsafe { WaitForSingleObject(event.0, millis(timeout)) };
            if wait != WAIT_OBJECT_0 {
                // The kernel must finish using the buffer and OVERLAPPED before
                // either goes out of scope, even on a deadline or failed wait.
                unsafe {
                    CancelIoEx(self.handle.0, &ov);
                    GetOverlappedResult(self.handle.0, &ov, &mut count, 1);
                }
                return Err(if wait == WAIT_TIMEOUT {
                    io::Error::new(io::ErrorKind::TimedOut, "keyd pipe I/O timed out")
                } else {
                    io::Error::last_os_error()
                });
            }
            if unsafe { GetOverlappedResult(self.handle.0, &ov, &mut count, 0) } == 0 {
                let error = unsafe { GetLastError() };
                if !write && error == ERROR_BROKEN_PIPE {
                    return Ok(0);
                }
                return Err(io::Error::from_raw_os_error(error as i32));
            }
        }
        Ok(count as usize)
    }
}
impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io(buf.as_mut_ptr(), buf.len(), false)
    }
}
impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io(buf.as_ptr().cast_mut(), buf.len(), true)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) fn set_timeout(stream: &Stream, timeout: Option<Duration>) -> io::Result<()> {
    *stream
        .timeout
        .lock()
        .map_err(|_| io::Error::other("pipe timeout lock poisoned"))? = timeout;
    Ok(())
}
fn millis(duration: Option<Duration>) -> u32 {
    duration.map_or(INFINITE, |d| {
        d.as_millis().min((INFINITE - 1) as u128) as u32
    })
}

struct Pending {
    pipe: Option<Handle>,
    event: Handle,
    ov: Box<OVERLAPPED>,
}
unsafe impl Send for Pending {}
impl Pending {
    fn new(path: &Path, first: bool) -> io::Result<Self> {
        let descriptor = Descriptor::owner_only()?;
        let attributes = descriptor.attributes();
        let name = wide(path);
        let pipe = handle(unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX
                    | FILE_FLAG_OVERLAPPED
                    | if first {
                        FILE_FLAG_FIRST_PIPE_INSTANCE
                    } else {
                        0
                    },
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                65536,
                65536,
                0,
                &attributes,
            )
        })?;
        let event = handle(unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) })?;
        let mut ov: Box<OVERLAPPED> = Box::new(unsafe { std::mem::zeroed() });
        ov.hEvent = event.0;
        if unsafe { ConnectNamedPipe(pipe.0, ov.as_mut()) } == 0 {
            match unsafe { GetLastError() } {
                ERROR_IO_PENDING => {}
                ERROR_PIPE_CONNECTED => unsafe {
                    SetEvent(event.0);
                },
                error => return Err(io::Error::from_raw_os_error(error as i32)),
            }
        }
        Ok(Self {
            pipe: Some(pipe),
            event,
            ov,
        })
    }
    fn ready(&self) -> io::Result<bool> {
        match unsafe { WaitForSingleObject(self.event.0, 0) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(pipe) = &self.pipe {
            unsafe {
                CancelIoEx(pipe.0, self.ov.as_ref());
                let mut count = 0;
                GetOverlappedResult(pipe.0, self.ov.as_ref(), &mut count, 1);
            }
        }
    }
}
pub(crate) struct Listener {
    path: std::path::PathBuf,
    pending: Mutex<Pending>,
}
pub(crate) fn bind(path: &Path) -> io::Result<Listener> {
    let name = path.to_string_lossy();
    // Test endpoints are arbitrary paths; hash them into a pipe suffix while
    // preserving the production SID-scoped endpoint verbatim.
    let path = if name.starts_with(r"\\.\pipe\") {
        path.to_path_buf()
    } else {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hash);
        std::path::PathBuf::from(format!(
            r"\\.\pipe\com.tchan.oculus.keyd.test.{:x}",
            hash.finish()
        ))
    };
    Ok(Listener {
        pending: Mutex::new(Pending::new(&path, true)?),
        path,
    })
}
pub(crate) fn accept_any(
    listeners: &[&Listener],
    within: Option<Duration>,
) -> io::Result<Option<Stream>> {
    let start = Instant::now();
    loop {
        for listener in listeners {
            let mut pending = listener
                .pending
                .lock()
                .map_err(|_| io::Error::other("pipe listener lock poisoned"))?;
            if pending.ready()? {
                let mut count = 0;
                let pipe = pending.pipe.as_ref().unwrap();
                if unsafe { GetOverlappedResult(pipe.0, pending.ov.as_ref(), &mut count, 0) } == 0 {
                    let error = unsafe { GetLastError() };
                    if error != ERROR_PIPE_CONNECTED {
                        return Err(io::Error::from_raw_os_error(error as i32));
                    }
                }
                // Keep a listening instance alive before releasing this one.
                let replacement = Pending::new(&listener.path, false)?;
                let stream = Stream::new(pending.pipe.take().unwrap());
                *pending = replacement;
                return Ok(Some(stream));
            }
        }
        if within.is_some_and(|d| start.elapsed() >= d) {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
pub(crate) fn connect(path: &Path) -> Result<Stream, ConnectError> {
    connect_inner(path, !cfg!(debug_assertions))
}

// Lifecycle checks send no requests: an older same-family broker is verified
// by the registrar before it is stopped, even after the package root changed.
#[cfg(feature = "client")]
pub(super) fn connect_for_lifecycle(path: &Path) -> Result<Stream, ConnectError> {
    connect_inner(path, false)
}

fn connect_inner(path: &Path, verify_server: bool) -> Result<Stream, ConnectError> {
    #[cfg(not(all(feature = "client", any(test, not(debug_assertions)))))]
    let _ = verify_server;
    let name = if path.to_string_lossy().starts_with(r"\\.\pipe\") {
        path.to_path_buf()
    } else {
        use std::hash::{Hash, Hasher};
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut hash);
        format!(r"\\.\pipe\com.tchan.oculus.keyd.test.{:x}", hash.finish()).into()
    };
    let name = wide(name);
    let start = Instant::now();
    loop {
        let value = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OVERLAPPED,
                std::ptr::null_mut(),
            )
        };
        if value != INVALID_HANDLE_VALUE {
            let stream = Stream::new(Handle(value));
            #[cfg(all(feature = "client", any(test, not(debug_assertions))))]
            if verify_server {
                #[cfg(feature = "test-support")]
                if test_brokers()
                    .lock()
                    .map_err(|_| ConnectError::Broken("test broker lock poisoned".into()))?
                    .contains(path)
                {
                    authenticate_test_server(&stream)
                        .map_err(|e| ConnectError::Broken(e.to_string()))?;
                    return Ok(stream);
                }
                authenticate_server(&stream).map_err(|e| ConnectError::Broken(e.to_string()))?;
            }
            return Ok(stream);
        }
        let error = unsafe { GetLastError() };
        if error == ERROR_FILE_NOT_FOUND
            || (error == ERROR_PIPE_BUSY && start.elapsed() >= Duration::from_secs(5))
        {
            return Err(ConnectError::Absent(
                io::Error::from_raw_os_error(error as i32).to_string(),
            ));
        }
        if error != ERROR_PIPE_BUSY {
            return Err(ConnectError::Broken(
                io::Error::from_raw_os_error(error as i32).to_string(),
            ));
        }
        unsafe {
            WaitNamedPipeW(name.as_ptr(), 100);
        }
    }
}
/// Keeps one explicitly registered fake endpoint available to this test process.
#[cfg(all(feature = "client", feature = "test-support"))]
#[must_use = "the test broker is registered only while this guard is held"]
pub struct TestBrokerRegistration(std::path::PathBuf);

#[cfg(all(feature = "client", feature = "test-support"))]
impl Drop for TestBrokerRegistration {
    fn drop(&mut self) {
        if let Ok(mut paths) = test_brokers().lock() {
            paths.remove(&self.0);
        }
    }
}

#[cfg(all(feature = "client", feature = "test-support"))]
fn test_brokers() -> &'static Mutex<std::collections::BTreeSet<std::path::PathBuf>> {
    static PATHS: std::sync::OnceLock<Mutex<std::collections::BTreeSet<std::path::PathBuf>>> =
        std::sync::OnceLock::new();
    PATHS.get_or_init(|| Mutex::new(std::collections::BTreeSet::new()))
}

/// Test-support only: the production pipe is never eligible, and the server
/// must be this process even after an endpoint has been registered.
#[cfg(all(feature = "client", feature = "test-support"))]
pub fn register_test_broker(path: &Path) -> io::Result<TestBrokerRegistration> {
    let prefix = format!("{}.test.", super::pipe_name()?.display());
    if !path.to_string_lossy().starts_with(&prefix) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "only isolated test broker endpoints can be registered",
        ));
    }
    if !test_brokers()
        .lock()
        .map_err(|_| io::Error::other("test broker lock poisoned"))?
        .insert(path.to_path_buf())
    {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "test broker endpoint is already registered",
        ));
    }
    Ok(TestBrokerRegistration(path.to_path_buf()))
}

#[cfg(all(
    feature = "client",
    feature = "test-support",
    any(test, not(debug_assertions))
))]
fn authenticate_test_server(stream: &Stream) -> io::Result<()> {
    let mut pid = 0;
    if unsafe { GetNamedPipeServerProcessId(stream.handle.0, &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if pid != std::process::id() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the test pipe server is not this process",
        ));
    }
    let process = super::security::open_process(pid, false)?;
    let mut still_pid = 0;
    if unsafe { GetNamedPipeServerProcessId(stream.handle.0, &mut still_pid) } == 0
        || pid != still_pid
    {
        return Err(io::Error::other(
            "test pipe server changed while checking it",
        ));
    }
    super::security::confirm_process(pid, process.0)
}

/// No credential is sent until Windows identifies the real installed broker.
#[cfg(all(feature = "client", any(test, not(debug_assertions))))]
fn authenticate_server(stream: &Stream) -> io::Result<()> {
    use super::security;
    let mut pid = 0;
    if unsafe { GetNamedPipeServerProcessId(stream.handle.0, &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let process = security::open_process(pid, false)?;
    let image = security::process_image(process.0)?;
    if security::process_sid(process.0)? != security::current_sid()?
        || !image
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("oculus-keyd.exe"))
        || !image.starts_with(security::package_root()?)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the pipe server is not the current Oculus MSIX credential broker",
        ));
    }
    let mut still_pid = 0;
    if unsafe { GetNamedPipeServerProcessId(stream.handle.0, &mut still_pid) } == 0
        || pid != still_pid
    {
        return Err(io::Error::other("pipe server changed while checking it"));
    }
    security::confirm_process(pid, process.0)
}

pub(crate) fn activated() -> io::Result<Vec<Listener>> {
    Ok(vec![bind(&super::pipe_name()?)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform;
    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("keyd-{name}-{}", std::process::id()))
    }

    #[test]
    fn scratch_roots_never_share_the_production_pipe() {
        let production = platform::pipe_name().unwrap();
        assert_eq!(crate::paths::socket(&crate::paths::data_dir()), production);
        let a = crate::paths::socket(&scratch("isolation-a"));
        let b = crate::paths::socket(&scratch("isolation-b"));
        assert_eq!(a, crate::paths::socket(&scratch("isolation-a")));
        assert_ne!(a, production);
        assert_ne!(b, production);
        assert_ne!(a, b);
        let prefix = format!("{}.test.", production.display());
        assert!(a.to_string_lossy().starts_with(&prefix));
        assert!(b.to_string_lossy().starts_with(&prefix));

        let first = bind(&a).unwrap();
        let second = bind(&b).unwrap();
        let mut client_a = connect_inner(&a, false).unwrap();
        let mut client_b = connect_inner(&b, false).unwrap();
        let mut server_a = accept_any(&[&first], Some(Duration::from_secs(2)))
            .unwrap()
            .unwrap();
        let mut server_b = accept_any(&[&second], Some(Duration::from_secs(2)))
            .unwrap()
            .unwrap();
        set_timeout(&server_a, Some(Duration::from_secs(2))).unwrap();
        set_timeout(&server_b, Some(Duration::from_secs(2))).unwrap();
        client_a.write_all(b"first").unwrap();
        client_b.write_all(b"other").unwrap();
        let mut bytes = [0; 5];
        server_b.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"other");
        server_a.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"first");
    }

    #[test]
    fn pipe_process_ids_are_checked_against_live_process_handles() {
        let path = scratch("process-identity");
        let listener = bind(&path).unwrap();
        let client = connect_inner(&path, false).unwrap();
        let server = accept_any(&[&listener], Some(Duration::from_secs(2)))
            .unwrap()
            .unwrap();
        let mut client_pid = 0;
        let mut server_pid = 0;
        assert_ne!(
            unsafe { GetNamedPipeClientProcessId(server.handle.0, &mut client_pid) },
            0
        );
        assert_ne!(
            unsafe { GetNamedPipeServerProcessId(client.handle.0, &mut server_pid) },
            0
        );
        assert_eq!(client_pid, std::process::id());
        assert_eq!(server_pid, std::process::id());
        let process = super::super::security::open_process(server_pid, false).unwrap();
        super::super::security::confirm_process(server_pid, process.0).unwrap();

        #[cfg(feature = "server")]
        {
            use crate::platform::{PeerCheck, Policy};
            let conn = platform::Conn(platform::Stream::Os(server));
            let peers = super::super::peer::WindowsPeers::new(Policy::SameUser);
            let caller = peers.inspect(&conn);
            assert_eq!(caller.pid, Some(client_pid));
            assert_eq!(
                caller.identifier,
                Some(super::super::security::current_sid().unwrap())
            );
            assert_eq!(
                caller.path,
                Some(super::super::security::process_image(process.0).unwrap())
            );
            assert!(caller.valid, "{caller:?}");
            assert!(caller.problems.is_empty(), "{caller:?}");
            peers.admit(&caller).unwrap();
            if super::super::security::package_root().is_err() {
                assert!(super::super::peer::WindowsPeers::new(Policy::Install)
                    .admit(&caller)
                    .is_err());
            }
        }
    }

    #[cfg(feature = "client")]
    #[test]
    fn fake_broker_is_broken_before_any_request_bytes_are_sent() {
        let path = scratch("fake-broker");
        let listener = bind(&path).unwrap();
        let client = connect_inner(&path, false).unwrap();
        let mut server = accept_any(&[&listener], Some(Duration::from_secs(2)))
            .unwrap()
            .unwrap();
        assert_eq!(
            authenticate_server(&client).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        set_timeout(&server, Some(Duration::from_millis(25))).unwrap();
        assert_eq!(
            server.read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        match connect_inner(&path, true) {
            Err(ConnectError::Broken(message)) => assert!(message.contains("current Oculus MSIX")),
            Err(ConnectError::Absent(message)) => {
                panic!("identity failure became a fallback: {message}")
            }
            Ok(_) => panic!("unsigned test process impersonated the credential broker"),
        }
    }

    #[cfg(all(feature = "client", feature = "test-support"))]
    #[test]
    fn test_broker_registration_is_scoped_and_excludes_the_production_pipe() {
        let production = super::super::pipe_name().unwrap();
        assert_eq!(
            register_test_broker(&production).err().unwrap().kind(),
            io::ErrorKind::PermissionDenied
        );
        let path = crate::paths::socket(&scratch("registered-broker"));
        let listener = bind(&path).unwrap();
        let registration = register_test_broker(&path).unwrap();
        assert_eq!(
            register_test_broker(&path).err().unwrap().kind(),
            io::ErrorKind::AlreadyExists
        );
        let mut client = connect_inner(&path, true).unwrap();
        let mut server = accept_any(&[&listener], Some(Duration::from_secs(2)))
            .unwrap()
            .unwrap();
        client.write_all(b"fixture").unwrap();
        let mut bytes = [0; 7];
        server.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"fixture");
        drop(registration);
        match connect_inner(&path, true) {
            Err(ConnectError::Broken(message)) => assert!(message.contains("current Oculus MSIX")),
            _ => panic!("an expired registration bypassed MSIX verification"),
        }
    }

    #[cfg(all(feature = "client", feature = "test-support"))]
    #[test]
    fn a_registered_test_broker_must_still_run_in_this_process() {
        const CHILD_PIPE: &str = "OCULUS_KEYD_TEST_CHILD_PIPE";
        if let Some(path) = std::env::var_os(CHILD_PIPE) {
            let listener = bind(Path::new(&path)).unwrap();
            let mut server = accept_any(&[&listener], Some(Duration::from_secs(5)))
                .unwrap()
                .expect("parent did not connect to child fixture");
            set_timeout(&server, Some(Duration::from_secs(2))).unwrap();
            assert_eq!(server.read(&mut [0]).unwrap(), 0);
            return;
        }
        let path = crate::paths::socket(&scratch("other-process-broker"));
        let _registration = register_test_broker(&path).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "platform::windows::endpoint::tests::a_registered_test_broker_must_still_run_in_this_process",
                "--test-threads=1",
            ])
            .env(CHILD_PIPE, &path)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        loop {
            match connect_inner(&path, true) {
                Err(ConnectError::Absent(_)) if started.elapsed() < Duration::from_secs(4) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(ConnectError::Broken(message)) => {
                    assert!(message.contains("test pipe server is not this process"));
                    break;
                }
                _ => panic!("a registered endpoint trusted another process"),
            }
        }
        assert!(child.wait().unwrap().success());
    }

    #[test]
    fn pipes_carry_bytes_and_enforce_read_deadlines() {
        let path = std::env::temp_dir().join(format!("keyd-pipe-test-{}", std::process::id()));
        let listener = platform::Listener::bind(&path).unwrap();
        assert!(
            platform::Listener::bind(&path).is_err(),
            "another server cannot take the endpoint"
        );
        assert!(
            platform::accept_any(std::slice::from_ref(&listener), Duration::from_millis(10))
                .unwrap()
                .is_none()
        );
        let mut client = platform::connect(&path).unwrap();
        let mut server = listener.accept().unwrap();
        client.write_all(b"hi").unwrap();
        let mut buffer = [0u8; 2];
        server.read_exact(&mut buffer).unwrap();
        assert_eq!(&buffer, b"hi");
        server.write_all(b"ok").unwrap();
        client.read_exact(&mut buffer).unwrap();
        assert_eq!(&buffer, b"ok");
        client.set_timeout(Some(Duration::from_millis(25))).unwrap();
        assert_eq!(
            client.read(&mut buffer).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        // A cancelled read does not corrupt the next request's buffer.
        server.write_all(b"go").unwrap();
        client.read_exact(&mut buffer).unwrap();
        assert_eq!(&buffer, b"go");
    }
    #[test]
    fn a_missing_pipe_is_absent() {
        let path = std::env::temp_dir().join(format!("keyd-missing-pipe-{}", std::process::id()));
        assert!(matches!(
            platform::connect(&path),
            Err(ConnectError::Absent(_))
        ));
    }
}
