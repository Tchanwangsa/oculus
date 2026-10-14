import { useEffect, useRef } from "react";
import type { MusicSnapshot } from "@/lib/music";
import { handleMusicSpaceKey } from "@/lib/music/spaceKey";
import { useMusicStore } from "@/stores/music/musicStore";
import { useTabId } from "@/components/tabs/TabContext";
import { useActivePaneId } from "@/stores/shell/tabs/selectors";

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
    const handler = (e: KeyboardEvent) => handleMusicSpaceKey(e, snapshotRef.current, run);
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [focused, run]);
}
