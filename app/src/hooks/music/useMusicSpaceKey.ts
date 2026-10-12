import { useEffect, useRef } from "react";
import { music, musicControllable, shownPlayback, type MusicSnapshot } from "@/lib/music";
import { useMusicStore } from "@/stores/music/musicStore";
import { useTabId } from "@/components/tabs/TabContext";
import { useActivePaneId } from "@/stores/shell/tabs/selectors";

/** Where Space belongs to the focused control: typing, and open popups that
 *  use it to pick. */
const OWNS_SPACE =
  'input, textarea, select, [contenteditable="true"], [role="dialog"], [role="menu"], [role="listbox"], [data-slot="popover-content"]';

/**
 * Space plays and pauses what the player shows, while this page is the
 * focused pane. It is taken from a focused button too, so Space after
 * clicking a track or the shuffle toggle never re-clicks it.
 */
export function useMusicSpaceKey(snapshot: MusicSnapshot | null) {
  const focused = useActivePaneId() === useTabId();
  const run = useMusicStore((s) => s.run);
  const snapshotRef = useRef(snapshot);
  snapshotRef.current = snapshot;

  useEffect(() => {
    if (!focused) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key !== " " || e.metaKey || e.ctrlKey || e.altKey) return;
      const target = e.target as HTMLElement | null;
      if (target?.closest(OWNS_SPACE)) return;
      e.preventDefault();
      const s = snapshotRef.current;
      if (e.repeat || !s || !musicControllable(s)) return;
      void run(shownPlayback(s).playing ? music.pausePlayer : music.play);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [focused, run]);
}
