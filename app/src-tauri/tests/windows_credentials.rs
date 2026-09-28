//! Uses an isolated synthetic credential; never opens a user's app credential.
#![cfg(windows)]

struct Cleanup(keyring::Entry);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = self.0.delete_credential();
    }
}

#[test]
fn windows_credential_manager_persists_and_deletes_an_isolated_entry() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let service = format!("com.tchan.oculus.test.{}.{stamp}", std::process::id());
    let account = "synthetic-integration-test";
    let entry = Cleanup(keyring::Entry::new(&service, account).unwrap());
    assert!(matches!(entry.0.get_password(), Err(keyring::Error::NoEntry)));
    entry.0.set_password("synthetic-test-value-only").unwrap();
    // A new handle proves this was saved to the native credential store,
    // rather than retained on one Entry object by a mock backend.
    let second = keyring::Entry::new(&service, account).unwrap();
    assert_eq!(second.get_password().unwrap(), "synthetic-test-value-only");
    entry.0.delete_credential().unwrap();
    assert!(matches!(second.get_password(), Err(keyring::Error::NoEntry)));
}
