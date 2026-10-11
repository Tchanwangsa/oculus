use std::path::Path;
use std::sync::{Arc, Mutex};

use super::instructions::thread_cwd;

/// Append-only file of raw provider lines for one thread.
#[derive(Clone)]
pub struct RawLog(Arc<Mutex<std::fs::File>>);

impl RawLog {
    pub fn open(data_dir: &Path, thread_id: i64) -> Option<RawLog> {
        let dir = thread_cwd(data_dir).join("threads");
        std::fs::create_dir_all(&dir).ok()?;
        let f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{thread_id}.ndjson")))
            .ok()?;
        Some(RawLog(Arc::new(Mutex::new(f))))
    }

    pub fn write(&self, line: &str) {
        use std::io::Write;
        if let Ok(mut f) = self.0.lock() {
            let _ = f
                .write_all(line.as_bytes())
                .and_then(|_| f.write_all(b"\n"));
        }
    }
}
