import { create } from "zustand";
import type { BrowserSnapshot, BrowserTab } from "@/lib/browser";

/** A mirror of Rust's browser tab list (`app/src-tauri/src/browser.rs`),
 *  fed by `useBrowserTabs`. Icons are keyed by **host** and arrive on their
 *  own event, so snapshots don't re-carry base64 on every load edge. */
interface BrowserState {
  tabs: BrowserTab[];
  /** Host → `data:` URL. */
  favicons: Record<string, string>;
  /** Until the first snapshot, an unknown tab id is unknown, not dead. */
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
  // Merged *under* icons that arrived while the database read was in flight.
  seedFavicons: (icons) =>
    set((s) => ({ favicons: { ...icons, ...s.favicons } })),
}));
