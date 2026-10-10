//! A fake oculus-keyd on a Unix socket.

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::providers::ratelimit::hold;

/// A stand-in `oculus-keyd` on `<dir>/keyd.sock`, speaking its framing: each
/// request's header and body go to `handler`, whose header and body are the
/// reply. Its thread ends with the test process.
pub struct FakeKeyd {
    requests: Arc<Mutex<Vec<(Value, Vec<u8>)>>>,
}

impl FakeKeyd {
    pub fn start<H>(dir: &Path, handler: H) -> Self
    where
        H: Fn(&Value, &[u8]) -> (Value, Vec<u8>) + Send + 'static,
    {
        use keyd_core::framing;

        let listener =
            keyd_core::platform::Listener::bind(&crate::library::paths::keyd_socket_path(dir))
                .unwrap();
        let requests: Arc<Mutex<Vec<(Value, Vec<u8>)>>> = Arc::new(Mutex::new(Vec::new()));
        let log = requests.clone();
        std::thread::spawn(move || {
            while let Ok(conn) = listener.accept() {
                let mut stream = std::io::BufReader::new(conn);
                while let Ok(Some(line)) = framing::read_line(&mut stream, framing::MAX_LINE) {
                    let (header, len) = framing::parse_header(&line).unwrap();
                    let body = framing::read_body(&mut stream, len).unwrap();
                    let (reply, out) = handler(&header, &body);
                    let streamed = header.get("stream") == Some(&Value::Bool(true));
                    hold(&log).push((header, body));
                    // A streamed reply is its header, then the body until the
                    // connection closes, as oculus-keyd sends it.
                    if streamed {
                        framing::write_frame(stream.get_mut(), &reply, b"").ok();
                        stream.get_mut().write_all(&out).ok();
                        break;
                    }
                    framing::write_frame(stream.get_mut(), &reply, &out).ok();
                }
            }
        });
        Self { requests }
    }

    /// Every request so far, in order: header, then body.
    pub fn requests(&self) -> Vec<(Value, Vec<u8>)> {
        hold(&self.requests).clone()
    }

    /// The ops requested so far, in order.
    pub fn ops(&self) -> Vec<String> {
        self.requests()
            .iter()
            .map(|(h, _)| h["op"].as_str().unwrap_or("").to_string())
            .collect()
    }
}
