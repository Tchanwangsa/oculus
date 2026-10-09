//! Where keyd's files live. The data dir is defined here once, and both keyd
//! and the app's `paths::data_dir` use it. What sits in it is named here too;
//! where the OS keeps its own registration is the registrar's business.

use std::path::{Path, PathBuf};

/// The app's bundle identifier (`identifier` in tauri.conf.json; the app's
/// tests hold the two together).
pub const IDENTIFIER: &str = "com.tchan.oculus";

/// keyd's executable name, in a bundle and in `bin/`.
pub const BINARY: &str = "oculus-keyd";

const STAMP: &str = "oculus-keyd.stamp";

/// The OS's per-user data dir plus the identifier: Tauri's `app_data_dir()`.
pub fn data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join(IDENTIFIER)
}

/// keyd's endpoint. Short on purpose: a macOS `sun_path` holds 104 bytes.
pub fn socket(data_dir: &Path) -> PathBuf {
    data_dir.join("keyd.sock")
}

/// Every secret, sealed under the master key only keyd reads.
pub fn vault(data_dir: &Path) -> PathBuf {
    data_dir.join("vault.bin")
}

/// Where an install that does not run in place puts keyd, and its stamp.
pub fn bin_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("bin")
}

pub fn installed_bin(data_dir: &Path) -> PathBuf {
    bin_dir(data_dir).join(BINARY)
}

/// The source hash of the keyd the agent runs, written after a good install.
pub fn stamp(data_dir: &Path) -> PathBuf {
    bin_dir(data_dir).join(STAMP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_sits_in_the_data_dir() {
        let d = Path::new("/d");
        assert_eq!(socket(d), Path::new("/d/keyd.sock"));
        assert_eq!(vault(d), Path::new("/d/vault.bin"));
        assert_eq!(installed_bin(d), Path::new("/d/bin/oculus-keyd"));
        assert_eq!(stamp(d), Path::new("/d/bin/oculus-keyd.stamp"));
        assert!(data_dir().ends_with(IDENTIFIER));
    }
}
