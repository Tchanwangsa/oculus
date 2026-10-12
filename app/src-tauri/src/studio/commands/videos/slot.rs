//! Rendering stills on the blocking pool, at most half the cores at once:
//! a page of thumbnails asks for many, and a `Renderer` holds a JS runtime
//! and a raster context. A request queues here without holding a thread.

use std::sync::OnceLock;

use crate::runtime::blocking;

pub async fn on_render_slot<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    static SLOTS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let slots = SLOTS.get_or_init(|| {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        tokio::sync::Semaphore::new((cores / 2).max(1))
    });
    let _slot = slots.acquire().await.map_err(|e| e.to_string())?;
    blocking::run(work).await
}
