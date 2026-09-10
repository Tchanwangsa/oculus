import { create } from "zustand";
import type { BrowserSnapshot, BrowserTab } from "@/lib/browser";

/**
 * A mirror of Rust's browser tab list (`app/src-tauri/src/browser.rs`),
 * replaced whole on every `browser-state` event. Read-only from here: the
 * tab strip titles its browser tabs from it and the browser page fills its
 * address bar from it. `useBrowserTabs` keeps it fed and reconciles it into
 * the tab strip.
 */
interface BrowserState {
  tabs: BrowserTab[];
  /** False until the first snapshot arrives — before that, an unknown tab
   *  id is unknown, not dead. */
  loaded: boolean;
  apply: (snapshot: BrowserSnapshot) => void;
}

export const useBrowserStore = create<BrowserState>((set) => ({
  tabs: [],
  loaded: false,
  apply: (snapshot) => set({ tabs: snapshot.tabs, loaded: true }),
}));
