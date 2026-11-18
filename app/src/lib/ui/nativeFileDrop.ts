import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { Point } from '@/lib/whiteboard/model';

/** Native drag coordinates are points, even though Tauri calls them PhysicalPosition. */
export function watchNativeFileDrop(target: HTMLElement, onHover: (paths: string[], at: Point | null) => void, onDrop: (paths: string[], at: Point) => void, slack = 0): () => void {
  let dead = false, ratio = 1, paths: string[] = [], unlisten: (() => void) | undefined;
  const view = getCurrentWebview();
  const measure = async () => {
    try {
      const [size, scale] = await Promise.all([view.window.innerSize(), view.window.scaleFactor()]);
      const points = size.width / (scale || 1);
      if (!dead && points > 0 && window.innerWidth > 0) ratio = window.innerWidth / points;
    } catch { /* Keep the last measured zoom during a window transition. */ }
  };
  const local = (position: Point): Point | null => {
    if (getComputedStyle(target).visibility === 'hidden') return null;
    const rect = target.getBoundingClientRect(), x = position.x * ratio - rect.left, y = position.y * ratio - rect.top;
    return x >= -slack && x <= rect.width + slack && y >= -slack && y <= rect.height + slack ? { x, y } : null;
  };
  void measure();
  window.addEventListener('resize', measure);
  view.onDragDropEvent(({ payload }) => {
    if (dead) return;
    if (payload.type === 'leave') { paths = []; onHover([], null); return; }
    if (payload.type === 'enter') { paths = payload.paths; void measure(); }
    const at = local(payload.position);
    if (payload.type === 'drop') { onHover([], null); if (at) onDrop(payload.paths, at); paths = []; }
    else onHover(paths, at);
  }).then(off => { if (dead) off(); else unlisten = off; }).catch(() => {});
  return () => { dead = true; window.removeEventListener('resize', measure); unlisten?.(); };
}
