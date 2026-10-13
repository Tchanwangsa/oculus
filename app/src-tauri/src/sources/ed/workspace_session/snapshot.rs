//! The commit Run and Mark take: a fresh `commit {id}` on the live session,
//! answered by the `new_commit` whose `context.id` echoes it, as Ed's web app
//! does. Run and Mark want the whole `<tree>:::<suffix>` id; `init.last_commits`
//! names only the tree half, which Ed's run and mark answer with a 500.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

use serde_json::{json, Value};

use super::context::Context;
use super::socket::Out;

/// Commits asked of the server and not yet answered.
#[derive(Default)]
pub struct Snapshots {
    next: u64,
    waiting: HashMap<u64, Sender<String>>,
}

impl Snapshots {
    /// A new commit id and where its hash will arrive.
    pub fn ask(&mut self) -> (u64, Receiver<String>) {
        self.next += 1;
        let (tx, rx) = mpsc::channel();
        self.waiting.insert(self.next, tx);
        (self.next, rx)
    }

    /// A `new_commit` arrived: true when its context answers one we asked.
    pub fn answered(&mut self, context: Option<&Value>, hash: &str) -> bool {
        let Some(id) = context.and_then(|c| context_id(&c["id"])) else {
            return false;
        };
        match self.waiting.remove(&id) {
            Some(tx) => {
                let _ = tx.send(hash.to_string());
                true
            }
            None => false,
        }
    }

    /// The session went offline or ended: nothing asked will be answered.
    pub fn fail_all(&mut self) {
        self.waiting.clear();
    }
}

fn context_id(id: &Value) -> Option<u64> {
    id.as_u64()
        .or_else(|| id.as_str().and_then(|s| s.parse().ok()))
}

/// Snapshot the workspace as a commit.
pub fn commit_frame(id: u64) -> String {
    json!({ "type": "commit", "data": { "id": id } }).to_string()
}

/// The hash Run and Mark send: a commit asked of the live session. Waits for
/// no longer than the session stays online.
pub fn latest(core: &Context, out: &Sender<Out>) -> Result<String, String> {
    let gone = || "the workspace went offline before Ed saved a commit of it".to_string();
    let answer = {
        let mut shared = core.shared();
        if shared.online.is_none() {
            return Err(gone());
        }
        let (id, answer) = shared.snapshots.ask();
        out.send(Out::Frame(commit_frame(id))).map_err(|_| gone())?;
        answer
    };
    answer.recv().map_err(|_| gone())
}
