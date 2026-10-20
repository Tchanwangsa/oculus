use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{
    class, define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly, Message,
};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSSize, NSString, NSURL};

use super::ShareAnchor;

thread_local! {
    // Picker and service delegates are weak. Retain one complete interaction
    // until cancellation or completion, including AirDrop's separate window.
    static ACTIVE: RefCell<Option<ActiveShare>> = const { RefCell::new(None) };
}

type Completion = tokio::sync::oneshot::Sender<Result<(), String>>;

struct ActiveShare {
    picker: Retained<AnyObject>,
    delegate: Retained<ShareDelegate>,
}

struct ShareIvars {
    window: Retained<AnyObject>,
    service: RefCell<Option<Retained<AnyObject>>>,
    completion: RefCell<Option<Completion>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OculusLinkShareDelegate"]
    #[ivars = ShareIvars]
    struct ShareDelegate;

    unsafe impl NSObjectProtocol for ShareDelegate {}

    impl ShareDelegate {
        #[unsafe(method(sharingServicePicker:delegateForSharingService:))]
        fn service_delegate(&self, _picker: &AnyObject, _service: &AnyObject) -> *const Self {
            self
        }

        #[unsafe(method(sharingServicePicker:didChooseSharingService:))]
        fn chose_service(&self, _picker: &AnyObject, service: Option<&AnyObject>) {
            if let Some(service) = service {
                *self.ivars().service.borrow_mut() = Some(service.retain());
            } else {
                let choosing = self.ivars().service.borrow().is_none();
                if choosing { self.finish(Ok(())); }
            }
        }

        #[unsafe(method(sharingService:sourceWindowForShareItems:sharingContentScope:))]
        fn source_window(&self, _service: &AnyObject, _items: &AnyObject, scope: *mut isize) -> *const AnyObject {
            if !scope.is_null() {
                // NSSharingContentScopeItem: the item is a link to the canvas.
                unsafe { *scope = 0; }
            }
            Retained::as_ptr(&self.ivars().window)
        }

        #[unsafe(method(sharingService:didShareItems:))]
        fn shared(&self, _service: &AnyObject, _items: &AnyObject) {
            self.finish(Ok(()));
        }

        #[unsafe(method(sharingService:didFailToShareItems:error:))]
        fn failed(&self, _service: &AnyObject, _items: &AnyObject, error: &AnyObject) {
            let code: isize = unsafe { msg_send![error, code] };
            self.finish(if code == 3072 || code == -128 {
                Ok(())
            } else {
                Err("The Mac could not finish sharing the link. Try again or copy the link.".into())
            });
        }
    }
);

impl ShareDelegate {
    fn finish(&self, result: Result<(), String>) {
        // Keep the callback receiver alive while clearing its weak delegates.
        let active = ACTIVE.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot
                .as_ref()
                .is_some_and(|active| std::ptr::eq(&*active.delegate, self))
            {
                slot.take()
            } else {
                None
            }
        });
        if let Some(active) = active {
            unsafe {
                let _: () = msg_send![&*active.picker, setDelegate: std::ptr::null::<AnyObject>()];
                if let Some(service) = self.ivars().service.borrow_mut().take() {
                    let _: () = msg_send![&*service, setDelegate: std::ptr::null::<AnyObject>()];
                }
            }
            if let Some(completion) = self.ivars().completion.borrow_mut().take() {
                let _ = completion.send(result);
            }
        }
    }
}

pub(super) fn close() {
    let active = ACTIVE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|active| (active.picker.clone(), active.delegate.clone()))
    });
    if let Some((picker, delegate)) = active {
        // Closing a picker cannot cancel a service's independent window.
        // Keep that service alive and prevent another picker until it finishes.
        let choosing = delegate.ivars().service.borrow().is_none();
        if choosing {
            unsafe {
                let _: () = msg_send![&*picker, close];
            }
            delegate.finish(Ok(()));
        }
    }
}

pub(super) async fn show(
    webview: tauri::Webview,
    link: String,
    anchor: ShareAnchor,
) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    webview
        .with_webview(move |platform| {
            let mut completion = Some(tx);
            if let Err(error) =
                unsafe { present(platform.inner().cast(), &link, anchor, &mut completion) }
            {
                if let Some(tx) = completion.take() {
                    let _ = tx.send(Err(error));
                }
            }
        })
        .map_err(|_| "Could not open the macOS share sheet. Try again.".to_owned())?;
    rx.await
        .map_err(|_| "The Oculus window closed before sharing finished.".to_owned())?
}

unsafe fn present(
    view: *mut AnyObject,
    link: &str,
    anchor: ShareAnchor,
    completion: &mut Option<Completion>,
) -> Result<(), String> {
    if ACTIVE.with(|slot| slot.borrow().is_some()) {
        return Err("Finish or cancel the open macOS share sheet before sharing again.".into());
    }
    if view.is_null() {
        return Err("The Oculus window is not available.".into());
    }
    let window: *mut AnyObject = msg_send![view, window];
    if window.is_null() {
        return Err("The Oculus window is not visible.".into());
    }
    let url = NSURL::URLWithString(&NSString::from_str(link))
        .ok_or_else(|| "This link cannot be shared.".to_owned())?;
    let items = NSArray::from_slice(&[&*url]);
    let allocated: *mut AnyObject = msg_send![class!(NSSharingServicePicker), alloc];
    let picker: *mut AnyObject = msg_send![allocated, initWithItems: &*items];
    // initWithItems returns an owned (+1) object, transferred into Retained.
    let picker = Retained::from_raw(picker)
        .ok_or_else(|| "Could not create the macOS share sheet.".to_owned())?;

    let bounds: NSRect = msg_send![view, bounds];
    if bounds.size.width <= 0. || bounds.size.height <= 0. {
        return Err("The Oculus window is not visible.".into());
    }
    let flipped: bool = msg_send![view, isFlipped];
    let sx = bounds.size.width / anchor.viewport_width;
    let sy = bounds.size.height / anchor.viewport_height;
    let x = (anchor.x * sx).clamp(0., bounds.size.width);
    let y = (anchor.y * sy).clamp(0., bounds.size.height);
    let width = (anchor.width * sx).min(bounds.size.width - x).max(1.);
    let height = (anchor.height * sy).min(bounds.size.height - y).max(1.);
    let rect = NSRect::new(
        NSPoint::new(
            bounds.origin.x + x,
            bounds.origin.y
                + if flipped {
                    y
                } else {
                    bounds.size.height - y - height
                },
        ),
        NSSize::new(width, height),
    );
    let mtm = MainThreadMarker::new().ok_or("Sharing must open on the Mac's main thread.")?;
    let delegate = ShareDelegate::alloc(mtm).set_ivars(ShareIvars {
        window: Retained::retain(window).ok_or("The Oculus window is not visible.")?,
        service: RefCell::new(None),
        completion: RefCell::new(completion.take()),
    });
    let delegate: Retained<ShareDelegate> = msg_send![super(delegate), init];
    let _: () = msg_send![&*picker, setDelegate: &*delegate];
    ACTIVE.with(|slot| {
        *slot.borrow_mut() = Some(ActiveShare {
            picker: picker.clone(),
            delegate,
        })
    });
    // NSMaxYEdge for a flipped view, NSMinYEdge otherwise: below the button.
    let edge: usize = if flipped { 3 } else { 1 };
    let _: () = msg_send![&*picker, showRelativeToRect: rect, ofView: view, preferredEdge: edge];
    Ok(())
}
