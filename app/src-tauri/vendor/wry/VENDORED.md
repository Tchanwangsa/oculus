# Vendored wry

wry 0.55.1 as published on crates.io (upstream commit
`a5bf203a1c8dbb3583588382538d6521655222a8`), patched in through
`[patch.crates-io]` in `app/src-tauri/Cargo.toml`.

Left out of the copy: `CHANGELOG.md`, `Cargo.lock`, `Cargo.toml.orig`,
`MOBILE.md`, `SECURITY.md`, `renovate.json`, `.cargo/`, `.gitignore` and
`.license_template`.

## Changes from upstream

None.

## Updating

The version here must equal the one `tauri-runtime-wry` resolves, or cargo
drops the patch without an error. After a tauri bump, copy the new wry
release over this folder, reapply the changes listed above, and check that
`cargo tree -i wry` prints this path.
