import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { MUSIC_STATE_EVENT, music, type MusicSnapshot } from "@/lib/music";
import { wireExclusivePlayback } from "@/lib/music/exclusive";
import { useMusicStore } from "@/stores/music/musicStore";
import { useMusicPrefsStore } from "@/stores/music/musicPrefsStore";

/**
 * Mirrors Rust's music snapshot into `musicStore`: one read on mount (a
 * reload has missed every event), then `music-state`. Also loads Settings →
 * Music's switch and wires music against lecture audio
 * (`lib/music/exclusive.ts`). Mounted once, in AppLayout.
 */
export function useMusicState() {
  useEffect(() => {
    let cancelled = false;
    const apply = (s: MusicSnapshot) => {
      if (!cancelled) useMusicStore.getState().apply(s);
    };
    music.state().then(apply).catch(() => {});
    const unlisten = listen<MusicSnapshot>(MUSIC_STATE_EVENT, (e) => apply(e.payload));
    void useMusicPrefsStore.getState().load().catch(() => {});
    const unwire = wireExclusivePlayback({
      playing: () => useMusicStore.getState().snapshot?.playing ?? false,
      subscribe: (fn) => useMusicStore.subscribe(fn),
    });
    return () => {
      cancelled = true;
      unlisten.then((off) => off()).catch(() => {});
      unwire();
    };
  }, []);
}
