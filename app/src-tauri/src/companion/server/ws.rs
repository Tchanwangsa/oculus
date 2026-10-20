//! The push socket: `GET /api/s/<id>/ws`. A browser cannot set headers on a
//! WebSocket, so the key arrives in the first frame, `hello`, within the
//! hello timeout. After `welcome` the server only pushes small frames:
//! `change` (inline, small entries), `revision` (pull `/changes`),
//! `presence`, `state`, and `session-ended` before it closes. Client frames
//! after `hello` only keep the socket alive. Commits never ride the socket.

use super::guard::host_ip;
use super::http::{closing, error, full, Body, Reply};
use super::{Connection, Net};
use crate::companion::core::Core;
use crate::companion::grants::{Access, EndReason, Push, Share};
use crate::companion::limits::{
    CLOSE_TIMEOUT, FRAME_BYTES, ID_CHARS, INLINE_CHANGE_BYTES, SOCKETS_PER_SHARE,
    UNAUTHENTICATED_SOCKETS, WRITE_TIMEOUT,
};
use futures_util::{SinkExt, StreamExt};
use hyper::body::Incoming;
use hyper::header::{HeaderValue, CONNECTION, HOST, SEC_WEBSOCKET_ACCEPT, UPGRADE};
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use serde_json::json;
use std::net::IpAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Role, WebSocketConfig};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

type Socket = WebSocketStream<TokioIo<hyper::upgrade::Upgraded>>;

/// Counts a socket that has not yet authenticated; uncounts on drop.
struct Unauthenticated(Arc<Net>);

impl Drop for Unauthenticated {
    fn drop(&mut self) {
        self.0.unauthenticated.fetch_sub(1, Ordering::SeqCst);
    }
}

pub fn upgrade(
    core: Arc<Core>,
    net: Arc<Net>,
    connection: Arc<Connection>,
    connection_id: u64,
    share_id: &str,
    request: &mut Request<Incoming>,
) -> Reply {
    let header = |name| {
        request
            .headers()
            .get(name)
            .and_then(|v: &HeaderValue| v.to_str().ok())
            .map(str::to_owned)
    };
    // Which of this Mac's addresses the device came in on, for the
    // endpoint list's connection counts.
    let via = header(HOST).as_deref().and_then(host_ip);
    let upgrade_header = header(UPGRADE);
    let version = header(hyper::header::SEC_WEBSOCKET_VERSION);
    let Some(key) = header(hyper::header::SEC_WEBSOCKET_KEY) else {
        return closing(bad_upgrade());
    };
    if !upgrade_header.is_some_and(|u| u.eq_ignore_ascii_case("websocket"))
        || version.as_deref() != Some("13")
    {
        return closing(bad_upgrade());
    }
    if core.grants.get(share_id).is_none() {
        return closing(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This share link is not valid.",
        ));
    }
    if net.unauthenticated.fetch_add(1, Ordering::SeqCst) >= UNAUTHENTICATED_SOCKETS {
        net.unauthenticated.fetch_sub(1, Ordering::SeqCst);
        return closing(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "busy",
            "Too many devices are connecting at once. Try again.",
        ));
    }
    let pending = Unauthenticated(net.clone());
    connection.serve(share_id);
    connection.upgraded.store(true, Ordering::SeqCst);
    let on_upgrade = hyper::upgrade::on(request);
    let share_id = share_id.to_string();
    let task_net = net.clone();
    net.tracker.spawn(async move {
        if let Ok(upgraded) = on_upgrade.await {
            let config = WebSocketConfig::default()
                .read_buffer_size(8 * 1024)
                .write_buffer_size(0)
                .max_message_size(Some(FRAME_BYTES))
                .max_frame_size(Some(FRAME_BYTES));
            let socket = WebSocketStream::from_raw_socket(
                TokioIo::new(upgraded),
                Role::Server,
                Some(config),
            )
            .await;
            session(&core, &connection, &share_id, via, socket, pending).await;
        }
        task_net.forget(connection_id);
    });
    let mut reply = Response::new(full(""));
    *reply.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    let headers = reply.headers_mut();
    headers.insert(UPGRADE, HeaderValue::from_static("websocket"));
    headers.insert(CONNECTION, HeaderValue::from_static("Upgrade"));
    if let Ok(accept) = HeaderValue::from_str(&derive_accept_key(key.as_bytes())) {
        headers.insert(SEC_WEBSOCKET_ACCEPT, accept);
    }
    reply
}

