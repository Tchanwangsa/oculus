//! The accept loop and one task per connection. A connection over the cap is
//! closed on accept. hyper's `header_read_timeout` bounds a slow request head
//! and an idle keep-alive wait alike; `max_buf_size` bounds the head's size.

use super::{router, Connection, Net};
use crate::companion::core::Core;
use crate::companion::limits::{CONNECTIONS, HEADER_BYTES, KEEPALIVE_INTERVAL, KEEPALIVE_TIME};
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub async fn accept_loop(core: Arc<Core>, net: Arc<Net>, listener: tokio::net::TcpListener) {
    loop {
        let stream = tokio::select! {
            _ = net.root.cancelled() => return,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => stream,
                Err(_) => {
                    // EMFILE and the like: back off instead of spinning.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
        };
        if net.connections().len() >= CONNECTIONS {
            continue;
        }
        stream.set_nodelay(true).ok();
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(KEEPALIVE_TIME)
            .with_interval(KEEPALIVE_INTERVAL);
        socket2::SockRef::from(&stream)
            .set_tcp_keepalive(&keepalive)
            .ok();
        let connection = Arc::new(Connection {
            cancel: net.root.child_token(),
            share: Mutex::new(None),
            upgraded: AtomicBool::new(false),
        });
        let id = net.next_connection.fetch_add(1, Ordering::SeqCst);
        net.connections().insert(id, connection.clone());
        let task = serve(core.clone(), net.clone(), connection, id, stream);
        net.tracker.spawn(task);
    }
}

async fn serve(
    core: Arc<Core>,
    net: Arc<Net>,
    connection: Arc<Connection>,
    id: u64,
    stream: tokio::net::TcpStream,
) {
    let (c, n, conn) = (core.clone(), net.clone(), connection.clone());
    let service = service_fn(move |request| {
        let (core, net, connection) = (c.clone(), n.clone(), conn.clone());
        async move { Ok::<_, Infallible>(router::route(core, net, connection, id, request).await) }
    });
    let serving = hyper::server::conn::http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(core.limits.header_timeout)
        .max_buf_size(HEADER_BYTES)
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades();
    tokio::pin!(serving);
    tokio::select! {
        _ = &mut serving => {}
        _ = connection.cancel.cancelled() => {}
    }
    // An upgraded socket lives on in its WebSocket task, which forgets it.
    if !connection.upgraded.load(Ordering::SeqCst) {
        net.forget(id);
    }
}
