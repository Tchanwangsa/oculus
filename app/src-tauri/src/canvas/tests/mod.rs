//! `Canvas` against a stand-in oculus-keyd that speaks the wire, and a fake
//! HTTP server for the hosts a redirect leaves Canvas for.

mod calendar;
mod download;
mod expiry;
mod redirects;
mod requests;

use keyd_core::okta::{outcome_to_wire, LoginError};
use serde_json::{json, Value};

use super::Canvas;
use crate::test_support::{FakeKeyd, Scratch};

pub(super) struct Rig {
    pub canvas: Canvas,
    pub keyd: FakeKeyd,
    pub dir: Scratch,
}

impl Rig {
    /// Every `forward` request keyd saw, as (path, whole header).
    pub fn forwards(&self) -> Vec<(String, Value)> {
        self.keyd
            .requests()
            .into_iter()
            .map(|(header, _)| header)
            .filter(|h| h["op"] == "forward")
            .map(|h| (h["path"].as_str().unwrap_or("").to_string(), h))
            .collect()
    }

    pub fn paths(&self) -> Vec<String> {
        self.forwards().into_iter().map(|(path, _)| path).collect()
    }
}

/// A Canvas whose oculus-keyd answers every request with `handler`.
pub(super) fn rig<H>(handler: H) -> Rig
where
    H: Fn(&Value, &[u8]) -> (Value, Vec<u8>) + Send + 'static,
{
    let dir = Scratch::new("canvas");
    let keyd = FakeKeyd::start(&dir, handler);
    Rig {
        canvas: Canvas::open(&dir),
        keyd,
        dir,
    }
}

/// A `forward` reply.
pub(super) fn answer(status: u16, headers: &[(&str, &str)], body: &[u8]) -> (Value, Vec<u8>) {
    let headers: Vec<[&str; 2]> = headers.iter().map(|(k, v)| [*k, *v]).collect();
    (json!({"status": status, "headers": headers}), body.to_vec())
}

/// A `forward` reply for a rejected request whose sign-in was refused.
pub(super) fn refused(
    status: u16,
    headers: &[(&str, &str)],
    body: &[u8],
    why: LoginError,
) -> (Value, Vec<u8>) {
    let (mut header, body) = answer(status, headers, body);
    header["signin"] = outcome_to_wire(&Err(why));
    (header, body)
}

pub(super) const CANVAS: &str = "https://canvas.lms.unimelb.edu.au";
