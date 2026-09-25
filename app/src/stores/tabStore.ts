import { create } from "zustand";
import { browser, browseId } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { navigateInTab } from "@/lib/tabRouters";

/**
 * The top tab strip. Each tab is a navigation context of its own (a router
 * and page in `app/src/components/tabs/TabPane.tsx`); this store holds which
 * exist, their order, which is in front, and each pane's path and history
 * reach, as the panes report them.
 *
 * A tab can be split (⌥⌘T) into two panes. Everything below a tab — router,
 * side-panel peek, playing lecture, Recent entry — is keyed by **pane id**; a
 * tab's main pane carries the tab's own id, which is why `AppTab extends
 * PaneState`.
 *
 * A browser tab's path is `/browse/<id>` and never changes: it stands for a
 * native WebView Rust holds (see `navigateActive` in
 * `app/src/lib/tabRouters.ts`).
 */

/** Which half of a split tab. An unsplit tab is all `"main"`. */
export type PaneSide = "main" | "split";

/** One mounted pane: a router, a page, and how far its history reaches. */
export interface PaneState {
  /** Unique across every pane in the window, never reused. A tab's main pane
   *  carries the tab's own id. */
  id: number;
  path: string;
  /** Reported by the pane — a memory router has no `window.history`. */
  canBack: boolean;
  canForward: boolean;
}

export interface AppTab extends PaneState {
  /** The pane beside the main one, or null when the tab is not split. */
  split: PaneState | null;
  /** The half the shell drives — sidebar rows, ⌘K, breadcrumbs, the strip's
   *  arrows. */
  focus: PaneSide;
}

interface TabHistory {
  canBack: boolean;
  canForward: boolean;
}

/** The new-tab page: where the last closed tab and a fresh split land. */
const HOME = "/new";
/** Where a first-run strip opens. */
const FIRST = "/chat";

/** The strip persists across reloads (a memory router restores nothing).
 *  `/browse/<id>` panes are reconciled against Rust by `useBrowserTabs`. */
const STORE_KEY = "oculus-tabs";

/** How far back ⇧⌘T reaches. */
const CLOSED_LIMIT = 10;

/**
 * A closed tab, kept for ⇧⌘T. A browser tab is remembered by **URL**: its
 * `/browse/<id>` path names a page Rust has destroyed. Which of `path`/`url`
 * is set says which kind it is.
 */
interface ClosedTab {
  /** Strip index to reopen at; clamped on the way out. */
  index: number;
  /** The route its main pane held, or null for a browser tab. */
  path: string | null;
  url: string | null;
  /** The split half's route. A split holding a browser page is dropped. */
  split: string | null;
}

interface StoredPane {
  id: number;
  path: string;
}

interface StoredTab extends StoredPane {
  split?: StoredPane | null;
  focus?: PaneSide;
}

interface StoredStrip {
  tabs: StoredTab[];
  activeId: number;
  closed?: ClosedTab[];
}

function pane(id: number, path: string): PaneState {
  return { id, path, canBack: false, canForward: false };
}

/** Generic so filtering a `StoredTab[]` does not narrow its elements down to
 *  the pane fields they share. */
function validPane<T extends StoredPane>(p: T | null | undefined): p is T {
  return !!p && Number.isInteger(p.id) && typeof p.path === "string";
}

function validClosed(e: Partial<ClosedTab> | null | undefined): boolean {
  return (
    !!e &&
    Number.isInteger(e.index) &&
    (typeof e.path === "string" || typeof e.url === "string")
  );
}

