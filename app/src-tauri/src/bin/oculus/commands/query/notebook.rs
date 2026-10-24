//! `oculus read` on a notebook: the pictures of its pages, since it has no text.

use crate::*;
use app_lib::library::paths::{notebook_pages_dir, read_notebook_pages};

#[derive(Serialize)]
pub(crate) struct NotebookPageView {
    pub(crate) page: usize,
    id: String,
    /// Absolute, so an agent can open it with its own image tool.
    pub(crate) image: String,
    updated_at: u64,
    pub(crate) age_seconds: u64,
}

#[derive(Serialize)]
pub(crate) struct NotebookView {
    pub(crate) page_count: usize,
    pub(crate) pages: Vec<NotebookPageView>,
}

fn none_yet(path: &str) -> String {
    format!(
        "{path} has no page pictures yet.\n       \
         They are drawn while the notebook is open in the Oculus app: open it there, \
         wait a few seconds, and read it again. Only a notebook (a canvas with pages) \
         can be read; a whiteboard has none."
    )
}

/// The pictures of a notebook's pages, `wanted` narrowing them, as the app last wrote them.
pub(crate) fn notebook_view(
    data_dir: &std::path::Path,
    file_id: i64,
    path: &str,
    wanted: Option<&[(i64, i64)]>,
    now: u64,
) -> Result<NotebookView, String> {
    let manifest = read_notebook_pages(data_dir, file_id).ok_or_else(|| none_yet(path))?;
    let dir = notebook_pages_dir(data_dir, file_id);
    let present: Vec<NotebookPageView> = manifest
        .pages
        .iter()
        .filter(|page| dir.join(&page.image).is_file())
        .map(|page| NotebookPageView {
            page: page.page,
            id: page.id.clone(),
            image: dir.join(&page.image).to_string_lossy().into_owned(),
            updated_at: page.updated_at,
            age_seconds: now.saturating_sub(page.updated_at),
        })
        .collect();
    if present.is_empty() {
        return Err(none_yet(path));
    }
    let pages: Vec<NotebookPageView> = present
        .into_iter()
        .filter(|page| wanted.is_none_or(|w| page_wanted(w, page.page as i64)))
        .collect();
    if pages.is_empty() {
        return Err(format!(
            "no such page — {path} has pages 1-{}",
            manifest.page_count
        ));
    }
    Ok(NotebookView {
        page_count: manifest.page_count,
        pages,
    })
}

pub(crate) fn age(seconds: u64) -> String {
    match seconds {
        0..=4 => "just now".into(),
        5..=59 => format!("{seconds}s ago"),
        60..=3599 => format!("{}m ago", seconds / 60),
        3600..=86399 => format!("{}h ago", seconds / 3600),
        _ => format!("{}d ago", seconds / 86400),
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Ctx {
    pub(crate) fn read_notebook(
        &self,
        file: &LibFile,
        wanted: Option<&[(i64, i64)]>,
    ) -> Result<(), String> {
        let view = notebook_view(
            &self.data_dir,
            file.id,
            &file.relative_path,
            wanted,
            unix_now(),
        )?;
        if self.json {
            #[derive(Serialize)]
            struct Out<'a> {
                path: &'a str,
                filename: &'a str,
                subject: &'a str,
                file_type: &'a str,
                notebook: &'a NotebookView,
            }
            return self.emit(&Out {
                path: &file.relative_path,
                filename: &file.filename,
                subject: &file.code,
                file_type: &file.file_type,
                notebook: &view,
            });
        }
        println!(
            "{}  {}",
            paint(&file.relative_path, BOLD),
            paint(&format!("notebook, {} page(s)", view.page_count), DIM)
        );
        println!();
        for page in &view.pages {
            println!(
                "page {}  {}  {}",
                page.page,
                page.image,
                paint(&format!("updated {}", age(page.age_seconds)), DIM)
            );
        }
        println!();
        println!(
            "{}",
            paint(
                "These are pictures of what the student has written, redrawn a few seconds after \
                 each stroke. Open the page you need with your image tool.",
                DIM
            )
        );
        Ok(())
    }
}
