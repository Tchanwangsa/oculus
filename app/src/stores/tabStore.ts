import { create } from "zustand";
import { browseId } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";

/**
 * Notion-style top tabs: each tab is an independent navigation context whose
 * path tracks wherever you go while it's active. Purely a UI affordance over
 * the single router — switching tabs navigates to the tab's stored path.
 *
 * A browser tab is a tab whose path is `/browse/<id>`: it stands for a
 * native page WebView that Rust holds, and its path never changes — the
 * page navigates, the route does not. Two rules keep that true, below.
 */
export interface AppTab {
  id: number;
  path: string;
}

/** Where the last tab goes when it is closed. */
const HOME = "/subjects";

interface TabState {
  tabs: AppTab[];
  activeId: number;
  /** Updates the active tab's path as the router moves (creates the first tab). */
  trackNavigation: (path: string) => void;
  /** Opens a fresh tab at `path` and makes it active. */
  addTab: (path: string) => void;
  setActive: (id: number) => void;
  /** Moves a tab to `toIndex`, shifting the others. */
  moveTab: (id: number, toIndex: number) => void;
  /** Removes a tab; returns the path to navigate to if the active tab changed. */
  closeTab: (id: number) => string | null;
}

let nextId = 1;

export const useTabStore = create<TabState>((set, get) => ({
  tabs: [],
  activeId: 0,

  trackNavigation: (path) => {
    const { tabs, activeId } = get();
    if (tabs.length === 0) {
      const id = nextId++;
      set({ tabs: [{ id, path }], activeId: id });
      return;
    }
    const active = tabs.find((t) => t.id === activeId);
    // A browser tab can hold nothing but its page: a navigation away from
    // it (the sidebar, say) gets a tab of its own rather than replacing
    // the page.
    if (active && browseId(active.path) != null && browseId(path) == null) {
      const id = nextId++;
      set({ tabs: [...tabs, { id, path }], activeId: id });
      return;
    }
    // History landing on a page that is already open elsewhere in the strip
    // switches to that tab, so one page is never two tabs.
    if (browseId(path) != null) {
      const owner = tabs.find((t) => t.path === path);
      if (owner && owner.id !== activeId) {
        set({ activeId: owner.id });
        return;
      }
    }
    set({
      tabs: tabs.map((t) => (t.id === activeId ? { ...t, path } : t)),
    });
  },

  addTab: (path) => {
    const id = nextId++;
    set((s) => ({ tabs: [...s.tabs, { id, path }], activeId: id }));
  },

  setActive: (id) => set({ activeId: id }),

  moveTab: (id, toIndex) =>
    set((s) => {
      const from = s.tabs.findIndex((t) => t.id === id);
      if (from === -1 || from === toIndex) return s;
      const tabs = [...s.tabs];
      const [tab] = tabs.splice(from, 1);
      tabs.splice(toIndex, 0, tab);
      return { tabs };
    }),

  closeTab: (id) => {
    const { tabs, activeId } = get();
    const idx = tabs.findIndex((t) => t.id === id);
    if (idx === -1) return null;
    // A lecture keeps playing when you switch away from its tab, so closing
    // that tab has to be what stops it — nothing downstream can tell the two
    // apart once the route is gone. The prompt, where there is one, has already
    // been answered by the time this runs (see `confirmLeavingLecture`).
    if (ownsPlayback(id)) stopLecturePlayback();
    // The last tab stays, but goes home — a sole browser tab whose page is
    // gone has nothing else to show.
    if (tabs.length <= 1) {
      if (tabs[0].path === HOME) return null;
      set({ tabs: [{ id, path: HOME }] });
      return HOME;
    }
    const next = tabs.filter((t) => t.id !== id);
    if (id !== activeId) {
      set({ tabs: next });
      return null;
    }
    const neighbour = next[Math.min(idx, next.length - 1)];
    set({ tabs: next, activeId: neighbour.id });
    return neighbour.path;
  },
}));
