import { create } from "zustand";
import { ask, dismiss, EMPTY_PROMPT, step, type Prompt, type View } from "@/lib/browser/downloads/prompt";
import type { Download, DownloadsSnapshot } from "@/lib/browser/downloads/types";

/** A mirror of Rust's download list (`app/src-tauri/src/shell/browser/downloads/`),
 *  fed by `useBrowserDownloads`, with which download the Save dialog is
 *  asking about (`lib/browser/downloads/prompt.ts`). */
interface DownloadsState {
  downloads: Download[];
  /** Whether any download has been seen this session: the toolbar's button
   *  shows from then on, even once the list is cleared. */
  seen: boolean;
  prompt: Prompt;
  /** The tabs in front and the live tabs, as last reported. */
  view: View;
  /** A list from Rust; `initial` for the mount's fetch, which asks nothing. */
  apply: (snapshot: DownloadsSnapshot, now: number, initial?: boolean) => void;
  setView: (view: View, now: number) => void;
  /** An undecided arrival may have waited long enough. */
  tick: (now: number) => void;
  /** Opens the dialog for a waiting download. */
  ask: (id: number) => void;
  dismiss: () => void;
}

export const useBrowserDownloadsStore = create<DownloadsState>((set) => ({
  downloads: [],
  seen: false,
  prompt: EMPTY_PROMPT,
  view: { front: [], live: new Set() },
  apply: (snapshot, now, initial = false) =>
    set((s) => ({
      downloads: snapshot.downloads,
      seen: s.seen || snapshot.downloads.length > 0,
      prompt: step(s.prompt, snapshot.downloads, s.view, now, initial),
    })),
  setView: (view, now) =>
    set((s) => ({ view, prompt: step(s.prompt, s.downloads, view, now) })),
  tick: (now) => set((s) => ({ prompt: step(s.prompt, s.downloads, s.view, now) })),
  ask: (id) => set((s) => ({ prompt: ask(s.prompt, s.downloads, id) })),
  dismiss: () => set((s) => ({ prompt: dismiss(s.prompt) })),
}));

/** The download the dialog shows, or undefined. */
export function useAskedDownload(): Download | undefined {
  return useBrowserDownloadsStore((s) =>
    s.prompt.open == null ? undefined : s.downloads.find((d) => d.id === s.prompt.open),
  );
}
