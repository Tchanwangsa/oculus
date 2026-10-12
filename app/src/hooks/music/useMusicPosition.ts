import { useEffect, useReducer } from "react";
import { positionNow, shownPlayback, type MusicSnapshot } from "@/lib/music";

/**
 * The shown track's position now (`shownPlayback`: the device playing
 * elsewhere, else Oculus), re-rendered on each whole second of it while it
 * is playing, `active` (the pane is on screen) and the window is
 * visible. Rust stamps the position rather than ticking it (docs/music.md).
 */
export function useMusicPosition(snapshot: MusicSnapshot | null, active = true): number {
  const [, tick] = useReducer((n: number) => n + 1, 0);
  const ticking = !!snapshot && shownPlayback(snapshot).playing && active;

  useEffect(() => {
    if (!ticking || !snapshot) return;
    let timer = 0;
    const schedule = () => {
      // Lands just past the next whole second, so the clock never skips one.
      const wait = 1000 - (positionNow(shownPlayback(snapshot)) % 1000) + 20;
      timer = window.setTimeout(() => {
        if (!document.hidden) tick();
        schedule();
      }, wait);
    };
    schedule();
    const onVisible = () => {
      if (!document.hidden) tick();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [ticking, snapshot]);

  return snapshot ? positionNow(shownPlayback(snapshot)) : 0;
}
