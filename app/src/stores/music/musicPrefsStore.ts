import { create } from "zustand";
import { getSetting, setSetting } from "@/lib/db";
import { MUSIC_SETTING, music, musicRow } from "@/lib/music";

/** Settings → Music's switch and the output choice, stored as the `music` row
 *  that Rust also reads at startup (`app/src-tauri/src/music/startup.rs`).
 *  Writes keep the row's other keys. */
interface MusicPrefs {
  enabled: boolean;
  loaded: boolean;
  load: () => Promise<void>;
  /** Saves, then connects from the stored login or disconnects. */
  setEnabled: (on: boolean) => Promise<void>;
  /** Moves playback to the output (null: the system default) and saves it. */
  setOutput: (name: string | null) => Promise<void>;
}

/** Merges `patch` into the stored row. */
async function saveRow(patch: Record<string, unknown>) {
  const row = musicRow(await getSetting(MUSIC_SETTING));
  await setSetting(MUSIC_SETTING, JSON.stringify({ ...row, ...patch }));
}

export const useMusicPrefsStore = create<MusicPrefs>((set) => ({
  enabled: false,
  loaded: false,
  load: async () => {
    const row = musicRow(await getSetting(MUSIC_SETTING));
    set({ enabled: row.enabled === true, loaded: true });
  },
  setEnabled: async (on) => {
    set({ enabled: on });
    await saveRow({ enabled: on });
    await (on ? music.connect() : music.disconnect());
  },
  setOutput: async (name) => {
    await music.setOutput(name);
    await saveRow({ output: name });
  },
}));
