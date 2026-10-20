//! Binding the listeners. IPv4 `0.0.0.0` is required; IPv6 `[::]` on the same
//! port is added when it binds, v6-only because the IPv4 listener already
//! takes IPv4. SO_REUSEADDR (Unix) avoids TIME_WAIT on a quick restart, but
//! on macOS it also lets a wildcard bind succeed beside a listener on
//! 127.0.0.1 alone, so a loopback connect probe skips ports someone serves.

use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

const PROBE: Duration = Duration::from_millis(100);
const BACKLOG: i32 = 128;

pub struct Bound {
    pub v4: TcpListener,
    pub v6: Option<TcpListener>,
    pub port: u16,
}

/// The first free port of `preferred` and the `attempts - 1` after it;
/// `preferred == 0` takes whatever port the OS picks.
pub fn bind(preferred: u16, attempts: u16) -> io::Result<Bound> {
    let candidates: Vec<u16> = if preferred == 0 {
        vec![0]
    } else {
        (0..attempts.max(1))
            .filter_map(|offset| preferred.checked_add(offset))
            .collect()
    };
    for port in candidates {
        if port != 0 && answers((Ipv4Addr::LOCALHOST, port).into()) {
            continue;
        }
        let v4 = match listen((Ipv4Addr::UNSPECIFIED, port).into()) {
            Ok(listener) => listener,
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => continue,
            Err(error) => return Err(error),
        };
        let port = v4.local_addr()?.port();
        let v6 = if answers((Ipv6Addr::LOCALHOST, port).into()) {
            None
        } else {
            listen((Ipv6Addr::UNSPECIFIED, port).into()).ok()
        };
        return Ok(Bound { v4, v6, port });
    }
    Err(io::Error::new(
        io::ErrorKind::AddrInUse,
        format!(
            "ports {preferred} to {} are all in use",
            preferred.saturating_add(attempts.saturating_sub(1))
        ),
    ))
}

/// Something already accepts connections on `addr`.
fn answers(addr: SocketAddr) -> bool {
    TcpStream::connect_timeout(&addr, PROBE).is_ok()
}

fn listen(addr: SocketAddr) -> io::Result<TcpListener> {
    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    if addr.is_ipv6() {
        socket.set_only_v6(true)?;
    }
    socket.bind(&addr.into())?;
    socket.listen(BACKLOG)?;
    Ok(socket.into())
}
