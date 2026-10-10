use std::fmt;

/// The master key's item in the OS secret store. The label is what an
/// access prompt quotes.
pub const MASTER_SERVICE: &str = "com.tchan.oculus.keyd";
pub const MASTER_ACCOUNT: &str = "master";
pub const MASTER_LABEL: &str = "Oculus keys";

#[derive(Clone)]
pub struct MasterKey(pub(super) [u8; 32]);

impl MasterKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        MasterKey(bytes)
    }

    pub fn generate() -> Result<Self, KeyError> {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes)
            .map_err(|e| KeyError::Platform(format!("getrandom: {e}")))?;
        Ok(MasterKey(bytes))
    }

    /// The secret store holds the key as 64 lowercase hex characters.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.len() != 64 || !text.is_ascii() {
            return None;
        }
        let mut bytes = [0u8; 32];
        for (i, out) in bytes.iter_mut().enumerate() {
            *out = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).ok()?;
        }
        Some(MasterKey(bytes))
    }
}

impl fmt::Debug for MasterKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterKey(..)")
    }
}

/// Why the master key, or an old item, could not be had. `Refused` is the
/// user or the sandbox saying no (a cancelled prompt, no access); `Platform`
/// is anything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    Refused(String),
    Platform(String),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::Refused(d) => write!(f, "the keychain refused the master key: {d}"),
            KeyError::Platform(d) => write!(f, "the keychain failed: {d}"),
        }
    }
}
