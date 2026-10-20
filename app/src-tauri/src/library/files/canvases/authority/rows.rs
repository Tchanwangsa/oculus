//! The `files` row follows each persisted change: size and modified time after
//! a commit, the path after a rename, the row after a delete. Writes go to one
//! background thread in commit order and never block or fail a commit; the
//! file on disk is the truth, so a failed row update is only logged.

use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;

pub trait FileRows: Send + Sync {
    fn touched(&self, file_id: i64, size_bytes: u64);
    fn renamed(&self, file_id: i64, filename: &str, relative_path: &str);
    fn deleted(&self, file_id: i64);
}

enum Op {
    Touch(i64, u64),
    Rename(i64, String, String),
    Delete(i64),
}

/// The shared database through `crate::db::store`.
pub struct DbRows;

impl DbRows {
    fn send(&self, op: Op) {
        static WRITER: OnceLock<Option<Sender<Op>>> = OnceLock::new();
        let writer = WRITER.get_or_init(|| {
            let (tx, rx) = channel::<Op>();
            let spawned = std::thread::Builder::new()
                .name("canvas-file-rows".into())
                .spawn(move || run(rx));
            match spawned {
                Ok(_) => Some(tx),
                Err(error) => {
                    elog!("[canvas-authority] file row writer did not start: {error}");
                    None
                }
            }
        });
        if let Some(writer) = writer {
            writer.send(op).ok();
        }
    }
}

impl FileRows for DbRows {
    fn touched(&self, file_id: i64, size_bytes: u64) {
        self.send(Op::Touch(file_id, size_bytes));
    }

    fn renamed(&self, file_id: i64, filename: &str, relative_path: &str) {
        self.send(Op::Rename(file_id, filename.into(), relative_path.into()));
    }

    fn deleted(&self, file_id: i64) {
        self.send(Op::Delete(file_id));
    }
}

fn run(ops: std::sync::mpsc::Receiver<Op>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            elog!("[canvas-authority] file row writer has no runtime: {error}");
            return;
        }
    };
    let mut pool = None;
    for op in ops {
        runtime.block_on(async {
            if pool.is_none() {
                pool = crate::db::store::open_pool()
                    .await
                    .map_err(|error| elog!("[canvas-authority] file rows: {error}"))
                    .ok();
            }
            let Some(pool) = pool.as_ref() else {
                return;
            };
            let (id, written) = match &op {
                Op::Touch(id, size) => (
                    *id,
                    crate::db::store::touch_file_row(pool, *id, *size, true).await,
                ),
                Op::Rename(id, filename, rel) => (
                    *id,
                    crate::db::store::rename_file_row(pool, *id, filename, rel).await,
                ),
                Op::Delete(id) => (*id, crate::db::store::delete_file_row(pool, *id).await),
            };
            if let Err(error) = written {
                elog!("[canvas-authority] file row {id}: {error}");
            }
        });
    }
}
