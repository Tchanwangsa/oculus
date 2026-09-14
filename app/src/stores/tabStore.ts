import { create } from "zustand";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { navigateInTab } from "@/lib/tabRouters";

/**
 * Notion-style top tabs. Each tab is a real navigation context — a router and
 * a mounted page of its own (`app/src/components/tabs/TabPane.tsx`) — and this
 * store is the strip's view of them: which exist, what order they are in,
 * which is in front, and for each, where it sits and whether its history has
 * anywhere to go. The panes report the last two; nothing here navigates
 * except the one case below that has to.
 *
 * A browser tab is a tab whose path is `/browse/<id>`: it stands for a native
 * page WebView that Rust holds, and its path never changes — the page
 * navigates, the route does not. What keeps it pinned to its page is
 * `navigateActive` in `app/src/lib/tabRouters.ts`, on the way in.
 */
export interface AppTab {
  id: number;
  path: string;
  /** Its own history's reach, maintained by its pane — a memory router has no
   *  `window.history` for the strip's arrows to read. */
  canBack: boolean;
  canForward: boolean;
}

/** Where a tab's history stands, as its pane sees it. */
export interface TabHistory {
  canBack: boolean;
  canForward: boolean;
}

/** Where the last tab goes when it is closed. */
const HOME = "/subjects";
/** Where a first-run strip opens. */
const FIRST = "/chat";

/**
 * The strip survives a reload. It used to come back from the hash URL — one
 * path, but at least a real one — and a memory router restores nothing at all,
 * so without this a dev reload dropped every tab on the floor. Only the id and
 * path are worth keeping; a restored tab's history starts empty because it is.
 *
 * `/browse/<id>` tabs are written out with the rest and left to `useBrowserTabs`
 * to reconcile: it asks Rust for the live page list on mount and closes the
 * strip tabs whose pages are gone.
 */
const STORE_KEY = "oculus-tabs";

interface StoredStrip {
  tabs: { id: number; path: string }[];
  activeId: number;
}

function restore(): { tabs: AppTab[]; activeId: number } {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    const saved = raw ? (JSON.parse(raw) as StoredStrip) : null;
    const tabs = (saved?.tabs ?? [])
      .filter((t) => Number.isInteger(t?.id) && typeof t?.path === "string")
      .map((t) => ({ id: t.id, path: t.path, canBack: false, canForward: false }));
    if (tabs.length > 0) {
      const activeId = tabs.some((t) => t.id === saved?.activeId)
        ? saved!.activeId
        : tabs[0].id;
      return { tabs, activeId };
    }
  } catch {
    /* corrupt or unavailable — a fresh strip is a fine fallback */
  }
  return { tabs: [{ id: 1, path: FIRST, canBack: false, canForward: false }], activeId: 1 };
}

const initial = restore();

let nextId = Math.max(0, ...initial.tabs.map((t) => t.id)) + 1;

interface TabState {
  tabs: AppTab[];
  activeId: number;
  /** Opens a fresh tab at `path` and makes it active. This *is* navigation
   *  into a new context: the pane is created seeded at that path. */
  addTab: (path: string) => void;
  setActive: (id: number) => void;
  /** Moves a tab to `toIndex`, shifting the others. */
  moveTab: (id: number, toIndex: number) => void;
  /** A pane reporting where its router has landed, and how far its history
   *  now reaches either way. */
  setPath: (id: number, path: string, history: TabHistory) => void;
  /** Removes a tab. The neighbour that comes forward is already sitting at
   *  its own path, so there is nothing for the caller to navigate to. */
  closeTab: (id: number) => void;
}

export const useTabStore = create<TabState>((set, get) => ({
  tabs: initial.tabs,
  activeId: initial.activeId,

  addTab: (path) => {
    const id = nextId++;
    set((s) => ({
      tabs: [...s.tabs, { id, path, canBack: false, canForward: false }],
      activeId: id,
    }));
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

  setPath: (id, path, history) =>
    set((s) => {
      const tab = s.tabs.find((t) => t.id === id);
      if (
        !tab ||
        (tab.path === path &&
          tab.canBack === history.canBack &&
          tab.canForward === history.canForward)
      )
        return s;
      return {
        tabs: s.tabs.map((t) =>
          t.id === id ? { ...t, path, ...history } : t,
        ),
      };
    }),

  closeTab: (id) => {
    const { tabs, activeId } = get();
    const idx = tabs.findIndex((t) => t.id === id);
    if (idx === -1) return;
    // A lecture keeps playing when you switch away from its tab, so closing
    // that tab has to be what stops it — nothing downstream can tell the two
    // apart once the pane is gone. The prompt, where there is one, has already
    // been answered by the time this runs (see `confirmLeavingLecture`).
    if (ownsPlayback(id)) stopLecturePlayback();
    // Whatever this tab had open in the side panel goes with it; ids are never
    // reused, but leaving the entry behind would still leak one row per tab
    // closed for as long as the app runs.
    useSidePanelStore.getState().dropTab(id);
    // The last tab stays, but goes home — a sole browser tab whose page is
    // gone has nothing else to show. This is the one place the store steers a
    // router: there is no neighbour to come forward, so the tab has to move.
    if (tabs.length <= 1) {
      if (tabs[0].path !== HOME) navigateInTab(id, HOME);
      return;
    }
    const next = tabs.filter((t) => t.id !== id);
    if (id !== activeId) {
      set({ tabs: next });
      return;
    }
    const neighbour = next[Math.min(idx, next.length - 1)];
    set({ tabs: next, activeId: neighbour.id });
  },
}));

/** The path the tab in front is showing — what the sidebar highlights against,
 *  now that it has no router to ask. */
export function useActivePath(): string {
  return useTabStore((s) => s.tabs.find((t) => t.id === s.activeId)?.path ?? "");
}

useTabStore.subscribe((s) => {
  try {
    const strip: StoredStrip = {
      tabs: s.tabs.map((t) => ({ id: t.id, path: t.path })),
      activeId: s.activeId,
    };
    localStorage.setItem(STORE_KEY, JSON.stringify(strip));
  } catch {
    /* quota or private mode — the strip is expendable */
  }
});
