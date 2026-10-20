//! Routing. GET and POST only; Host and Origin are checked before anything
//! else. `/c/` and `/c/assets/**` are the public browser bundle; everything
//! under `/api/s/<shareId>/` needs the share's key: `snapshot`, `changes`,
//! `commit` (POST) and `ws` (its key arrives in the `hello` frame).

use super::guard::{authenticate, header, host_allowed};
use super::http::{closing, error, full, no_store, respond, Reply};
use super::{ws, Connection, Net};
use crate::companion::assets::Lookup;
use crate::companion::bridge;
use crate::companion::core::Core;
use hyper::body::Incoming;
use hyper::header::{HeaderValue, CACHE_CONTROL, HOST, LOCATION, ORIGIN};
use hyper::{Method, Request, StatusCode};
use std::sync::Arc;

pub async fn route(
    core: Arc<Core>,
    net: Arc<Net>,
    connection: Arc<Connection>,
    connection_id: u64,
    mut request: Request<Incoming>,
) -> Reply {
    let method = request.method().clone();
    if method != Method::GET && method != Method::POST {
        return closing(error(
            StatusCode::METHOD_NOT_ALLOWED,
            "method",
            "Only GET and POST are served.",
        ));
    }
    let host = header(request.headers(), HOST);
    let origin = header(request.headers(), ORIGIN);
    if !host_allowed(&core, net.listening.port, host, origin) {
        return closing(error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This server answers only on this computer's own addresses.",
        ));
    }
    let path = request.uri().path().to_string();
    if path == "/c" {
        let mut reply = respond(StatusCode::PERMANENT_REDIRECT, "text/plain", full(""));
        reply
            .headers_mut()
            .insert(LOCATION, HeaderValue::from_static("/c/"));
        return reply;
    }
    if let Some(rest) = path.strip_prefix("/c/") {
        if method != Method::GET {
            return closing(not_allowed());
        }
        return asset(&core, rest.to_string()).await;
    }
    let Some((share_id, action)) = path
        .strip_prefix("/api/s/")
        .and_then(|rest| rest.split_once('/'))
    else {
        return closing(not_found());
    };
    let share_id = share_id.to_string();
    match (action, &method) {
        ("ws", &Method::GET) => ws::upgrade(
            core,
            net,
            connection,
            connection_id,
            &share_id,
            &mut request,
        ),
        ("snapshot" | "changes", &Method::GET) | ("commit", &Method::POST) => {
            let share = match authenticate(&core, &share_id, request.headers()) {
                Ok(share) => share,
                Err(reply) => return closing(*reply),
            };
            connection.serve(&share.id);
            match action {
                "snapshot" => bridge::read::snapshot(&core, &share).await,
                "changes" => bridge::read::changes(&share, request.uri().query()),
                _ => bridge::commit::commit(&core, &share, request).await,
            }
        }
        ("ws" | "snapshot" | "changes" | "commit", _) => closing(not_allowed()),
        _ => closing(not_found()),
    }
}

fn not_found() -> Reply {
    error(
        StatusCode::NOT_FOUND,
        "not-found",
        "Nothing is served here.",
    )
}

fn not_allowed() -> Reply {
    error(
        StatusCode::METHOD_NOT_ALLOWED,
        "method",
        "This method is not served here.",
    )
}

async fn asset(core: &Arc<Core>, rest: String) -> Reply {
    let reader = core.clone();
    let lookup = tokio::task::spawn_blocking(move || reader.assets.get(&rest)).await;
    match lookup {
        Ok(Lookup::Found { bytes, mime }) => {
            let html = mime.starts_with("text/html");
            let mut reply = respond(StatusCode::OK, mime, full(bytes));
            if html {
                let headers = reply.headers_mut();
                headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
                headers.insert(
                    "content-security-policy",
                    HeaderValue::from_static("frame-ancestors 'none'"),
                );
                no_store(reply)
            } else {
                // Vite names every built asset by its content hash.
                reply.headers_mut().insert(
                    CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=31536000, immutable"),
                );
                reply
            }
        }
        Ok(Lookup::Missing) => no_store(respond(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            full("Not found."),
        )),
        Ok(Lookup::NoBundle) => no_store(respond(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            full(
                "The device-sharing page is not built: dist/companion is missing. \
                 Run `bun run companion:build` once in app/.",
            ),
        )),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "The page could not be read.",
        ),
    }
}
