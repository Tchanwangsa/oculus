//! One filename for SQLite and all of its journal/locking companions.
//!
//! MSIX filesystem virtualization can expose the same database through a
//! logical AppData path and a physical package path while putting their WAL
//! files in different directories. Resolve the database *file*, not its parent:
//! the logical library directory can also contain courses absent from the
//! physical database directory. Library content paths must remain unchanged.

use std::path::{Path, PathBuf};

/// Resolve an existing SQLite file before opening any connection to it.
/// Windows callers must fail closed if the database cannot be resolved.
pub fn resolve_path(path: &Path) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let physical = dunce::canonicalize(path)
            .map_err(|error| format!("cannot resolve database {}: {error}", path.display()))?;
        if !physical.is_file() {
            return Err(format!("database is not a file: {}", physical.display()));
        }
        check_companions(path, &physical)?;
        Ok(physical)
    }
    #[cfg(not(windows))]
    Ok(path.to_path_buf())
}

#[cfg(windows)]
fn companion(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(windows)]
fn check_companions(logical: &Path, physical: &Path) -> Result<(), String> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let source = companion(logical, suffix);
        let existing = match dunce::canonicalize(&source) {
            Ok(existing) => existing,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!("cannot resolve database companion {}: {error}", source.display()));
            }
        };
        let destination = companion(physical, suffix);
        if dunce::canonicalize(&destination).ok().as_ref() != Some(&existing) {
            return Err(format!(
                "Database journal path mismatch: {} belongs to a different location than {}. \
                 Close Oculus and recover the database together with its journal before reopening.",
                source.display(), physical.display()
            ));
        }
    }
    Ok(())
}

/// URL shared by tauri-plugin-sql's migration registration and Database.load.
/// Create only a missing, empty database file so Windows can resolve its actual
/// storage location before SQLite creates journals. Existing bytes are never
/// truncated or copied. Native/headless callers use resolve_path instead and
/// continue to refuse missing databases.
pub fn plugin_url(path: &Path) -> Result<String, String> {
    let parent = path.parent().ok_or("database has no parent directory")?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create database directory {}: {error}", parent.display()))?;
    match std::fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("cannot create database {}: {error}", path.display())),
    }
    let resolved = resolve_path(path)?;
    sqlite_url(&resolved)
}

