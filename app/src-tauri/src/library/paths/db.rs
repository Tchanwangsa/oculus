use std::path::PathBuf;

pub fn db_path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("oculus.db")
}

/// The database plus the two files SQLite keeps beside it in WAL mode.
///
/// The harness sandboxes' one exception to "nothing outside `agents/` is
/// writable", so `oculus project`/`task` can write: SQLite needs the `-wal` and
/// `-shm` files too, or it reports a readonly database. Files, not the
/// directory, which also holds the session cookie and the Ed token.
pub fn db_write_paths(data_dir: &std::path::Path) -> Vec<PathBuf> {
    let db = db_path(data_dir);
    let sidecar = |suffix: &str| {
        let mut p = db.clone().into_os_string();
        p.push(suffix);
        PathBuf::from(p)
    };
    vec![db.clone(), sidecar("-wal"), sidecar("-shm")]
}
