//! One reconcile of a workspace folder with Ed: every file here, in the
//! record and on Ed goes through the decision table (`table`), and what it
//! decides is done. The record is not saved here: the caller saves the
//! [`Outcome`]. Each step leaves the sides so that a second run decides the
//! same or `Record`, so a run cut short is finished by the next one.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::server_copy;
use super::table::{decide, Action};
use crate::sources::ed::unpack_workspace;
use crate::sources::ed::workspace_session::context::{sha256, write_file};
use crate::sources::ed::workspace_session::paths;
use crate::sources::ed::workspace_state::WorkspaceState;

/// Ed's side of a reconcile.
pub trait Remote {
    /// The whole tree as a zip (`download "."`).
    fn tree(&mut self) -> Result<Vec<u8>, String>;
    /// Upload one file; the answer is Ed's commit.
    fn upload(&mut self, path: &str, bytes: &[u8]) -> Result<String, String>;
    /// Remove one file on Ed. The record keeps the file until a later
    /// reconcile finds it gone from both sides, so a remove that is lost on
    /// the way is sent again.
    fn remove(&mut self, path: &str) -> Result<(), String>;
}

/// What the server is known to hold.
#[derive(Debug, Clone, Copy)]
pub struct Head<'a> {
    /// Ed's newest commit, the new base when nothing is uploaded.
    pub hash: Option<&'a str>,
    /// Ed changed since the base, whatever `hash` says: the tree is
    /// downloaded even when `hash` is the base commit.
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub path: String,
    /// Ed's version, workspace-relative (`server_copy::name`).
    pub server_path: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The record's new `files`.
    pub files: BTreeMap<String, String>,
    pub base_commit: Option<String>,
    pub conflicts: Vec<Conflict>,
    /// Files written or deleted here.
    pub changed_here: BTreeSet<String>,
    /// One sentence per file of Ed's left out, for the log.
    pub skipped: Vec<String>,
}

pub fn run(
    root: &Path,
    record: &WorkspaceState,
    head: Head,
    remote: &mut dyn Remote,
) -> Result<Outcome, String> {
    if !root.is_dir() {
        return Err("the workspace folder is missing from the library".into());
    }
    let local = read_local(root)?;
    let mut out = Outcome {
        files: record.files.clone(),
        ..Outcome::default()
    };
    let unchanged_on_ed =
        !head.changed && head.hash.is_some() && head.hash == record.base_commit.as_deref();
    // With Ed unchanged since the base, Ed holds the base: no download.
    let Server {
        shas: server,
        bytes: server_bytes,
        unknown,
        skipped,
    } = if unchanged_on_ed {
        Server {
            shas: record.files.clone(),
            ..Server::default()
        }
    } else {
        read_server(root, &remote.tree()?)?
    };
    out.skipped = skipped;

    let mut every: BTreeSet<&String> = local.keys().collect();
    every.extend(record.files.keys());
    every.extend(server.keys());
    let mut last_upload = None;
    for path in every {
        // Ed's copy was left out of the zip: its side is unknown.
        if unknown.contains(path) {
            continue;
        }
        let mine = local.get(path).map(|(sha, _)| sha.as_str());
        let base = record.files.get(path).map(String::as_str);
        let theirs = server.get(path).map(String::as_str);
        match decide(mine, base, theirs) {
            Action::Nothing => {}
            Action::Record => agree(&mut out.files, path, mine),
            Action::Take => {
                let Some(bytes) = server_bytes.get(path) else {
                    continue;
                };
                write_file(&paths::resolve(root, path)?, bytes)?;
                agree(&mut out.files, path, theirs);
                out.changed_here.insert(path.clone());
            }
            Action::Push => {
                // A file waiting on its conflict goes up only when resolved.
                let copy = paths::resolve(root, &server_copy::name(path))?;
                if copy.exists() {
                    continue;
                }
                let Some((sha, bytes)) = local.get(path) else {
                    continue;
                };
                last_upload = Some(remote.upload(path, bytes)?);
                out.files.insert(path.clone(), sha.clone());
            }
            Action::Remove => remote.remove(path)?,
            Action::DeleteLocal => {
                match std::fs::remove_file(paths::resolve(root, path)?) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("could not delete {path}: {e}")),
                }
                out.files.remove(path);
                out.changed_here.insert(path.clone());
            }
            Action::Conflict => {
                let Some(bytes) = server_bytes.get(path) else {
                    continue;
                };
                let server_path = server_copy::name(path);
                let copy = paths::resolve(root, &server_path)?;
                if std::fs::read(&copy).ok().as_deref() != Some(bytes.as_slice()) {
                    write_file(&copy, bytes)?;
                    out.changed_here.insert(server_path.clone());
                }
                out.conflicts.push(Conflict {
                    path: path.clone(),
                    server_path,
                });
            }
        }
    }
    out.base_commit = last_upload
        .or_else(|| head.hash.map(str::to_string))
        .or_else(|| record.base_commit.clone());
    Ok(out)
}

/// Set the record to what both sides hold.
fn agree(files: &mut BTreeMap<String, String>, path: &str, sha: Option<&str>) {
    match sha {
        Some(sha) => files.insert(path.to_string(), sha.to_string()),
        None => files.remove(path),
    };
}

/// Every file here but the server copies: path → (sha, bytes).
fn read_local(root: &Path) -> Result<BTreeMap<String, (String, Vec<u8>)>, String> {
    let mut local = BTreeMap::new();
    for path in paths::list(root) {
        if server_copy::is_copy(&path) {
            continue;
        }
        let bytes =
            std::fs::read(root.join(&path)).map_err(|e| format!("could not read {path}: {e}"))?;
        local.insert(path, (sha256(&bytes), bytes));
    }
    Ok(local)
}

#[derive(Default)]
struct Server {
    shas: BTreeMap<String, String>,
    bytes: BTreeMap<String, Vec<u8>>,
    /// Files Ed has that the zip left out (too big, an unsafe name).
    unknown: BTreeSet<String>,
    skipped: Vec<String>,
}

/// Ed's tree. A path the library cannot hold (`paths::resolve`) or one
/// shaped like a server copy is left out.
fn read_server(root: &Path, zip: &[u8]) -> Result<Server, String> {
    let (files, unpack_skipped) = unpack_workspace(zip)?;
    let mut server = Server::default();
    for skip in unpack_skipped {
        server.skipped.push(skip.to_string());
        server.unknown.insert(skip.path);
    }
    for file in files {
        if server_copy::is_copy(&file.path) {
            server.skipped.push(format!(
                "{}: named like Oculus's copy of a conflict, skipped",
                file.path
            ));
            continue;
        }
        if let Err(e) = paths::resolve(root, &file.path) {
            server.skipped.push(format!("{e}, skipped"));
            server.unknown.insert(file.path);
            continue;
        }
        server.shas.insert(file.path.clone(), sha256(&file.bytes));
        server.bytes.insert(file.path, file.bytes);
    }
    Ok(server)
}
