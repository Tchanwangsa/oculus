//! PDF imports hold only documents opened through scoped library paths.
//! Later raster requests carry a session ID, never an arbitrary path.
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Component, Path};
use std::sync::{Arc, Mutex, OnceLock};

use hayro::hayro_syntax::Pdf;
use serde::Serialize;

use crate::library::pdf_render::{self, budget, OpenError};

#[cfg(test)]
mod tests;

const MAX_PDF_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SESSIONS: usize = 4;

struct Session {
    owner: String,
    pdf: Arc<Pdf>,
}

fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Serialize)]
pub struct PageSize {
    width: f32,
    height: f32,
}

#[derive(Serialize)]
pub struct OpenedImport {
    id: String,
    name: String,
    pages: Vec<PageSize>,
}

fn load(path: &Path) -> Result<Pdf, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot read PDF: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Choose a PDF file".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_PDF_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Cannot read PDF: {e}"))?;
    if bytes.len() as u64 > MAX_PDF_BYTES {
        return Err("Choose a PDF smaller than 128 MB".into());
    }
    pdf_render::parse(bytes).map_err(|error| match error {
        OpenError::Encrypted => {
            "This PDF is password protected. Unlock it before importing.".into()
        }
        OpenError::Invalid => "This file is not a readable PDF".into(),
        OpenError::Panicked => "This PDF could not be opened".into(),
        OpenError::Read(error) => error,
    })
}

fn register(owner: String, name: String, pdf: Pdf) -> Result<OpenedImport, String> {
    let pages = pdf_render::guarded(|| {
        pdf.pages()
            .iter()
            .map(|page| {
                let (width, height) = page.render_dimensions();
                PageSize { width, height }
            })
            .collect::<Vec<_>>()
    })
    .map_err(|_| "Could not read PDF page sizes")?;
    if pages.is_empty() {
        return Err("This PDF has no pages".into());
    }
    if pages
        .iter()
        .any(|p| !p.width.is_finite() || !p.height.is_finite() || p.width <= 0.0 || p.height <= 0.0)
    {
        return Err("This PDF has an invalid page size".into());
    }
    let mut held = crate::providers::ratelimit::hold(sessions());
    if held.values().filter(|s| s.owner == owner).count() >= MAX_SESSIONS {
        return Err("Close another PDF import before opening one".into());
    }
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = format!(
        "{}-{}",
        crate::runtime::clock::now_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    held.insert(
        id.clone(),
        Session {
            owner,
            pdf: Arc::new(pdf),
        },
    );
    Ok(OpenedImport { id, name, pages })
}

fn document(owner: &str, id: &str) -> Result<Arc<Pdf>, String> {
    crate::providers::ratelimit::hold(sessions())
        .get(id)
        .filter(|session| session.owner == owner)
        .map(|session| session.pdf.clone())
        .ok_or_else(|| "This PDF import has been closed. Choose the PDF again.".into())
}

fn close(owner: &str, id: &str) {
    let mut held = crate::providers::ratelimit::hold(sessions());
    if held.get(id).is_some_and(|session| session.owner == owner) {
        held.remove(id);
    }
}

fn library_path(root: &Path, relative: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(relative);
    let mut components = path.components();
    let inside = matches!(components.next(), Some(Component::Normal(first))
        if ["courses", "lectures", "agents"].iter().any(|s| first == *s));
    if !inside || !components.all(|part| matches!(part, Component::Normal(_))) {
        return Err("Choose a PDF inside the library".into());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let chosen = root.join(path).canonicalize().map_err(|e| e.to_string())?;
    if !chosen.starts_with(root) {
        return Err("Choose a PDF inside the library".into());
    }
    Ok(chosen)
}

#[tauri::command]
pub async fn canvas_open_library_pdf(
    webview: tauri::Webview,
    relative_path: String,
) -> Result<OpenedImport, String> {
    let owner = webview.label().to_string();
    crate::runtime::blocking::run(move || {
        let path = library_path(&crate::library::paths::data_dir(), &relative_path)?;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        register(owner, name, load(&path)?)
    })
    .await
}

fn raster(pdf: &Pdf, page: u32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    if width == 0
        || height == 0
        || width > 2400
        || height > 2400
        || u64::from(width) * u64::from(height) > 4_000_000
    {
        return Err("PDF image size exceeds the import limit".into());
    }
    let page = page
        .checked_sub(1)
        .and_then(|i| pdf.pages().get(i as usize))
        .ok_or("Choose an existing PDF page")?;
    let _held = budget::global().reserve(budget::Lane::Viewer, budget::estimate(width, height));
    let rgba = pdf_render::render_rgba(page, width, height)
        .map_err(|_| "Could not render this PDF page")?;
    let image = image::RgbaImage::from_raw(width, height, rgba).ok_or("Invalid PDF image")?;
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}

#[tauri::command]
pub async fn canvas_render_pdf_page(
    webview: tauri::Webview,
    id: String,
    page: u32,
    width: u32,
    height: u32,
) -> Result<tauri::ipc::Response, String> {
    static SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let _slot = SLOTS
        .get_or_init(|| tokio::sync::Semaphore::new(2))
        .acquire()
        .await
        .map_err(|e| e.to_string())?;
    let owner = webview.label().to_string();
    let bytes = pdf_render::on_render_thread(move || {
        let pdf = document(&owner, &id)?;
        raster(&pdf, page, width, height)
    })
    .await?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn canvas_close_pdf(webview: tauri::Webview, id: String) -> Result<(), String> {
    close(webview.label(), &id);
    Ok(())
}
