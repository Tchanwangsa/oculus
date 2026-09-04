import { create } from "zustand";

/**
 * Notion-style top tabs: each tab is an independent navigation context whose
 * path tracks wherever you go while it's active. Purely a UI affordance over
 * the single router — switching tabs navigates to the tab's stored path.
 */
export interface AppTab {
  id: number;
  path: string;
}

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
    if (tabs.length <= 1) return null; // the last tab stays
    const idx = tabs.findIndex((t) => t.id === id);
    if (idx === -1) return null;
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
