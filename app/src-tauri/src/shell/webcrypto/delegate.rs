//! The website data store delegate that hands WebKit its WebCrypto master
//! key. WebKit asks it through private SPI, `-[WKWebsiteDataStore
//! set_delegate:]` and `webCryptoMasterKey:`, for page, worker and
//! service-worker IndexedDB alike, on the main thread, once per wrap or
//! unwrap. Answering nil makes WebKit read its own keychain item.
//!
//! One instance serves every store and lives for the process: WebKit holds
//! it weakly. The first ask fetches from keyd on a thread of its own, and the
//! answer comes back to the main thread for every handler waiting on it
//! (`cache.rs`). Each handler is called exactly once, as WebKit checks.

use std::cell::{OnceCell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};

use block2::{Block, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_foundation::NSData;
use objc2_web_kit::WKWebsiteDataStore;
use tauri::AppHandle;

use super::cache::{Ask, Key, KeyCache};

/// The completion handler WebKit passes: `void (^)(NSData *)`.
type Handler = Block<dyn Fn(*mut NSData)>;
/// A handler copied to answer later.
type Kept = RcBlock<dyn Fn(*mut NSData)>;

thread_local! {
    // Main thread only: WebKit calls the delegate there, the fetch's answer
    // is sent back there, and blocks are not `Send`.
    static DELEGATE: OnceCell<Retained<KeyDelegate>> = const { OnceCell::new() };
    static CACHE: RefCell<KeyCache<Kept>> = const { RefCell::new(KeyCache::new()) };
}

static UNSUPPORTED_LOGGED: AtomicBool = AtomicBool::new(false);

pub struct Ivars {
    app: AppHandle,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "OculusWebCryptoKeyDelegate"]
    #[ivars = Ivars]
    pub struct KeyDelegate;

    unsafe impl NSObjectProtocol for KeyDelegate {}

    impl KeyDelegate {
        /// `- (void)webCryptoMasterKey:(void (^)(NSData *))completionHandler`.
        #[unsafe(method(webCryptoMasterKey:))]
        fn web_crypto_master_key(&self, handler: Option<&Handler>) {
            if let Some(handler) = handler {
                self.ask(handler);
            }
        }
    }
);

impl KeyDelegate {
    fn new(mtm: MainThreadMarker, app: AppHandle) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { app });
        unsafe { msg_send![super(this), init] }
    }

    fn ask(&self, handler: &Handler) {
        // The handler outlives this call only as a copy.
        let Some(copy) = (unsafe { RcBlock::copy(std::ptr::from_ref(handler).cast_mut()) }) else {
            elog!("[oculus] webcrypto: could not keep WebKit's handler; WebKit reads its own keychain item");
            answer(handler, None);
            return;
        };
        // `try_with`: a panic here would abort, and at exit the cache is gone.
        match CACHE.try_with(|cache| cache.borrow_mut().ask(copy)) {
            Ok(Ask::Answer(key, handler)) => answer(&handler, key),
            Ok(Ask::Wait) => {}
            Ok(Ask::Fetch) => fetch(self.ivars().app.clone()),
            Err(_) => answer(handler, None),
        }
    }
}

/// Fetches from keyd off the main thread, which keyd can keep waiting on a
/// keychain prompt, and settles the cache back on it.
fn fetch(app: AppHandle) {
    elog!("[oculus] webcrypto: WebKit asked for its master key; asking oculus-keyd");
    let spawned = std::thread::Builder::new()
        .name("webcrypto-key".into())
        .spawn(move || {
            let key = std::panic::catch_unwind(super::fetch).unwrap_or_else(|_| {
                elog!("[oculus] webcrypto: the keyd read panicked; WebKit reads its own keychain item");
                None
            });
            if let Err(e) = app.run_on_main_thread(move || settle(key)) {
                elog!("[oculus] webcrypto: could not answer WebKit: {e}");
            }
        });
    if let Err(e) = spawned {
        elog!("[oculus] webcrypto: could not start the keyd read: {e}; WebKit reads its own keychain item");
        settle(None);
    }
}

