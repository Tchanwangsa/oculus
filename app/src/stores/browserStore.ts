import { create } from "zustand";
import type { BrowserSnapshot, BrowserTab } from "@/lib/browser";

/**
 * A mirror of Rust's browser tab list (`app/src-tauri/src/browser.rs`),
 * replaced whole on every `browser-state` event. Read-only from here: the
 * tab strip titles its browser tabs from it and the browser page fills its
 * address bar, its arrows and its zoom from it. `useBrowserTabs` keeps it fed
 * and reconciles it into the tab strip.
 *
 * Site icons ride alongside rather than inside the snapshot. They are keyed by
 * **host**, not by tab — one icon serves every tab on a site, and the history
 * list wants the same icons for sites no tab is on — and they arrive one at a
 * time on their own event, because a snapshot that carried kilobytes of base64
 * would carry them again on every page-load edge.
 */
interface BrowserState {
  tabs: BrowserTab[];
  /** Host → `data:` URL. Seeded from the database on startup and added to as
   *  Rust finds more. */
  favicons: Record<string, string>;
  /** False until the first snapshot arrives — before that, an unknown tab
   *  id is unknown, not dead. */
  loaded: boolean;
  apply: (snapshot: BrowserSnapshot) => void;
  setFavicon: (host: string, icon: string) => void;
  seedFavicons: (icons: Record<string, string>) => void;
}

export const useBrowserStore = create<BrowserState>((set) => ({
  tabs: [],
  favicons: {},
  loaded: false,
  apply: (snapshot) => set({ tabs: snapshot.tabs, loaded: true }),
  setFavicon: (host, icon) =>
    set((s) => ({ favicons: { ...s.favicons, [host]: icon } })),
  // Merged *under* what is already there: a fresh icon that arrived while the
  // database read was in flight is the newer answer.
  seedFavicons: (icons) =>
    set((s) => ({ favicons: { ...icons, ...s.favicons } })),
}));
