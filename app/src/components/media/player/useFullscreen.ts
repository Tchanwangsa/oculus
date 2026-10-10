import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Not `requestFullscreen()`: WKWebView gates element fullscreen behind
 * Tauri's `macos-private-api`, and even then shows only the element's subtree,
 * cutting off every Radix popup portalled to `document.body`. So the player is
 * a fixed overlay over a window-fullscreened app. Entering takes the window
 * fullscreen too; leaving the overlay keeps the window's; leaving the
 * window's fullscreen leaves both.
 */
export function useFullscreen() {
  const [isFullscreen, setIsFullscreen] = useState(false);
  const isFullscreenRef = useRef(false);
  isFullscreenRef.current = isFullscreen;

  const toggleFullscreen = useCallback(async () => {
    const next = !isFullscreenRef.current;
    setIsFullscreen(next);
    if (!next) return;
    try {
      const win = getCurrentWindow();
      if (!(await win.isFullscreen())) await win.setFullscreen(true);
    } catch {
      /* not fatal — the overlay just covers a windowed app */
    }
  }, []);

  // Leaving window fullscreen another way (green button, ⌃⌘F) arrives as a
  // resize; only the leaving matters.
  useEffect(() => {
    const win = getCurrentWindow();
    const unlisten = win.onResized(async () => {
      if (!isFullscreenRef.current) return;
      try {
        if (!(await win.isFullscreen())) setIsFullscreen(false);
      } catch {
        /* ignore */
      }
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  return { isFullscreen, isFullscreenRef, toggleFullscreen };
}