fn sqlite_url(path: &Path) -> Result<String, String> {
    let path = path.to_str().ok_or("database path is not valid UTF-8")?;
    // SQLx percent-decodes filenames. Encode literal '%' and URL punctuation
    // without encoding the drive/separators: plugin-sql must still recognise
    // this as an absolute filename when it joins its app_config_dir prefix.
    let mut encoded = String::from("sqlite:");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~:/\\".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(encoded, "%{byte:02X}").expect("write to string");
        }
    }
    Ok(encoded)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use sqlx::{Connection, Row};
    use std::str::FromStr;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!(
                "oculus-database-{}-{nonce} 100% 学生", std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn junction(&self, name: &str, target: &Path) -> PathBuf {
            let link = self.0.join(name);
            let quote = |path: &Path| path.to_string_lossy().replace('\'', "''");
            let script = format!(
                "$ErrorActionPreference='Stop'; $null = New-Item -ItemType Junction -Path '{}' -Target '{}'",
                quote(&link), quote(target)
            );
            let status = crate::platform::command("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .status().unwrap();
            assert!(status.success());
            link
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Junctions are explicitly removed in the tests before recursive
            // cleanup. The target is this unique temporary fixture only.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_database_is_not_created_by_native_readers() {
        let fixture = Fixture::new();
        let path = fixture.0.join("missing.db");
        assert!(resolve_path(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn plugin_url_preserves_existing_bytes_and_decodes_special_names() {
        let fixture = Fixture::new();
        let path = fixture.0.join("database #100%.db");
        std::fs::write(&path, b"existing bytes").unwrap();
        let connection = plugin_url(&path).unwrap();
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(&connection).unwrap();
        assert_eq!(options.get_filename(), resolve_path(&path).unwrap());
        assert_eq!(std::fs::read(path).unwrap(), b"existing bytes");
        // This is exactly the plugin's path_mapper operation. The absolute
        // URL path must replace the prefix rather than be appended to it.
        let mut mapped = PathBuf::from(r"C:\unrelated\config");
        mapped.push(connection.split_once(':').unwrap().1);
        let mapped = format!("sqlite:{}", mapped.display());
        let options = sqlx::sqlite::SqliteConnectOptions::from_str(&mapped).unwrap();
        assert_eq!(options.get_filename(), resolve_path(&fixture.0.join("database #100%.db")).unwrap());
    }

    #[tokio::test]
    async fn plugin_and_native_aliases_share_database_and_wal() {
        let fixture = Fixture::new();
        let actual = fixture.0.join("physical");
        std::fs::create_dir(&actual).unwrap();
        let alias = fixture.junction("logical", &actual);
        let database = actual.join("oculus.db");
        let logical = alias.join("oculus.db");
        let url = plugin_url(&logical).unwrap();
        let mut plugin = sqlx::SqliteConnection::connect(&url).await.unwrap();
        sqlx::query("PRAGMA journal_mode=WAL").execute(&mut plugin).await.unwrap();
        sqlx::query("CREATE TABLE state (id INTEGER PRIMARY KEY, source TEXT)")
            .execute(&mut plugin).await.unwrap();
        let native = crate::retrieval::pool(&logical).await.unwrap();
        let cli = crate::retrieval::pool(&database).await.unwrap();
        let plugin_file: String = sqlx::query("PRAGMA database_list")
            .fetch_one(&mut plugin).await.unwrap().get("file");
        let native_file: String = sqlx::query("PRAGMA database_list")
            .fetch_one(&native).await.unwrap().get("file");
        let cli_file: String = sqlx::query("PRAGMA database_list")
            .fetch_one(&cli).await.unwrap().get("file");
        assert_eq!(PathBuf::from(plugin_file), PathBuf::from(native_file));
        assert_eq!(PathBuf::from(cli_file), resolve_path(&database).unwrap());
        for id in 0..50 {
            sqlx::query("INSERT INTO state VALUES (?1, 'plugin')")
                .bind(id * 2).execute(&mut plugin).await.unwrap();
            sqlx::query("INSERT INTO state VALUES (?1, 'native')")
                .bind(id * 2 + 1).execute(&native).await.unwrap();
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM state")
            .fetch_one(&cli).await.unwrap();
        assert_eq!(count, 100);
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&cli).await.unwrap();
        assert_eq!(integrity, "ok");
        assert_eq!(
            dunce::canonicalize(companion(&logical, "-wal")).unwrap(),
            dunce::canonicalize(companion(&database, "-wal")).unwrap()
        );
        native.close().await;
        cli.close().await;
        plugin.close().await.unwrap();
        std::fs::remove_dir(alias).unwrap();
    }

    #[test]
    fn a_legacy_journal_in_another_location_is_rejected() {
        let fixture = Fixture::new();
        let actual = fixture.0.join("physical.db");
        let alias = fixture.0.join("logical.db");
        std::fs::write(&actual, b"database").unwrap();
        // The split-name companion layout is what MSIX virtualization can
        // expose. Validate this guard independently of the OS redirector.
        std::fs::write(companion(&alias, "-wal"), b"uncheckpointed data").unwrap();
        let error = check_companions(&alias, &actual).unwrap_err();
        assert!(error.contains("journal path mismatch"));
        assert_eq!(std::fs::read(companion(&alias, "-wal")).unwrap(), b"uncheckpointed data");
        assert!(!companion(&actual, "-wal").exists());
    }
}

#[cfg(test)]
mod shared_engine_tests {
    use sqlx::Connection;

    // Both native platforms must link the same patched engine: the SQL plugin,
    // CLI, migration tests and background workers all depend on its WAL rules.
    #[tokio::test]
    async fn bundled_engine_contains_the_wal_reset_fix() {
        let mut database = sqlx::SqliteConnection::connect("sqlite::memory:").await.unwrap();
        let version: String = sqlx::query_scalar("SELECT sqlite_version()")
            .fetch_one(&mut database).await.unwrap();
        let source_id: String = sqlx::query_scalar("SELECT sqlite_source_id()")
            .fetch_one(&mut database).await.unwrap();
        assert_eq!(version, "3.53.4", "Do not ship the old vulnerable SQLite engine");
        assert_eq!(source_id,
            "2026-07-24 19:02:57 bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e09459bcc");
        database.close().await.unwrap();
    }
}
