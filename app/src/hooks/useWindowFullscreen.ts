import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Whether the *window* is fullscreen. Fullscreen here is always the window's,
 * never an element's (see `LecturePlayer`), so chrome that has to dodge native
 * furniture — the traffic-light gap in the tab strip — can ask this instead of
 * threading state down from whoever pressed the button. The green button and
 * ⌃⌘F change it too, and resize is how that arrives.
 */
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
