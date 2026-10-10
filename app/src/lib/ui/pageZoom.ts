// The window's page zoom (never CSS `zoom`, see docs/ui.md). `AppLayout`
// owns changing it; `App` applies the stored value at boot so a view shown
// instead of the shell (onboarding) renders at the same size.
import { getCurrentWebview } from "@tauri-apps/api/webview";

const ZOOM_KEY = "oculus-zoom";
export const DEFAULT_ZOOM = 1.15;
export const ZOOM_MIN = 0.7;
export const ZOOM_MAX = 1.8;

export function storedZoom(): number {
  const stored = Number(localStorage.getItem(ZOOM_KEY));
  return stored >= ZOOM_MIN && stored <= ZOOM_MAX ? stored : DEFAULT_ZOOM;
}

/** Remember `zoom` and apply it. The `--app-zoom` var is only for chrome that
 *  must stay at device size. */
export function applyZoom(zoom: number): void {
  localStorage.setItem(ZOOM_KEY, String(zoom));
  document.documentElement.style.setProperty("--app-zoom", String(zoom));
  getCurrentWebview()
    .setZoom(zoom)
    .catch(() => {});
}
