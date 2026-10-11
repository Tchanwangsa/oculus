import { create } from "zustand";
import type { MusicSnapshot } from "@/lib/music";

/** A mirror of Rust's music snapshot (`app/src-tauri/src/music/state.rs`),
 *  fed by `useMusicState`, plus the last playback command that failed (the
 *  snapshot's `error` is Rust's own). */
interface MusicState {
  snapshot: MusicSnapshot | null;
  commandError: string | null;
  apply: (snapshot: MusicSnapshot) => void;
  /** Runs a `music.*` command; a failure is kept until one succeeds.
   *  Resolves to whether it succeeded, for a control's own confirmation. */
  run: (command: () => Promise<unknown>) => Promise<boolean>;
}

export const useMusicStore = create<MusicState>((set) => ({
  snapshot: null,
  commandError: null,
  apply: (snapshot) => set({ snapshot }),
  run: (command) =>
    command().then(
      () => {
        set({ commandError: null });
        return true;
      },
      (e: unknown) => {
        set({ commandError: String(e) });
        return false;
      },
    ),
}));

/** What the player shows as its error: Rust's, else a failed command's. */
export function musicError(s: MusicState): string | null {
  return s.snapshot?.error ?? s.commandError;
}
