//! A still of the page.
//!
//! The DOM cannot draw over a native view, so for a popup the frontend paints
//! a snapshot of the page in its slot and hides the live page behind it.
//! Raw `msg_send!`: the typed wrapper returns `NSImage` and would pull in all
//! of AppKit.

use super::*;

/// The block is `Fn` but the send consumes the sender, hence the lock.
#[cfg(target_os = "macos")]
pub(super) fn deliver(
    cell: &Mutex<Option<tokio::sync::oneshot::Sender<Option<Vec<u8>>>>>,
    png: Option<Vec<u8>>,
) {
    if let Some(tx) = cell.lock().ok().and_then(|mut slot| slot.take()) {
        let _ = tx.send(png);
    }
}

/// `NSImage` → TIFF → PNG, at the image's own pixel size (2x stays 2x).
#[cfg(target_os = "macos")]
unsafe fn png_bytes(image: *mut objc2::runtime::AnyObject) -> Option<Vec<u8>> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};

    if image.is_null() {
        return None;
    }
    let tiff: *mut AnyObject = msg_send![image, TIFFRepresentation];
    if tiff.is_null() {
        return None;
    }
    let rep: *mut AnyObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
    if rep.is_null() {
        return None;
    }
    let props: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
    // NSBitmapImageFileTypePNG.
    let data: *mut AnyObject = msg_send![rep, representationUsingType: 4usize, properties: props];
    if data.is_null() {
        return None;
    }
    let len: usize = msg_send![data, length];
    let bytes: *const u8 = msg_send![data, bytes];
    if bytes.is_null() || len == 0 {
        return None;
    }
    Some(std::slice::from_raw_parts(bytes, len).to_vec())
}

#[cfg(target_os = "macos")]
pub(super) fn snapshot_page(
    app: &AppHandle,
    id: u32,
    reply: tokio::sync::oneshot::Sender<Option<Vec<u8>>>,
) {
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let cell = std::sync::Arc::new(Mutex::new(Some(reply)));
    let Some(page) = page(app, id) else {
        deliver(&cell, None);
        return;
    };
    let outer = cell.clone();
    let queued = page.with_webview(move |platform| {
        let view = platform.inner() as *mut AnyObject;
        if view.is_null() {
            deliver(&outer, None);
            return;
        }
        let inner = outer.clone();
        let done = RcBlock::new(move |image: *mut AnyObject, _error: *mut AnyObject| {
            deliver(&inner, unsafe { png_bytes(image) });
        });
        // A nil configuration snapshots the visible viewport.
        unsafe {
            let config: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![
                view,
                takeSnapshotWithConfiguration: config,
                completionHandler: &*done,
            ];
        }
    });
    // Nothing will call the block, so release the waiter here.
    if queued.is_err() {
        deliver(&cell, None);
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn snapshot_page(
    _app: &AppHandle,
    _id: u32,
    reply: tokio::sync::oneshot::Sender<Option<Vec<u8>>>,
) {
    let _ = reply.send(None);
}
