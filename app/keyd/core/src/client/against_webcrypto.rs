//! `webcrypto_key` from the client, through the real server loop and ops in
//! memory, as the peer check names each caller.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::ops::State;
use crate::platform::{memory, Role};
use crate::server::Server;
use crate::test_support::{OldItems, Peers, Reads, Scratch, BUILD};
use crate::vault::{MasterKey, StaticKey};

const WEBKIT_DEV: &str = "Zm9vYmFyZm9vYmFyZm9vYg==";

/// A keyd over `dir` whose WebKit items are `items`, admitting its callers
/// as `peers` says.
fn serve(peers: Peers, dir: &Scratch, items: OldItems) -> Client {
    let state = State::new(
        BUILD,
        dir.0.clone(),
        Box::new(StaticKey(MasterKey::from_bytes([7; 32]))),
        Box::new(items),
    );
    let (listener, connector) = memory::listener();
    let server = Arc::new(Server::new(state, Box::new(peers), Duration::from_secs(60)));
    std::thread::spawn(move || server.run(&[listener]));
    Client {
        endpoint: PathBuf::from("memory"),
        connect: Arc::new(move || Ok(connector.connect())),
    }
}

fn webkit(account: &'static str, text: &str) -> OldItems {
    OldItems::new(vec![], Reads::default()).webkit_item(account, Ok(Some(text.into())))
}

#[test]
fn the_dev_app_gets_webkits_key_then_the_vaults() {
    let dir = Scratch::new("client-webcrypto");
    let items = webkit("com.apple.WebKit.WebCrypto.master+app", WEBKIT_DEV);
    let reads = items.webkit_reads();
    let keyd = serve(Peers::admit_app("app"), &dir, items);
    let first = keyd.webcrypto_key("app").unwrap();
    assert_eq!(first.key.bytes(), b"foobarfoobarfoob");
    assert_eq!(first.source, "keychain");
    let again = keyd.webcrypto_key("app").unwrap();
    assert_eq!((again.key, again.source.as_str()), (first.key, "vault"));
    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn only_the_app_that_is_that_application_is_served() {
    let dir = Scratch::new("client-webcrypto-callers");
    let items = || webkit("com.apple.WebKit.WebCrypto.master+app", WEBKIT_DEV);
    for peers in [
        Peers::admit_as(Role::Cli),
        Peers::admit_as(Role::Unknown),
        Peers::admit_as(Role::App),
        Peers::admit_app("com.tchan.oculus"),
    ] {
        let keyd = serve(peers, &dir, items());
        assert!(matches!(
            keyd.webcrypto_key("app"),
            Err(KeydError::Caller(_))
        ));
    }
    assert!(!crate::paths::vault(&dir.0).exists(), "nothing was stored");
}

#[test]
fn a_refused_read_is_a_keychain_error_and_no_key() {
    let dir = Scratch::new("client-webcrypto-refused");
    let items = OldItems::new(vec![], Reads::default()).webkit_item(
        "com.apple.WebKit.WebCrypto.master+com.tchan.oculus",
        Err(crate::vault::KeyError::Refused("OSStatus -128".into())),
    );
    let keyd = serve(Peers::admit_app("com.tchan.oculus"), &dir, items);
    assert_eq!(
        keyd.webcrypto_key("com.tchan.oculus").unwrap_err(),
        KeydError::Keychain("OSStatus -128".into())
    );
}