function restore(): { tabs: AppTab[]; activeId: number; closed: ClosedTab[] } {
  let closed: ClosedTab[] = [];
  try {
    const raw = localStorage.getItem(STORE_KEY);
    const saved = raw ? (JSON.parse(raw) as StoredStrip) : null;
    closed = (saved?.closed ?? []).filter(validClosed).map((e) => ({
      index: e.index,
      path: typeof e.path === "string" ? e.path : null,
      url: typeof e.url === "string" ? e.url : null,
      split: typeof e.split === "string" ? e.split : null,
    }));
    const tabs = (saved?.tabs ?? []).filter(validPane).map((t) => {
      const split = validPane(t.split) ? pane(t.split.id, t.split.path) : null;
      return {
        ...pane(t.id, t.path),
        split,
        focus: split && t.focus === "split" ? ("split" as const) : ("main" as const),
      };
    });
    if (tabs.length > 0) {
      const activeId = tabs.some((t) => t.id === saved?.activeId)
        ? saved!.activeId
        : tabs[0].id;
      return { tabs, activeId, closed };
    }
  } catch {
    /* corrupt or unavailable — a fresh strip is a fine fallback */
  }
  return {
    tabs: [{ ...pane(1, FIRST), split: null, focus: "main" }],
    activeId: 1,
    closed,
  };
}

const initial = restore();

/** Pane ids share one counter, so it has to clear both halves of every
 *  restored tab. */
let nextId =
  Math.max(
    0,
    ...initial.tabs.flatMap((t) => [t.id, t.split?.id ?? 0]),
  ) + 1;

interface TabState {
  tabs: AppTab[];
  activeId: number;
  /** Closed tabs, newest last — the stack ⇧⌘T pops. */
  closed: ClosedTab[];
  addTab: (path: string) => void;
  setActive: (id: number) => void;
  moveTab: (id: number, toIndex: number) => void;
  /** A pane reporting where its router landed. Addressed by **pane** id. */
  setPath: (paneId: number, path: string, history: TabHistory) => void;
  /** Removes a tab and its split half. */
  closeTab: (id: number) => void;
  /** Pushes onto the reopen stack. A browser tab is remembered by the strip
   *  before Rust destroys the page, since its URL is gone by the time the
   *  pane closes. */
  remember: (entry: ClosedTab) => void;
  /** ⇧⌘T. A browser tab reopens by URL; `useBrowserTabs` adopts the page. */
  reopenTab: () => void;
  /** Splits `tabId` at `path`, or — already split — focuses the split. */
  openSplit: (tabId: number, path?: string) => void;
  closeSplit: (tabId: number) => void;
  /** ⌥⌘T: split if whole, close the split if focused there, else focus it. */
  toggleSplit: (tabId: number) => void;
  focusPane: (tabId: number, side: PaneSide) => void;
}

export function panesOf(tab: AppTab): PaneState[] {
  return tab.split ? [tab, tab.split] : [tab];
}

export function focusedPane(tab: AppTab): PaneState {
  return tab.focus === "split" && tab.split ? tab.split : tab;
}

