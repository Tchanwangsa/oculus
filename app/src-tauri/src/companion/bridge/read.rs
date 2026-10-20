//! `GET /api/s/<id>/snapshot` and `GET /api/s/<id>/changes?from=<rev>`.
//! A snapshot is the canvas file text, serialized once by the authority and
//! streamed from that one buffer with its exact length. Catch-up joins the
//! log's stored entries into a JSON array.

use crate::companion::core::Core;
use crate::companion::grants::Share;
use crate::companion::server::http::{
    error, full, no_store, respond, session_ended, streamed, Reply,
};
use bytes::{BufMut, Bytes, BytesMut};
use hyper::header::HeaderValue;
use hyper::StatusCode;
use std::sync::Arc;

pub async fn snapshot(core: &Arc<Core>, share: &Arc<Share>) -> Reply {
    let authority = core.authority;
    let file_id = share.file_id;
    let read = tokio::task::spawn_blocking(move || authority.snapshot(file_id)).await;
    let snapshot = match read {
        Ok(Ok(snapshot)) => snapshot,
        Ok(Err(failure)) => {
            return error(
                super::status(failure.code),
                &super::code_name(&failure),
                &failure.message,
            )
        }
        Err(_) => {
            return error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "io",
                "The canvas could not be read.",
            )
        }
    };
    if share.epoch().as_deref() != Some(snapshot.epoch.as_str()) {
        return session_ended();
    }
    let body = streamed(Bytes::from(snapshot.text), share.cancel.clone());
    let mut reply = no_store(respond(StatusCode::OK, "text/plain; charset=utf-8", body));
    let headers = reply.headers_mut();
    for (name, value) in [
        ("x-oculus-epoch", snapshot.epoch),
        ("x-oculus-revision", snapshot.revision.to_string()),
        (
            "x-oculus-phase",
            if snapshot.paused { "paused" } else { "live" }.into(),
        ),
    ] {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(name, value);
        }
    }
    reply
}

pub fn changes(share: &Share, query: Option<&str>) -> Reply {
    let mut from = None;
    let mut epoch = None;
    for pair in query.unwrap_or("").split('&') {
        match pair.split_once('=') {
            Some(("from", value)) => from = value.parse::<u64>().ok(),
            Some(("epoch", value)) => epoch = Some(value),
            _ => {}
        }
    }
    let Some(from) = from else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Name the revision to catch up from.",
        );
    };
    let Ok(entries) = share.since(from, epoch) else {
        return error(
            StatusCode::GONE,
            "gone",
            "These changes are no longer kept. Reload the canvas.",
        );
    };
    let size = entries.iter().map(|e| e.json.len() + 1).sum::<usize>() + 2;
    let mut body = BytesMut::with_capacity(size);
    body.put_u8(b'[');
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            body.put_u8(b',');
        }
        body.put_slice(&entry.json);
    }
    body.put_u8(b']');
    let mut reply = no_store(respond(
        StatusCode::OK,
        "application/json",
        full(body.freeze()),
    ));
    if let Some((epoch, revision)) = share.head() {
        let headers = reply.headers_mut();
        if let Ok(value) = HeaderValue::from_str(&epoch) {
            headers.insert("x-oculus-epoch", value);
        }
        headers.insert("x-oculus-revision", HeaderValue::from(revision));
    }
    reply
}
