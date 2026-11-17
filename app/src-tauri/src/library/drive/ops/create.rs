//! `create_folder`: one new, empty folder; a taken name is an `exists`
//! error, never renamed (the browser picks a free name first).

use std::path::Path;

use super::error::{shown, DriveError, DriveResult};
use super::guard::{parent_rel, target_folder};
use super::names::check_name;

pub fn create_folder_at(data_dir: &Path, rel: &str) -> DriveResult<String> {
    let parent = target_folder(data_dir, parent_rel(rel))?;
    check_name(shown(rel), parent.is_subject_root())?;
    match std::fs::create_dir(data_dir.join(rel)) {
        Ok(()) => Ok(rel.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(DriveError::exists(rel)),
        Err(e) => Err(e.into()),
    }
}