export const useTabStore = create<TabState>((set, get) => ({
  tabs: initial.tabs,
  activeId: initial.activeId,
  closed: initial.closed,

  addTab: (path) => {
    const id = nextId++;
    set((s) => ({
      tabs: [...s.tabs, { ...pane(id, path), split: null, focus: "main" }],
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

  setPath: (paneId, path, history) =>
    set((s) => {
      const same = (p: PaneState) =>
        p.path === path &&
        p.canBack === history.canBack &&
        p.canForward === history.canForward;
      const tab = s.tabs.find((t) => panesOf(t).some((p) => p.id === paneId));
      if (!tab) return s;
      const target = panesOf(tab).find((p) => p.id === paneId)!;
      if (same(target)) return s;
      return {
        tabs: s.tabs.map((t) => {
          if (t !== tab) return t;
          if (t.id === paneId) return { ...t, path, ...history };
          return { ...t, split: { ...t.split!, path, ...history } };
        }),
      };
    }),

  openSplit: (tabId, path = HOME) =>
    set((s) => ({
      tabs: s.tabs.map((t) => {
        if (t.id !== tabId) return t;
        if (t.split) return { ...t, focus: "split" };
        return { ...t, split: pane(nextId++, path), focus: "split" };
      }),
    })),

  closeSplit: (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab?.split) return;
    // Playback and peek are keyed by the pane id that is about to go away.
    if (ownsPlayback(tab.split.id)) stopLecturePlayback();
    useSidePanelStore.getState().close(tab.split.id);
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === tabId ? { ...t, split: null, focus: "main" } : t,
      ),
    }));
  },

  toggleSplit: (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab) return;
    if (!tab.split) get().openSplit(tabId);
    else if (tab.focus === "main") get().focusPane(tabId, "split");
    else get().closeSplit(tabId);
  },

  focusPane: (tabId, side) =>
    set((s) => {
      const tab = s.tabs.find((t) => t.id === tabId);
      if (!tab || tab.focus === side || (side === "split" && !tab.split)) return s;
      return { tabs: s.tabs.map((t) => (t.id === tabId ? { ...t, focus: side } : t)) };
    }),

  remember: (entry) =>
    set((s) => ({ closed: [...s.closed, entry].slice(-CLOSED_LIMIT) })),

  reopenTab: () => {
    const { closed } = get();
    const entry = closed[closed.length - 1];
    if (!entry) return;
    set({ closed: closed.slice(0, -1) });
    // Rust makes the page and `useBrowserTabs` adopts it in front; it cannot
    // go back to its old index since the strip only hears of it afterwards.
    if (entry.url != null) {
      void browser.open(entry.url).catch(() => {});
      return;
    }
    if (entry.path == null) return;
    const id = nextId++;
    const tab: AppTab = {
      ...pane(id, entry.path),
      split: entry.split != null ? pane(nextId++, entry.split) : null,
      focus: "main",
    };
    set((s) => {
      const tabs = s.tabs.slice();
      tabs.splice(Math.min(Math.max(entry.index, 0), tabs.length), 0, tab);
      return { tabs, activeId: id };
    });
  },

  closeTab: (id) => {
    const { tabs, activeId } = get();
    const idx = tabs.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const tab = tabs[idx];
    // Browser tabs were remembered by the strip already; an empty new-tab page
    // is what ⌘T makes, so neither joins the stack.
    if (browseId(tab.path) == null && !(tab.path === HOME && !tab.split)) {
      const split = tab.split;
      get().remember({
        index: idx,
        path: tab.path,
        url: null,
        split: split && browseId(split.path) == null ? split.path : null,
      });
    }
    // A lecture keeps playing across tab switches, so closing its tab is what
    // stops it (the prompt was already answered — see `confirmLeavingLecture`).
    if (panesOf(tab).some((p) => ownsPlayback(p.id))) stopLecturePlayback();
    for (const p of panesOf(tab)) useSidePanelStore.getState().close(p.id);
    // The last tab stays but goes back to the new-tab page — the one place
    // this store steers a router, since no neighbour comes forward.
    if (tabs.length <= 1) {
      if (tab.split)
        set({ tabs: [{ ...tab, split: null, focus: "main" }] });
      if (tab.path !== HOME) navigateInTab(id, HOME);
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

export function activeTab(): AppTab | undefined {
  const { tabs, activeId } = useTabStore.getState();
  return tabs.find((t) => t.id === activeId);
}

/** The focused half of the tab in front. Every navigation from outside a
 *  router resolves through this. */
export function activePane(): PaneState | undefined {
  const tab = activeTab();
  return tab && focusedPane(tab);
}

/** The pane a peek, a Recent entry or a playing lecture belongs to right now.
 *  `0` is no pane — the id no tab ever has. */
export function useActivePaneId(): number {
  return useTabStore((s) => {
    const tab = s.tabs.find((t) => t.id === s.activeId);
    return tab ? focusedPane(tab).id : 0;
  });
}

/** The focused pane's path — what the sidebar highlights against. */
export function useActivePath(): string {
  return useTabStore((s) => {
    const tab = s.tabs.find((t) => t.id === s.activeId);
    return tab ? focusedPane(tab).path : "";
  });
}

useTabStore.subscribe((s) => {
  try {
    const strip: StoredStrip = {
      tabs: s.tabs.map((t) => ({
        id: t.id,
        path: t.path,
        split: t.split ? { id: t.split.id, path: t.split.path } : null,
        focus: t.focus,
      })),
      activeId: s.activeId,
      closed: s.closed,
    };
    localStorage.setItem(STORE_KEY, JSON.stringify(strip));
  } catch {
    /* quota or private mode — the strip is expendable */
  }
});
