//! A real server on loopback over a real authority and canvas file, driven
//! by a raw HTTP/1.1 client and tokio-tungstenite.

mod client;
mod e2e;
mod http;
mod lifecycle;
mod network;
mod sockets;

use super::assets::Assets;
use super::core::Config;
use super::limits::Limits;
use super::wire::{CompanionState, ShareInfo};
use super::Companion;
use crate::library::files::canvases::authority::rows::FileRows;
use crate::library::files::canvases::authority::session::Op;
use crate::library::files::canvases::authority::{format, Authority};
use crate::runtime::network::fake::{addr, fake, FakeNetwork};
use crate::test_support::Scratch;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const FILE: i64 = 7;
/// A documentation-range address standing in for this Mac's LAN address.
pub const LAN: &str = "192.0.2.10";

/// The fake network's debounce window.
pub const WINDOW: Duration = Duration::from_millis(60);

struct NoRows;

impl FileRows for NoRows {
    fn touched(&self, _: i64, _: u64) {}
    fn renamed(&self, _: i64, _: &str, _: &str) {}
    fn deleted(&self, _: i64) {}
}

pub fn shape(id: &str, x: f64) -> Value {
    json!({"id": id, "type": "rectangle", "x": x, "y": 0, "width": 40, "height": 30,
        "stroke": "#000", "fill": "transparent", "strokeWidth": 2, "style": "clean", "seed": 1})
}

pub fn document(elements: Vec<Value>) -> Value {
    json!({"type": "oculus-canvas", "version": 1, "elements": elements,
        "viewport": {"x": 0, "y": 0, "zoom": 1}, "background": "#ffffff"})
}

/// A canvas with one embedded PNG of about `bytes` base64 characters.
pub fn image_document(bytes: usize) -> Value {
    let mut data = String::from("iVBORw0KGgoAAAANSUhEUgAA");
    data.push_str(&"A".repeat(bytes - bytes % 4));
    let mut image = shape("img", 0.0);
    image["type"] = json!("image");
    image["dataUrl"] = json!(format!("data:image/png;base64,{data}"));
    document(vec![shape("a", 0.0), image])
}

pub fn add(id: &str) -> Value {
    json!({"edits": [{"id": id, "before": null, "after": shape(id, 0.0)}]})
}

pub struct Harness {
    pub root: Scratch,
    pub authority: &'static Authority,
    pub companion: Companion,
    pub states: Arc<Mutex<Vec<CompanionState>>>,
    /// This Mac's addresses: `LAN` on `en9` until a test changes them.
    pub net: FakeNetwork,
}

impl Harness {
    pub fn new(name: &str) -> Self {
        Self::with(name, Limits::default(), 0)
    }

    pub fn with(name: &str, limits: Limits, preferred_port: u16) -> Self {
        let root = Scratch::new(&format!("companion-{name}"));
        let bundle = root.join("dist/companion");
        std::fs::create_dir_all(bundle.join("assets")).unwrap();
        std::fs::write(bundle.join("index.html"), "<!doctype html><title>t</title>").unwrap();
        std::fs::write(bundle.join("assets/geometry.wasm"), b"\0asm\x01\0\0\0").unwrap();
        std::fs::write(root.join("outside.js"), "outside the bundle").unwrap();
        let authority: &'static Authority = Box::leak(Box::new(Authority::new(Arc::new(NoRows))));
        let (network, net) = fake(vec![addr(LAN, "en9")], WINDOW);
        let companion = Companion::new(
            authority,
            Assets::disk(bundle),
            Config {
                preferred_port,
                port_attempts: 20,
                limits,
                network,
            },
        );
        let states = Arc::new(Mutex::new(Vec::new()));
        let sink = states.clone();
        companion.on_state(move |state| sink.lock().unwrap().push(state.clone()));
        Self {
            root,
            authority,
            companion,
            states,
            net,
        }
    }

    /// Write a canvas for `file_id` and open its authority session.
    pub fn open(&self, file_id: i64, document: &Value) -> String {
        let rel = format!("courses/TEST/canvases/File{file_id}.canvas");
        let path = self.root.join(&rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let text = format::serialize(document).unwrap();
        std::fs::write(&path, &text).unwrap();
        self.authority
            .open(file_id, &self.root, &rel, &text)
            .unwrap();
        rel
    }

    pub fn share(&self, file_id: i64) -> Shared {
        let info = self.companion.share_start(file_id).unwrap();
        Shared::from(info)
    }

    /// A commit straight to the authority, as a desktop pane makes.
    pub fn desktop_commit(&self, file_id: i64, seq: u64, transaction: Value) {
        self.authority
            .commit(
                file_id,
                Op {
                    client_id: "desk".into(),
                    seq,
                    op_id: format!("desk-{seq}"),
                    transaction,
                    viewport: None,
                    epoch: None,
                },
            )
            .unwrap();
    }
}

/// What a test needs from a share: where it is and its key.
pub struct Shared {
    pub info: ShareInfo,
    pub port: u16,
    pub token: String,
}

impl From<ShareInfo> for Shared {
    fn from(info: ShareInfo) -> Self {
        let url = info.endpoints[0].url.clone().expect("a link");
        let token = url.split_once('#').expect("a fragment").1.to_string();
        Self {
            port: info.port,
            info,
            token,
        }
    }
}

impl Shared {
    pub fn id(&self) -> &str {
        &self.info.share_id
    }

    pub fn secret(&self) -> &str {
        self.token.split_once('.').unwrap().1
    }

    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }

    pub fn path(&self, action: &str) -> String {
        format!("/api/s/{}/{action}", self.info.share_id)
    }
}
