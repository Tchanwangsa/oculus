/// The master key's item: read, or created on first use.
#[cfg(feature = "server")]
pub fn master_key() -> Box<dyn crate::vault::KeySource> {
    super::imp::master_key()
}

/// The per-service items that predate the vault, for import on use.
#[cfg(feature = "server")]
pub fn legacy_items() -> Box<dyn crate::vault::LegacySource> {
    super::imp::legacy_items()
}
