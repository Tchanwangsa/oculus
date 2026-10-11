//! The `dock` WKScriptMessageHandler: how a dock page's injected scripts
//! (`scripts/`) report to Rust. It is WebKit's own channel, not Tauri IPC,
//! so it reaches no command and needs no capability. A message counts only
//! from the page's main frame, over https, on one of the service's domains
//! (WebKit supplies the sender's origin, so an iframe can't speak for the
//! service). The body is parsed as hostile input (`message.rs`), and the only
//! thing a message can do is add an Inbox item.

use super::services::DockService;

pub(super) const HANDLER: &str = "dock";

/// True for an https origin whose host is one of `service.domains` or a
/// subdomain of one. Opaque origins (`://:0`) have no protocol or host.
pub(super) fn origin_allowed(service: &DockService, protocol: &str, host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    protocol == "https"
        && service.domains.iter().any(|domain| {
            host == *domain
                || host
                    .strip_suffix(domain)
                    .is_some_and(|sub| sub.len() > 1 && sub.ends_with('.'))
        })
}

#[cfg(target_os = "macos")]
pub(super) use macos::{install, remove};

#[cfg(target_os = "macos")]
mod macos {
    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_foundation::{NSDictionary, NSString};
    use objc2_web_kit::{
        WKScriptMessage, WKScriptMessageHandler, WKUserContentController, WKWebView,
    };
    use tauri::webview::Webview;
    use tauri::AppHandle;

    use super::super::inbox;
    use super::super::message::{Message, MAX_RAW};
    use super::super::services::DockService;
    use super::{origin_allowed, HANDLER};

    pub struct Ivars {
        app: AppHandle,
        service: &'static DockService,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "OculusDockMessageHandler"]
        #[ivars = Ivars]
        struct DockHandler;

        unsafe impl NSObjectProtocol for DockHandler {}

        unsafe impl WKScriptMessageHandler for DockHandler {
            #[unsafe(method(userContentController:didReceiveScriptMessage:))]
            fn did_receive(
                &self,
                _controller: &WKUserContentController,
                message: &WKScriptMessage,
            ) {
                let ivars = self.ivars();
                if let Some(message) = unsafe { read(ivars.service, message) } {
                    inbox::record(&ivars.app, ivars.service.id, message);
                }
            }
        }
    );

    impl DockHandler {
        fn new(
            mtm: MainThreadMarker,
            app: AppHandle,
            service: &'static DockService,
        ) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(Ivars { app, service });
            unsafe { msg_send![super(this), init] }
        }
    }

    /// The message, if it came from an allowed main frame and parses.
    unsafe fn read(service: &DockService, message: &WKScriptMessage) -> Option<Message> {
        let frame = message.frameInfo();
        if !frame.isMainFrame() {
            return None;
        }
        let origin = frame.securityOrigin();
        if !origin_allowed(
            service,
            &origin.protocol().to_string(),
            &origin.host().to_string(),
        ) {
            return None;
        }
        let body = message.body();
        let dict = body.downcast_ref::<NSDictionary>()?;
        Message::parse(|key| {
            let value = dict.objectForKey(&NSString::from_str(key))?;
            let text = value.downcast_ref::<NSString>()?;
            (text.length() <= MAX_RAW).then(|| text.to_string())
        })
    }

    /// Adds the handler to the page's user content controller. Pages created
    /// before it exists queue their messages (`scripts/post.js`).
    pub(in super::super) fn install(
        page: &Webview<tauri::Wry>,
        app: &AppHandle,
        service: &'static DockService,
    ) {
        let app = app.clone();
        page.with_webview(move |platform| unsafe {
            let ptr = platform.inner() as *mut WKWebView;
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            if ptr.is_null() {
                return;
            }
            let controller = (*ptr).configuration().userContentController();
            let handler = DockHandler::new(mtm, app, service);
            controller.addScriptMessageHandler_name(
                ProtocolObject::from_ref(&*handler),
                &NSString::from_str(HANDLER),
            );
        })
        .ok();
    }

    /// The controller holds its handler strongly; a closed page drops it.
    pub(in super::super) fn remove(page: &Webview<tauri::Wry>) {
        page.with_webview(|platform| unsafe {
            let ptr = platform.inner() as *mut WKWebView;
            if ptr.is_null() {
                return;
            }
            (*ptr)
                .configuration()
                .userContentController()
                .removeScriptMessageHandlerForName(&NSString::from_str(HANDLER));
        })
        .ok();
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn install(
    _page: &tauri::webview::Webview<tauri::Wry>,
    _app: &tauri::AppHandle,
    _service: &'static DockService,
) {
}

#[cfg(not(target_os = "macos"))]
pub(super) fn remove(_page: &tauri::webview::Webview<tauri::Wry>) {}

#[cfg(test)]
mod tests {
    use super::super::services::service;
    use super::origin_allowed;

    #[test]
    fn accepts_only_https_on_the_services_domains() {
        let whatsapp = service("whatsapp").unwrap();
        let instagram = service("instagram").unwrap();
        let messenger = service("messenger").unwrap();
        let discord = service("discord").unwrap();

        assert!(origin_allowed(whatsapp, "https", "web.whatsapp.com"));
        assert!(origin_allowed(instagram, "https", "www.instagram.com"));
        assert!(origin_allowed(instagram, "https", "instagram.com"));
        assert!(origin_allowed(messenger, "https", "www.facebook.com"));
        assert!(origin_allowed(messenger, "https", "www.messenger.com"));
        assert!(origin_allowed(discord, "https", "discord.com"));
        assert!(origin_allowed(discord, "https", "ptb.discord.com"));
        assert!(origin_allowed(discord, "https", "Discord.com"));

        // Opaque origins, other schemes, other services, look-alikes.
        assert!(!origin_allowed(discord, "", ""));
        assert!(!origin_allowed(discord, "http", "discord.com"));
        assert!(!origin_allowed(discord, "file", ""));
        assert!(!origin_allowed(discord, "https", "evildiscord.com"));
        assert!(!origin_allowed(discord, "https", "discord.com.evil.net"));
        assert!(!origin_allowed(discord, "https", ".discord.com"));
        assert!(!origin_allowed(discord, "https", "newassets.hcaptcha.com"));
        assert!(!origin_allowed(whatsapp, "https", "flows.whatsapp.net"));
        assert!(!origin_allowed(instagram, "https", "www.facebook.com"));
        assert!(!origin_allowed(instagram, "https", "www.fbsbx.com"));
        assert!(!origin_allowed(messenger, "https", "www.instagram.com"));
    }
}
