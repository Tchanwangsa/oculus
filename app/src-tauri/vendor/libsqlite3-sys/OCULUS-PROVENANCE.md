# Oculus SQLite engine patch

The crate remains `libsqlite3-sys` **0.30.1**, matching SQLx 0.8.6 and
`tauri-plugin-sql` 2.4.0. Oculus's Cargo patch makes the frontend SQL plugin,
native backend pools and bundled CLI link the same static SQLite engine.

The unmodified crate source was taken from the Cargo registry distribution of
[`libsqlite3-sys` 0.30.1](https://crates.io/crates/libsqlite3-sys/0.30.1).
Its MIT license is retained in `LICENSE`; SQLite is public domain. Cargo cache
markers were omitted. No build flags or Rust API declarations were changed.

The original bundled SQLite 3.46.0 predates the
[WAL-reset corruption fix](https://sqlite.org/wal.html#the_wal_reset_bug),
released in SQLite 3.51.3. This patch replaces only the SQLite amalgamation
files with **3.53.4**, the official release verified on 2026-09-19:

- Source: [sqlite-amalgamation-3530400.zip](https://sqlite.org/2026/sqlite-amalgamation-3530400.zip)
- Archive SHA3-256: `628a44cfe82c66aed1ccbbe85a562d2e33ebe64b3288981ed76285612227934e`
- Published checksum: [SQLite downloads](https://sqlite.org/download.html)
- Source ID: `2026-07-24 19:02:57 bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e09459bcc`

| Replaced file | SHA-256 |
| --- | --- |
| `sqlite3/sqlite3.c` | `b1dd5d74ec7f29055a6684fa06fb3c2f6821c87dd38f9a458dfd2e8a1db28189` |
| `sqlite3/sqlite3.h` | `919e7f2e8ed1d8f56ac17b412b8971c76aa5d1a879752cc6058f75e7d5910e1d` |
| `sqlite3/sqlite3ext.h` | `ac9645e5c9ff0cf176efdd6e75cb5e98f46295d38e02db5c4d208826a39ab4be` |

The existing generated bindings intentionally retain the 0.30.1 API surface.
Only `SQLITE_VERSION`, `SQLITE_VERSION_NUMBER`, and `SQLITE_SOURCE_ID` were
updated in `sqlite3/bindgen_bundled_version.rs` and its `_ext.rs` companion.
SQLite guarantees backward compatibility for the
[3.x C interface](https://sqlite.org/versionnumbers.html); no new C APIs are
needed by Oculus. This avoids introducing build-time libclang or changing
SQLx. Unused SQLCipher sources remain as supplied upstream.

The existing `cc` build retains its thread safety, column metadata,
foreign-key defaults, FTS, JSON, and `unlock_notify` feature configuration.
No `SQLITE3_*` environment override or separately packaged SQLite DLL is
required. The original upstream `README.md` and `upgrade.sh` describe the
unpatched 0.30.1 crate; this file records the Oculus-specific engine change.

Validation from `app/src-tauri`:

```text
cargo test --release --lib database:: -- --nocapture
```

The runtime regression checks both `sqlite_version()` and
`sqlite_source_id()`, so a build that accidentally links the old engine
fails. The other database tests check plugin/native alias identity, WAL
sharing, integrity after writes, and preservation of existing database bytes.
