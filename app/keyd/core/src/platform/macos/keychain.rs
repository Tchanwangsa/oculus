//! The login keychain: the master key's one item, and plain reads of the
//! items that predate the vault (for import on use).
//!
//! The master key is added with `SecItemAdd` and never updated: if another
//! writer got there first, its key is the one read back and used.

use core_foundation::data::CFData;
use security_framework::item::{ItemAddOptions, ItemAddValue, ItemClass};
use security_framework::passwords::{generic_password, PasswordOptions};

use crate::vault::{KeyError, KeySource, LegacySource, MasterKey, MASTER_ACCOUNT, MASTER_LABEL, MASTER_SERVICE};

// SecBase.h
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
const ERR_SEC_DUPLICATE_ITEM: i32 = -25299;
const ERR_SEC_USER_CANCELED: i32 = -128;
const ERR_SEC_AUTH_FAILED: i32 = -25293;
const ERR_SEC_INTERACTION_NOT_ALLOWED: i32 = -25308;
const ERR_SEC_NO_ACCESS_FOR_ITEM: i32 = -25243;

/// The master key's item: `com.tchan.oculus.keyd` / `master`, labelled
/// "Oculus keys".
pub(crate) struct MasterItem;

impl KeySource for MasterItem {
    fn get_or_create(&self) -> Result<MasterKey, KeyError> {
        if let Some(key) = read_master()? {
            return Ok(key);
        }
        let key = MasterKey::generate()?;
        let mut add = ItemAddOptions::new(ItemAddValue::Data {
            class: ItemClass::generic_password(),
            data: CFData::from_buffer(key.to_hex().as_bytes()),
        });
        add.set_service(MASTER_SERVICE).set_account_name(MASTER_ACCOUNT).set_label(MASTER_LABEL);
        match add.add() {
            Ok(()) => Ok(key),
            Err(e) if e.code() == ERR_SEC_DUPLICATE_ITEM => read_master()?
                .ok_or_else(|| KeyError::Platform("the master key exists but cannot be found".to_string())),
            Err(e) => Err(classify("adding the master key", e.code())),
        }
    }
}

fn read_master() -> Result<Option<MasterKey>, KeyError> {
    match read_password(MASTER_SERVICE, MASTER_ACCOUNT)? {
        None => Ok(None),
        Some(text) => MasterKey::from_hex(&text)
            .map(Some)
            .ok_or_else(|| KeyError::Platform("the master key item does not hold a 256-bit hex key".to_string())),
    }
}

/// The old per-service items, read as they are.
pub(crate) struct LegacyKeychain;

impl LegacySource for LegacyKeychain {
    fn read(&self, service: &str, account: &str) -> Result<Option<String>, KeyError> {
        read_password(service, account)
    }
}

/// A generic password as UTF-8. `Ok(None)` only when no such item exists.
fn read_password(service: &str, account: &str) -> Result<Option<String>, KeyError> {
    match generic_password(PasswordOptions::new_generic_password(service, account)) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| KeyError::Platform(format!("{service}/{account} is not UTF-8"))),
        Err(e) if e.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(None),
        Err(e) => Err(classify(&format!("reading {service}/{account}"), e.code())),
    }
}

fn classify(step: &str, code: i32) -> KeyError {
    let detail = format!("{step}: OSStatus {code}");
    match code {
        ERR_SEC_USER_CANCELED | ERR_SEC_AUTH_FAILED | ERR_SEC_INTERACTION_NOT_ALLOWED | ERR_SEC_NO_ACCESS_FOR_ITEM => {
            KeyError::Refused(detail)
        }
        _ => KeyError::Platform(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_are_told_apart_from_failures() {
        for code in [-128, -25293, -25308, -25243] {
            assert!(matches!(classify("x", code), KeyError::Refused(_)), "{code}");
        }
        assert!(matches!(classify("x", -25291), KeyError::Platform(_)));
    }
}
