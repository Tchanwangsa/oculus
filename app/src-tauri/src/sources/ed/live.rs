//! Holding a challenge workspace live while it is downloaded. Ed's workspace
//! server only hands out a download for a workspace someone has open, so the
//! sync opens the web app's WebSocket with the connect ticket, waits for its
//! `init` frame, and closes it once the zip is in.
//!
//! The socket goes straight to the workspace host over rustls: the ticket is
//! the credential, so it is never sent through keyd and never logged.

use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tungstenite::client::IntoClientRequest;
use tungstenite::{Message, WebSocket};

/// Ed's AU `workspaceWebSocketUrl` host.
const HOST: &str = "sahara.au.edstem.org";
/// The web app's origin, which the browser sends on the handshake.
const ORIGIN: &str = "https://edstem.org";
/// How long the server has to send `init`, and the bound on every read and
/// write on the socket.
const INIT_WAIT: Duration = Duration::from_secs(30);

type Tls = rustls::StreamOwned<rustls::ClientConnection, TcpStream>;

/// An open workspace session; dropping it closes the socket.
pub struct LiveWorkspace {
    socket: WebSocket<Tls>,
}

impl Drop for LiveWorkspace {
    fn drop(&mut self) {
        let _ = self.socket.close(None);
        let _ = self.socket.flush();
    }
}

/// Open the workspace socket with a connect ticket and wait for `init`.
pub fn open_live_workspace(ticket: &str) -> Result<LiveWorkspace, String> {
    let tcp = connect_tcp()?;
    let name = rustls::pki_types::ServerName::try_from(HOST)
        .map_err(|_| "the workspace host is not a valid name".to_string())?;
    let tls = rustls::ClientConnection::new(Arc::new(tls_config()), name)
        .map_err(|e| format!("could not start TLS to the workspace server: {e}"))?;
    let stream = rustls::StreamOwned::new(tls, tcp);

    let ticket: String = url::form_urlencoded::byte_serialize(ticket.as_bytes()).collect();
    let mut request = format!("wss://{HOST}/connect?ticket={ticket}")
        .into_client_request()
        .map_err(|_| "the workspace ticket does not make a valid URL".to_string())?;
    request.headers_mut().insert(
        "origin",
        tungstenite::http::HeaderValue::from_static(ORIGIN),
    );
    let (socket, _) = tungstenite::client(request, stream).map_err(|e| match e {
        tungstenite::HandshakeError::Failure(e) => socket_error(&e),
        tungstenite::HandshakeError::Interrupted(_) => {
            "the workspace server did not finish the handshake".to_string()
        }
    })?;
    let mut live = LiveWorkspace { socket };
    live.wait_for_init()?;
    Ok(live)
}

impl LiveWorkspace {
    /// Read frames until `init`, ignoring every other kind. tungstenite
    /// queues the pong for a ping and sends it on the next read.
    fn wait_for_init(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + INIT_WAIT;
        while Instant::now() < deadline {
            match self.socket.read() {
                Ok(Message::Text(text)) => {
                    let frame: serde_json::Value =
                        serde_json::from_str(text.as_str()).unwrap_or_default();
                    if frame["type"] == "init" {
                        return Ok(());
                    }
                }
                Ok(Message::Close(_)) => {
                    return Err("the workspace server closed the session before it opened".into())
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    break;
                }
                Err(e) => return Err(socket_error(&e)),
            }
        }
        Err(format!(
            "the workspace server did not open the session within {} s",
            INIT_WAIT.as_secs()
        ))
    }
}

fn connect_tcp() -> Result<TcpStream, String> {
    let unreachable = |e: std::io::Error| format!("could not reach {HOST}: {e}");
    let addrs = (HOST, 443).to_socket_addrs().map_err(unreachable)?;
    let mut last = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, INIT_WAIT) {
            Ok(tcp) => {
                tcp.set_read_timeout(Some(INIT_WAIT)).map_err(unreachable)?;
                tcp.set_write_timeout(Some(INIT_WAIT))
                    .map_err(unreachable)?;
                return Ok(tcp);
            }
            Err(e) => last = Some(e),
        }
    }
    Err(last.map_or_else(|| format!("{HOST} has no address"), unreachable))
}

fn tls_config() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
        .expect("ring supports TLS 1.2 and 1.3")
        .with_root_certificates(roots)
        .with_no_client_auth()
}

/// A socket failure in words. Never the request URL: it carries the ticket.
fn socket_error(e: &tungstenite::Error) -> String {
    match e {
        tungstenite::Error::Http(response) => format!(
            "the workspace server refused the session (HTTP {})",
            response.status().as_u16()
        ),
        tungstenite::Error::Url(_) => "the workspace ticket does not make a valid URL".into(),
        tungstenite::Error::Io(io) => format!("the workspace session failed: {}", io.kind()),
        other => format!("the workspace session failed: {other}"),
    }
}