fn bad_upgrade() -> Response<Body> {
    error(
        StatusCode::BAD_REQUEST,
        "invalid",
        "Expected a WebSocket upgrade.",
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hello {
    r#type: String,
    client_id: String,
    secret: String,
}

/// Wait for `hello` and check its key; on failure the socket is closed.
async fn greet(
    core: &Core,
    share_id: &str,
    socket: &mut Socket,
) -> Result<Arc<Share>, (CloseCode, &'static str, Option<EndReason>)> {
    let refused = (CloseCode::Policy, "hello required", None);
    let first = timeout(core.limits.hello_timeout, socket.next())
        .await
        .map_err(|_| refused)?;
    let Some(Ok(Message::Text(text))) = first else {
        return Err(refused);
    };
    let hello: Hello = serde_json::from_str(text.as_str()).map_err(|_| refused)?;
    if hello.r#type != "hello" || hello.client_id.is_empty() || hello.client_id.len() > ID_CHARS {
        return Err(refused);
    }
    // The link fragment is `<shareId>.<secret>`; accept it whole too.
    let secret = hello
        .secret
        .strip_prefix(share_id)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(&hello.secret);
    match core.grants.authorize(share_id, secret) {
        Access::Granted(share) => Ok(share),
        Access::Ended(reason) => Err((CloseCode::Normal, "session ended", Some(reason))),
        Access::Denied => Err((CloseCode::Policy, "not valid", None)),
    }
}

async fn session(
    core: &Arc<Core>,
    connection: &Connection,
    share_id: &str,
    via: Option<IpAddr>,
    mut socket: Socket,
    pending: Unauthenticated,
) {
    let greeted = tokio::select! {
        greeted = greet(core, share_id, &mut socket) => greeted,
        _ = connection.cancel.cancelled() => Err((CloseCode::Away, "server stopping", None)),
    };
    let share = match greeted {
        Ok(share) => share,
        Err((code, reason, ended)) => {
            if let Some(ended) = ended {
                send(&mut socket, ended_frame(ended)).await;
            }
            close(&mut socket, code, reason).await;
            return;
        }
    };
    drop(pending);
    if !share.join(SOCKETS_PER_SHARE, via) {
        close(&mut socket, CloseCode::Again, "too many devices").await;
        return;
    }
    core.notify();
    let pushes = share.subscribe();
    serve(&share, connection, &mut socket, pushes, core).await;
    share.leave(via);
    core.notify();
}

async fn serve(
    share: &Share,
    connection: &Connection,
    socket: &mut Socket,
    mut pushes: tokio::sync::broadcast::Receiver<Push>,
    core: &Core,
) {
    let Some((epoch, revision)) = share.head() else {
        return;
    };
    let welcome = json!({"type": "welcome", "epoch": epoch, "revision": revision, "clients": share.clients()});
    if !send(socket, welcome.to_string()).await {
        return;
    }
    let ping_every = core.limits.ping_every;
    let dead_after = core.limits.dead_after;
    let mut tick = tokio::time::interval(ping_every.min(dead_after) / 2);
    let mut last_heard = Instant::now();
    let mut last_ping = Instant::now();
    loop {
        tokio::select! {
            // Revoking a share cancels both tokens; the share says why.
            _ = share.cancel.cancelled() => return goodbye(share, socket).await,
            _ = connection.cancel.cancelled() => return goodbye(share, socket).await,
            push = pushes.recv() => {
                let frame = match push {
                    Ok(Push::Change(entry)) if entry.json.len() <= INLINE_CHANGE_BYTES => {
                        // `{"type":"change",` + the entry's own fields.
                        let mut frame = String::with_capacity(entry.json.len() + 16);
                        frame.push_str("{\"type\":\"change\",");
                        frame.push_str(std::str::from_utf8(&entry.json[1..]).unwrap_or("}"));
                        frame
                    }
                    Ok(Push::Change(entry)) => revision_frame(&entry.epoch, entry.revision),
                    Ok(Push::State(phase)) => json!({"type": "state", "phase": phase}).to_string(),
                    Ok(Push::Presence(clients)) => json!({"type": "presence", "clients": clients}).to_string(),
                    Err(RecvError::Lagged(_)) => match share.head() {
                        Some((epoch, revision)) => revision_frame(&epoch, revision),
                        None => continue,
                    },
                    Err(RecvError::Closed) => return,
                };
                if !send(socket, frame).await {
                    return;
                }
            }
            message = socket.next() => match message {
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => return,
                // Protocol pings are answered by tungstenite; a text `pong`
                // or anything else only counts as the client being alive.
                Some(Ok(_)) => last_heard = Instant::now(),
            },
            _ = tick.tick() => {
                if last_heard.elapsed() > dead_after {
                    return;
                }
                if last_ping.elapsed() >= ping_every {
                    last_ping = Instant::now();
                    if !matches!(timeout(WRITE_TIMEOUT, socket.send(Message::Ping(Default::default()))).await, Ok(Ok(()))) {
                        return;
                    }
                    // Page code never sees protocol pings, so its own dead
                    // timer needs a text frame; it answers `{"type":"pong"}`.
                    if !send(socket, r#"{"type":"ping"}"#.to_string()).await {
                        return;
                    }
                }
            }
        }
    }
}

/// `session-ended` with the share's reason (a server stopping without one
/// is `shutdown`), then Close.
async fn goodbye(share: &Share, socket: &mut Socket) {
    let reason = share.ended().unwrap_or(EndReason::Shutdown);
    send_briefly(socket, ended_frame(reason)).await;
    close(socket, CloseCode::Normal, "session ended").await;
}

fn revision_frame(epoch: &str, revision: u64) -> String {
    json!({"type": "revision", "epoch": epoch, "revision": revision}).to_string()
}

fn ended_frame(reason: EndReason) -> String {
    json!({"type": "session-ended", "reason": reason}).to_string()
}

/// A bounded write; false when the peer is gone or too slow.
async fn send(socket: &mut Socket, text: String) -> bool {
    matches!(
        timeout(WRITE_TIMEOUT, socket.send(Message::text(text))).await,
        Ok(Ok(()))
    )
}

/// A goodbye write, bounded tightly so revoke and shutdown stay fast.
async fn send_briefly(socket: &mut Socket, text: String) {
    timeout(CLOSE_TIMEOUT, socket.send(Message::text(text)))
        .await
        .ok();
}

async fn close(socket: &mut Socket, code: CloseCode, reason: &'static str) {
    let frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    timeout(CLOSE_TIMEOUT, socket.close(Some(frame))).await.ok();
}
