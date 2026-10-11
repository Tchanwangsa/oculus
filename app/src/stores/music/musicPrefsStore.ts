import { create } from "zustand";
import { getSetting, setSetting } from "@/lib/db";
import { MUSIC_SETTING, music, musicEnabled } from "@/lib/music";

/** Settings → Music's switch, stored as the `music` row that Rust also reads
 *  at startup to decide whether to connect (`app/src-tauri/src/music/startup.rs`). */
interface MusicPrefs {
  enabled: boolean;
  loaded: boolean;
  load: () => Promise<void>;
  /** Saves, then connects from the stored login or disconnects. */
  setEnabled: (on: boolean) => Promise<void>;
}

export const useMusicPrefsStore = create<MusicPrefs>((set) => ({
  enabled: false,
  loaded: false,
  load: async () => {
    set({ enabled: musicEnabled(await getSetting(MUSIC_SETTING)), loaded: true });
  },
  setEnabled: async (on) => {
    set({ enabled: on });
    await setSetting(MUSIC_SETTING, JSON.stringify({ enabled: on }));
    await (on ? music.connect() : music.disconnect());
  },
}));
