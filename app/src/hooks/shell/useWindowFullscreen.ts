import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Whether the window is fullscreen (never an element; see `LecturePlayer`).
 *  The green button and ⌃⌘F change it too, which arrives as a resize. */
export function useWindowFullscreen(): boolean {
  const [fullscreen, setFullscreen] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let alive = true;
    const read = async () => {
      try {
        const next = await win.isFullscreen();
        if (alive) setFullscreen(next);
      } catch {
        /* ignore — the chrome just keeps its windowed spacing */
      }
    };
    read();
    const unlisten = win.onResized(read);
    return () => {
      alive = false;
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  return fullscreen;
}
