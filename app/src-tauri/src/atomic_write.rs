//! Replace a file so a reader sees the old contents or the new, never a torn
//! write.

use std::fs;
use std::io::Write;
use std::path::Path;

/// Write `body` to `tmp`, fsync, and rename it over `path`. `tmp` must be on
/// the same filesystem; it is removed on any failure.
pub fn write(path: &Path, tmp: &Path, body: &[u8]) -> Result<(), String> {
    let staged = fs::File::create(tmp)
        .map_err(|e| format!("create {}: {e}", tmp.display()))
        .and_then(|mut file| {
            file.write_all(body)
                .and_then(|()| file.sync_all())
                .map_err(|e| format!("write {}: {e}", tmp.display()))
        });
    if let Err(error) = staged {
        fs::remove_file(tmp).ok();
        return Err(error);
    }
    fs::rename(tmp, path).map_err(|e| {
        fs::remove_file(tmp).ok();
        format!("rename into {}: {e}", path.display())
    })
}
