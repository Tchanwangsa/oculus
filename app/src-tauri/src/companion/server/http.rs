//! Responses and request bodies. Every response carries `nosniff` and
//! `no-referrer`; JSON errors are `{code, message}`. A body is read in full
//! into one checked allocation, never past its cap, with an idle bound per
//! chunk. A large response streams as 64 KiB slices of one buffer with its
//! exact length, so a revoked share stops it mid-way.

use bytes::Bytes;
use futures_util::stream;
use http_body_util::{combinators::BoxBody, BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming, SizeHint};
use hyper::header::{HeaderValue, CACHE_CONTROL, CONNECTION, CONTENT_TYPE};
use hyper::{Response, StatusCode};
use serde::Serialize;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub type Body = BoxBody<Bytes, std::io::Error>;
pub type Reply = Response<Body>;

const CHUNK: usize = 64 * 1024;

pub fn full(bytes: impl Into<Bytes>) -> Body {
    Full::new(bytes.into())
        .map_err(|never| match never {})
        .boxed()
}

/// A response with the headers every response carries.
pub fn respond(status: StatusCode, content_type: &'static str, body: Body) -> Reply {
    let mut response = Response::new(body);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response
}

pub fn json(status: StatusCode, value: &impl Serialize) -> Reply {
    match serde_json::to_vec(value) {
        Ok(bytes) => no_store(respond(status, "application/json", full(bytes))),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "Response not encoded.",
        ),
    }
}

pub fn error(status: StatusCode, code: &str, message: &str) -> Reply {
    #[derive(Serialize)]
    struct Wire<'a> {
        code: &'a str,
        message: &'a str,
    }
    let body = serde_json::to_vec(&Wire { code, message }).unwrap_or_default();
    no_store(respond(status, "application/json", full(body)))
}

pub fn session_ended() -> Reply {
    error(
        StatusCode::GONE,
        "session-ended",
        "This canvas is no longer shared.",
    )
}

/// An error after which the connection must not be reused: the request's
/// body may be partly unread.
pub fn closing(mut reply: Reply) -> Reply {
    reply
        .headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    reply
}

pub fn no_store(mut reply: Reply) -> Reply {
    reply
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    reply
}

/// `bytes` as a stream of slices with an exact Content-Length; it fails
/// (which aborts the connection) once `cancel` fires.
pub fn streamed(bytes: Bytes, cancel: CancellationToken) -> Body {
    let len = bytes.len() as u64;
    let chunks = stream::unfold(bytes, move |mut rest| {
        let cancel = cancel.clone();
        async move {
            if rest.is_empty() {
                return None;
            }
            if cancel.is_cancelled() {
                return Some((Err(std::io::Error::other("share revoked")), Bytes::new()));
            }
            let chunk = rest.split_to(rest.len().min(CHUNK));
            Some((Ok(Frame::data(chunk)), rest))
        }
    });
    Sized {
        inner: StreamBody::new(Box::pin(chunks)),
        len,
    }
    .boxed()
}

/// hyper sends Content-Length instead of chunking only for an exact size hint.
struct Sized<B> {
    inner: B,
    len: u64,
}

impl<B> hyper::body::Body for Sized<B>
where
    B: hyper::body::Body<Data = Bytes, Error = std::io::Error> + Unpin,
{
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
        Pin::new(&mut self.inner).poll_frame(cx)
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.len)
    }
}

pub enum BodyError {
    TooLarge,
    /// The client stalled, the connection broke, or memory ran out.
    Broken,
}

/// Read the whole body, refusing it past `cap` before allocating for it.
pub async fn read_capped(
    mut body: Incoming,
    declared: Option<u64>,
    cap: usize,
    idle: Duration,
) -> Result<Vec<u8>, BodyError> {
    if declared.is_some_and(|len| len > cap as u64) {
        return Err(BodyError::TooLarge);
    }
    let mut out = Vec::new();
    if let Some(len) = declared {
        out.try_reserve_exact(len as usize)
            .map_err(|_| BodyError::Broken)?;
    }
    loop {
        let frame = match tokio::time::timeout(idle, body.frame()).await {
            Ok(Some(Ok(frame))) => frame,
            Ok(None) => return Ok(out),
            Ok(Some(Err(_))) | Err(_) => return Err(BodyError::Broken),
        };
        let Ok(data) = frame.into_data() else {
            continue;
        };
        if out.len() + data.len() > cap {
            return Err(BodyError::TooLarge);
        }
        out.try_reserve(data.len()).map_err(|_| BodyError::Broken)?;
        out.extend_from_slice(&data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_streamed_body_has_an_exact_length_and_stops_on_cancel() {
        let bytes = Bytes::from(vec![7u8; 3 * CHUNK + 5]);
        let body = streamed(bytes.clone(), CancellationToken::new());
        assert_eq!(
            hyper::body::Body::size_hint(&body).exact(),
            Some(bytes.len() as u64)
        );
        assert_eq!(body.collect().await.unwrap().to_bytes(), bytes);
        let cancel = CancellationToken::new();
        let mut body = streamed(bytes, cancel.clone());
        assert!(body.frame().await.unwrap().is_ok());
        cancel.cancel();
        assert!(body.frame().await.unwrap().is_err());
    }
}
