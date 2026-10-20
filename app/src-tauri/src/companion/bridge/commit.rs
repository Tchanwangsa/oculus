//! `POST /api/s/<id>/commit`: `{clientId, seq, opId, epoch, transaction}`.
//! The only write path. The client id becomes `web:<clientId>`, so a device
//! can never pose as a desktop pane and panes can tell their own echoes
//! apart. Idempotent by (clientId, seq) through the authority's receipts:
//! a retry after a lost response gets the original ack.

use super::{code_name, status};
use crate::companion::core::Core;
use crate::companion::grants::Share;
use crate::companion::limits::ID_CHARS;
use crate::companion::server::http::{
    closing, error, json, read_capped, session_ended, BodyError, Reply,
};
use crate::library::files::canvases::authority::error::AuthorityError;
use crate::library::files::canvases::authority::session::{Ack, Op};
use hyper::body::Incoming;
use hyper::header::CONTENT_LENGTH;
use hyper::{Request, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Body {
    client_id: String,
    seq: u64,
    op_id: String,
    epoch: String,
    transaction: Value,
}

pub async fn commit(core: &Arc<Core>, share: &Arc<Share>, request: Request<Incoming>) -> Reply {
    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    let limits = core.limits;
    let read = read_capped(
        request.into_body(),
        declared,
        limits.commit_bytes,
        limits.body_idle,
    );
    let bytes = tokio::select! {
        read = read => read,
        _ = share.cancel.cancelled() => return closing(session_ended()),
    };
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(BodyError::TooLarge) => {
            return closing(error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too-large",
                "This change is too large to send.",
            ))
        }
        Err(BodyError::Broken) => {
            return closing(error(
                StatusCode::BAD_REQUEST,
                "invalid",
                "The change did not arrive in full.",
            ))
        }
    };
    let authority = core.authority;
    let file_id = share.file_id;
    let applied = tokio::task::spawn_blocking(move || apply(authority, file_id, &bytes)).await;
    match applied {
        Ok(Ok(ack)) => json(StatusCode::OK, &ack),
        Ok(Err(failure)) => error(status(failure.code), &code_name(&failure), &failure.message),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "io",
            "The change could not be applied.",
        ),
    }
}

/// Parse the body and commit it. Runs on a blocking thread: parsing a
/// large body and writing the canvas both take a while.
fn apply(
    authority: &crate::library::files::canvases::authority::Authority,
    file_id: i64,
    bytes: &[u8],
) -> Result<Ack, AuthorityError> {
    let body: Body = serde_json::from_slice(bytes)
        .map_err(|_| AuthorityError::invalid("The change is not a valid commit."))?;
    let valid_id = |id: &str| !id.is_empty() && id.len() <= ID_CHARS;
    if !valid_id(&body.client_id) || !valid_id(&body.op_id) {
        return Err(AuthorityError::invalid(
            "A change needs a client id and an operation id.",
        ));
    }
    authority.commit(
        file_id,
        Op {
            client_id: format!("web:{}", body.client_id),
            seq: body.seq,
            op_id: body.op_id,
            transaction: body.transaction,
            viewport: None,
            epoch: Some(body.epoch),
        },
    )
}
