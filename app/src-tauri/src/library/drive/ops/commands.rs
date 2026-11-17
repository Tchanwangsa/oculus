//! The folder commands the browser calls (`app/src/lib/drive/commands.ts`).
//! Each refuses with a JSON-serialised `DriveError`, and announces what it
//! did on `drive-changed`.

use sqlx::SqlitePool;
use tauri::AppHandle;

use super::copy::copy_to_own_at;
use super::create::create_folder_at;
use super::delete::{delete_entries_at, delete_preview_at};
use super::error::{DriveError, DriveResult};
use super::events::{emit, DriveChange, Moved};
use super::guard::subject_folder;
use super::moves::{blocking, move_entries_at, rename_entry_at, Moves};
use super::rows::subject_ids;

async fn pool() -> DriveResult<SqlitePool> {
    crate::db::store::open_pool().await.map_err(DriveError::io)
}

/// Every subject folder `paths` are in.
fn folders<'a>(paths: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for folder in paths.into_iter().filter_map(|p| subject_folder(p)) {
        if !out.iter().any(|f| f == folder) {
            out.push(folder.to_string());
        }
    }
    out
}

async fn announce(app: &AppHandle, pool: &SqlitePool, mut change: DriveChange) {
    let paths: Vec<String> = change
        .moved
        .iter()
        .flat_map(|m| [m.from.clone(), m.to.clone()])
        .chain(change.removed.iter().cloned())
        .chain(change.added.iter().cloned())
        .collect();
    change.subject_ids = subject_ids(pool, &folders(&paths)).await;
    emit(app, &change);
}

fn moved(moves: &Moves) -> Vec<Moved> {
    moves
        .iter()
        .filter(|(from, to)| from != to)
        .map(|(from, to)| Moved {
            from: from.clone(),
            to: to.clone(),
        })
        .collect()
}

fn data_dir() -> std::path::PathBuf {
    crate::library::paths::data_dir()
}

#[tauri::command]
pub async fn create_folder(app: AppHandle, rel: String) -> Result<String, String> {
    let made = blocking(move || create_folder_at(&data_dir(), &rel)).await?;
    let pool = pool().await?;
    let change = DriveChange {
        added: vec![made.clone()],
        ..DriveChange::default()
    };
    announce(&app, &pool, change).await;
    pool.close().await;
    Ok(made)
}

#[tauri::command]
pub async fn rename_entry(app: AppHandle, rel: String, new_name: String) -> Result<String, String> {
    let pool = pool().await?;
    let result = rename_entry_at(&data_dir(), &pool, &rel, &new_name).await;
    if let Ok(moves) = &result {
        let change = DriveChange {
            moved: moved(moves),
            ..DriveChange::default()
        };
        announce(&app, &pool, change).await;
    }
    pool.close().await;
    let moves = result?;
    Ok(moves.into_iter().next().map(|(_, to)| to).unwrap_or(rel))
}

#[tauri::command]
pub async fn move_entries(
    app: AppHandle,
    rels: Vec<String>,
    target_dir: String,
) -> Result<Vec<String>, String> {
    let pool = pool().await?;
    let result = move_entries_at(&data_dir(), &pool, &rels, &target_dir).await;
    if let Ok(moves) = &result {
        let change = DriveChange {
            moved: moved(moves),
            ..DriveChange::default()
        };
        announce(&app, &pool, change).await;
    }
    pool.close().await;
    Ok(result?.into_iter().map(|(_, to)| to).collect())
}

#[tauri::command]
pub async fn delete_entries(app: AppHandle, rels: Vec<String>) -> Result<(), String> {
    let pool = pool().await?;
    let result = delete_entries_at(&data_dir(), &pool, &rels).await;
    if result.is_ok() {
        let change = DriveChange {
            removed: rels,
            ..DriveChange::default()
        };
        announce(&app, &pool, change).await;
    }
    pool.close().await;
    Ok(result?)
}

#[tauri::command]
pub async fn delete_preview(rels: Vec<String>) -> Result<Vec<String>, String> {
    Ok(blocking(move || delete_preview_at(&data_dir(), &rels)).await?)
}

#[tauri::command]
pub async fn copy_to_own(
    app: AppHandle,
    rels: Vec<String>,
    target_dir: String,
) -> Result<Vec<String>, String> {
    let pool = pool().await?;
    let result = copy_to_own_at(&data_dir(), &pool, &rels, &target_dir).await;
    if let Ok(copies) = &result {
        let change = DriveChange {
            added: copies.clone(),
            ..DriveChange::default()
        };
        announce(&app, &pool, change).await;
    }
    pool.close().await;
    Ok(result?)
}
