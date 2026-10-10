import { invoke } from "@tauri-apps/api/core";

/**
 * Rasterising a page (`pdf_render`): sizes clamped to what Rust accepts, and
 * a queue that keeps two renders in flight, newest request first.
 */

/** Rust refuses a raster past these (`pdf_render`'s "too-large"). Past them
 *  the canvas is CSS-scaled: a 16M-pixel page can take ~1 GB to render. */
const MAX_SIDE = 8192;
const MAX_PIXELS = 16_000_000;

/** A raster size for a page of `width`×`height` device pixels, scaled down to
 *  the limits above. */
export function rasterSize(width: number, height: number): { width: number; height: number } {
  const w = Math.max(1, width);
  const h = Math.max(1, height);
  const k = Math.min(1, MAX_SIDE / w, MAX_SIDE / h, Math.sqrt(MAX_PIXELS / (w * h)));
  return { width: Math.max(1, Math.floor(w * k)), height: Math.max(1, Math.floor(h * k)) };
}

interface Job {
  run: () => Promise<void>;
  live: () => boolean;
}

/** Renders in flight at once; the rest wait, newest first, so the pages a
 *  fast scroll lands on beat the ones it passed. */
const RENDER_SLOTS = 2;
const queue: Job[] = [];
let running = 0;

function pump() {
  while (running < RENDER_SLOTS && queue.length) {
    const job = queue.pop()!;
    if (!job.live()) continue;
    running += 1;
    job.run().finally(() => {
      running -= 1;
      pump();
    });
  }
}

/** One page as pixels, or null when `live()` turned false before its turn. */
export function renderPage(
  path: string,
  page: number,
  width: number,
  height: number,
  live: () => boolean,
): Promise<ImageData | null> {
  return new Promise((resolve, reject) => {
    queue.push({
      live: () => {
        const on = live();
        if (!on) resolve(null);
        return on;
      },
      run: () =>
        invoke<ArrayBuffer>("pdf_render", { path, page, width, height }).then(
          (buf) => resolve(live() ? new ImageData(new Uint8ClampedArray(buf), width, height) : null),
          reject,
        ),
    });
    pump();
  });
}