fn settle(key: Key) {
    let waiting = CACHE
        .try_with(|cache| cache.borrow_mut().settle(key))
        .unwrap_or_default();
    elog!(
        "[oculus] webcrypto: answered {} waiting request(s) {}",
        waiting.len(),
        if key.is_some() {
            "with keyd's key"
        } else {
            "with nothing; WebKit reads its own keychain item"
        }
    );
    for handler in waiting {
        answer(&handler, key);
    }
}

/// Calls the handler once: the 16 bytes, or nil for WebKit's own item.
fn answer(handler: &Handler, key: Key) {
    match key {
        Some(bytes) => {
            let data = NSData::with_bytes(&bytes);
            handler.call((Retained::as_ptr(&data).cast_mut(),));
        }
        None => handler.call((std::ptr::null_mut(),)),
    }
}

/// Makes the process's one delegate `store`'s, so WebKit asks it for the
/// master key. A store without `set_delegate:` (before macOS 15) is left to
/// WebKit's keychain item, logged once. Main thread only.
pub fn install(app: &AppHandle, store: &WKWebsiteDataStore, mtm: MainThreadMarker) {
    let responds: bool = unsafe { msg_send![store, respondsToSelector: sel!(set_delegate:)] };
    if !responds {
        if !UNSUPPORTED_LOGGED.swap(true, Ordering::Relaxed) {
            elog!("[oculus] webcrypto: WebKit has no set_delegate: (macOS 15+); it reads its own keychain item");
        }
        return;
    }
    let delegate = DELEGATE.with(|cell| {
        cell.get_or_init(|| KeyDelegate::new(mtm, app.clone()))
            .clone()
    });
    let _: () = unsafe { msg_send![store, set_delegate: &*delegate] };
    let id = unsafe { store.identifier() }.map(|id| id.UUIDString().to_string());
    elog!(
        "[oculus] webcrypto: store {} asks Oculus for the master key",
        id.as_deref().unwrap_or("default")
    );
}

#[cfg(test)]
mod tests {
    use objc2::ClassType;

    use super::*;

    #[test]
    fn this_webkit_takes_the_delegate_through_set_delegate() {
        // macOS 15+. `_setDelegate:` is not the setter and throws.
        let class = WKWebsiteDataStore::class();
        assert!(class.responds_to(sel!(set_delegate:)));
        assert!(!class.responds_to(sel!(_setDelegate:)));
    }

    #[test]
    fn the_delegate_answers_web_crypto_master_key_with_a_block_argument() {
        let class = KeyDelegate::class();
        let selector = sel!(webCryptoMasterKey:);
        assert!(class.responds_to(selector));
        let method = class.instance_method(selector).expect("the method");
        assert_eq!(method.return_type().to_str(), Ok("v"));
        assert_eq!(method.argument_type(0).unwrap().to_str(), Ok("@"));
        assert_eq!(method.argument_type(1).unwrap().to_str(), Ok(":"));
        assert_eq!(method.argument_type(2).unwrap().to_str(), Ok("@?"));
        assert!(method.argument_type(3).is_none());
    }

    #[test]
    fn a_key_is_answered_as_its_16_bytes_and_none_as_nil() {
        use std::cell::Cell;

        let got = std::rc::Rc::new(Cell::new(None::<Vec<u8>>));
        let calls = std::rc::Rc::new(Cell::new(0));
        let (seen, count) = (got.clone(), calls.clone());
        let handler = RcBlock::new(move |data: *mut NSData| {
            count.set(count.get() + 1);
            seen.set(unsafe { data.as_ref() }.map(|data| data.to_vec()));
        });
        answer(&handler, Some(*b"foobarfoobarfoob"));
        assert_eq!(got.take().as_deref(), Some(&b"foobarfoobarfoob"[..]));
        answer(&handler, None);
        assert_eq!(got.take(), None);
        assert_eq!(calls.get(), 2);
    }
}
