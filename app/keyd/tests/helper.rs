//! The helper app's Info.plist (`bundle/Info.plist`, which keyd-build.mjs
//! puts in `Oculus Helper.app`) agrees with the names keyd and its installer
//! use, so a keychain prompt names the helper and launchd's label is its
//! bundle identifier.

use keyd_core::paths;

const PLIST: &str = include_str!("../bundle/Info.plist");

/// The `<string>` (or `<true/>`) after `<key>name</key>`. Scanned, not parsed.
fn value(key: &str) -> Option<&'static str> {
    let after = PLIST
        .split(&format!("<key>{key}</key>"))
        .nth(1)?
        .trim_start();
    if after.starts_with("<true/>") {
        return Some("true");
    }
    let rest = after.strip_prefix("<string>")?;
    Some(&rest[..rest.find("</string>")?])
}

#[test]
fn the_helper_is_named_for_the_prompt_and_runs_keyd() {
    assert_eq!(value("CFBundleName"), Some(paths::HELPER));
    assert_eq!(value("CFBundleDisplayName"), Some(paths::HELPER));
    assert_eq!(value("CFBundleExecutable"), Some(paths::BINARY));
    assert_eq!(value("CFBundlePackageType"), Some("APPL"));
    assert_eq!(value("CFBundleIconFile"), Some("icon.icns"));
}

#[test]
fn its_identifier_is_keyds_signing_identifier_and_launchd_label() {
    assert_eq!(
        value("CFBundleIdentifier"),
        Some(format!("{}.keyd", paths::IDENTIFIER).as_str())
    );
}

#[test]
fn its_version_is_keyds() {
    assert_eq!(
        value("CFBundleShortVersionString"),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(value("CFBundleVersion"), Some(env!("CARGO_PKG_VERSION")));
}

/// Never a Dock icon or a menu bar: keyd draws nothing, and launchd starts
/// it on a socket connect, not LaunchServices.
#[test]
fn it_is_background_only() {
    assert_eq!(value("LSBackgroundOnly"), Some("true"));
    assert_eq!(value("LSUIElement"), None);
}
