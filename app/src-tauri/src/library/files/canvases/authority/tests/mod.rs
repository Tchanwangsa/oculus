mod fixtures;
mod format;
mod sessions;

use super::rows::FileRows;
use std::sync::Mutex;

/// Records file-row updates instead of writing the database.
#[derive(Default)]
pub(super) struct Rows(pub Mutex<Vec<String>>);

impl Rows {
    pub fn log(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl FileRows for Rows {
    fn touched(&self, file_id: i64, size_bytes: u64) {
        self.0
            .lock()
            .unwrap()
            .push(format!("touch {file_id} {size_bytes}"));
    }

    fn renamed(&self, file_id: i64, filename: &str, relative_path: &str) {
        self.0
            .lock()
            .unwrap()
            .push(format!("rename {file_id} {filename} {relative_path}"));
    }

    fn deleted(&self, file_id: i64) {
        self.0.lock().unwrap().push(format!("delete {file_id}"));
    }
}
