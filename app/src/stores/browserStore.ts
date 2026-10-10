import { create } from "zustand";
import { shallow } from "zustand/shallow";
import type { BrowserSnapshot, BrowserTab } from "@/lib/browser";

/** A mirror of Rust's browser tab list (`app/src-tauri/src/browser/`),
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
  apply: (snapshot) => set((s) => {
    const previous = new Map(s.tabs.map((tab) => [tab.id, tab]));
    const tabs = snapshot.tabs.map((tab) => {
      const cached = previous.get(tab.id);
      return cached && shallow(cached, tab) ? cached : tab;
    });
    // Native snapshots carry every page, even if only one changed. Preserve
    // the other page objects so their panes' selectors do not re-render.
    return s.loaded && shallow(s.tabs, tabs) ? s : { tabs, loaded: true };
  }),
  setFavicon: (host, icon) =>
    set((s) => s.favicons[host] === icon ? s : { favicons: { ...s.favicons, [host]: icon } }),
  // Merged *under* icons that arrived while the database read was in flight.
  seedFavicons: (icons) =>
    set((s) => {
      const favicons = { ...icons, ...s.favicons };
      return shallow(s.favicons, favicons) ? s : { favicons };
    }),
}));
