import { useEffect, useRef, useState } from 'react';
import { shouldUseNativeWhiteboardFullscreen } from './fullscreenPolicy';

/** Focus mode works on plain HTTP and browsers without the Fullscreen API. */
export function useWhiteboardFocus() {
  const root = useRef<HTMLDivElement>(null);
  const [focused, setFocused] = useState(false);
  const ownsFullscreen = useRef(false);
  const pending = useRef(0);
  useEffect(() => {
    const changed = () => {
      if (document.fullscreenElement === root.current) { ownsFullscreen.current = true; setFocused(true); }
      else if (ownsFullscreen.current) { ownsFullscreen.current = false; setFocused(false); }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !event.defaultPrevented) { pending.current++; setFocused(false); }
    };
    document.addEventListener('fullscreenchange', changed);
    window.addEventListener('keydown', escape, true);
    return () => {
      pending.current++;
      document.removeEventListener('fullscreenchange', changed);
      window.removeEventListener('keydown', escape, true);
      if (ownsFullscreen.current && document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    };
  }, []);
  const toggle = async () => {
    const request = ++pending.current;
    if (focused) {
      setFocused(false);
      if (document.fullscreenElement === root.current) await document.exitFullscreen().catch(() => {});
      return;
    }
    setFocused(true);
    try {
      if (shouldUseNativeWhiteboardFullscreen(navigator) && root.current?.requestFullscreen && document.fullscreenEnabled) {
        await root.current.requestFullscreen();
        if (pending.current !== request && document.fullscreenElement === root.current) await document.exitFullscreen();
      }
    } catch { /* Focus mode still fills the browser viewport when fullscreen is unavailable. */ }
  };
  return { root, focused, toggle };
}
