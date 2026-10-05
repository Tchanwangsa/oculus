import { create } from "zustand";
import { browser, browseId } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { dropTabRouter, hasTabRouter, navigateInTab } from "@/lib/tabRouters";
import {
  clampRatio,
  frontItem,
  frontOf,
  pushItem,
  removeItem,
  restoreClosedSide,
  restoreFocus,
  restoreSide,
  retargetItem,
  sideFromPaths,
  storeSide,
  type SideItem,
  type SidePanel,
  type StoredSide,
} from "@/lib/sideStack";

/**
 * The top tab strip. Each tab is a navigation context of its own (a router
 * and page in `app/src/components/tabs/TabPane.tsx`); this store holds which
 * exist, their order, which is in front, and each pane's path and history
 * reach, as the panes report them.
 *
 * A tab can have a side panel on its right half: a stack of panes, one in
 * front (`app/src/lib/sideStack.ts`). Everything below a tab — router,
 * playing lecture, Recent entry — is keyed by **pane id**; a tab's main pane
 * carries the tab's own id, which is why `AppTab extends PaneState`.
 *
 * A browser tab's path is `/browse/<id>` and never changes: it stands for a
 * native WebView Rust holds (see `navigateActive` in
 * `app/src/lib/tabRouters.ts`).
 */

/** Which part of a tab: its main pane, or its side panel. */
export type PaneSide = "main" | "side";

/** One pane: a router, a page, and how far its history reaches. */
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
  /** Location state for the main pane router's first entry (a file's
   *  `locate`), as `SideItem.entryState`. Never stored: a restored tab opens
   *  on its path alone. */
  entryState?: unknown;
  /** The side panel, or null when it is closed. */
  side: SidePanel | null;
  /** The part the shell drives — sidebar rows, ⌘K, breadcrumbs, the strip's
   *  arrows. */
  focus: PaneSide;
}

interface TabHistory {
  canBack: boolean;
  canForward: boolean;
}

/** The new-tab page: where the last closed tab and a fresh side panel land. */
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
  /** The side panel's routes in list order. Browser items are dropped. */
  side: string[] | null;
}

interface StoredPane {
  id: number;
  path: string;
}

/** Fields are read as untrusted: `restoreSide` validates them. */
interface StoredTab extends StoredPane {
  side?: StoredSide | null;
  /** A one-pane side panel, read when `side` is absent. */
  split?: unknown;
  focus?: unknown;
}

/** A stored closed tab, whose side panel may be a lone `split` path. */
type StoredClosed = Partial<Record<keyof ClosedTab | "split", unknown>>;

interface StoredStrip {
  tabs: StoredTab[];
  activeId: number;
  closed?: StoredClosed[];
}

function pane(id: number, path: string): PaneState {
  return { id, path, canBack: false, canForward: false };
}

/** Generic so filtering a `StoredTab[]` does not narrow its elements down to
 *  the pane fields they share. */
function validPane<T extends StoredPane>(p: T | null | undefined): p is T {
  return !!p && Number.isInteger(p.id) && typeof p.path === "string";
}

function validClosed(e: StoredClosed | null | undefined): boolean {
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
      index: e.index as number,
      path: typeof e.path === "string" ? e.path : null,
      url: typeof e.url === "string" ? e.url : null,
      side: restoreClosedSide(e.side, e.split),
    }));
    const tabs = (saved?.tabs ?? []).filter(validPane).map((t): AppTab => {
      const side = restoreSide(t.side, t.split, t.id);
      return { ...pane(t.id, t.path), side, focus: restoreFocus(t.focus, side) };
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
    tabs: [{ ...pane(1, FIRST), side: null, focus: "main" }],
    activeId: 1,
    closed,
  };
}

const initial = restore();

/** Pane ids share one counter, so it has to clear every restored pane, side
 *  panel items included. */
let nextId =
  Math.max(
    0,
    ...initial.tabs.flatMap((t) => panesOf(t).map((p) => p.id)),
  ) + 1;

interface TabState {
  tabs: AppTab[];
  activeId: number;
  /** Closed tabs, newest last — the stack ⇧⌘T pops. */
  closed: ClosedTab[];
  /** Opens `path` in a new tab in front, its first entry carrying `state`.
   *  Returns the new tab's id. */
  addTab: (path: string, state?: unknown) => number;
  setActive: (id: number) => void;
  moveTab: (id: number, toIndex: number) => void;
  /** A pane reporting where its router landed. Addressed by **pane** id. */
  setPath: (paneId: number, path: string, history: TabHistory) => void;
  /** Removes a tab and its side panel. */
  closeTab: (id: number) => void;
  /** Pushes onto the reopen stack. A browser tab is remembered by the strip
   *  before Rust destroys the page, since its URL is gone by the time the
   *  pane closes. */
  remember: (entry: ClosedTab) => void;
  /** ⇧⌘T. A browser tab reopens by URL; `useBrowserTabs` adopts the page. */
  reopenTab: () => void;
  /** Opens a side panel on `/new`, or — already open — focuses it. */
  openSide: (tabId: number) => void;
  /** Closes the side panel and clears its stack. */
  closeSide: (tabId: number) => void;
  /** ⌥⌘T and the strip button: open the side panel if closed, else close it. */
  toggleSide: (tabId: number) => void;
  /** Opens `path` in the side panel and focuses it (`pushItem`): a new item,
   *  or the one naming the same thing, brought to the front and navigated
   *  there with `state`. */
  pushSide: (tabId: number, path: string, state?: unknown) => void;
  /** Brings an item to the front, keeping its place in the list. */
  frontSide: (tabId: number, itemId: number) => void;
  /** Removes one item; the last one closes the side panel. `handover`: its
   *  page continues in another pane, so a lecture keeps playing and a browser
   *  page stays open. */
  removeSide: (tabId: number, itemId: number, opts?: { handover?: boolean }) => void;
  setSideRatio: (tabId: number, ratio: number) => void;
  focusPane: (tabId: number, side: PaneSide) => void;
}

/** The main pane and every side panel item, in front or not. */
export function panesOf(tab: AppTab): PaneState[] {
  return tab.side ? [tab, ...tab.side.items] : [tab];
}

/** The side panel item on show, or null with the side panel closed. */
export function sideFront(tab: AppTab): SideItem | null {
  return tab.side && frontOf(tab.side);
}

export function focusedPane(tab: AppTab): PaneState {
  return (tab.focus === "side" && sideFront(tab)) || tab;
}

/** A side item that is going away: its router is dropped and, unless its page
 *  is handed over to another pane, its lecture stops and its browser page
 *  closes — left open, `useBrowserTabs` would adopt it as a new strip tab. */
function releaseItem(item: PaneState, handover = false): void {
  dropTabRouter(item.id);
  if (handover) return;
  if (ownsPlayback(item.id)) stopLecturePlayback();
  const page = browseId(item.path);
  if (page != null) browser.close(page).catch(() => {});
}

/** A lecture sent to the back of the side panel pauses: only the front item
 *  is mounted to show it. */
function leaveFront(side: SidePanel, next: SidePanel | null): void {
  const was = side.front;
  if (next?.front !== was && ownsPlayback(was)) stopLecturePlayback();
}

/** The store update replacing tab `tabId` with `fn` of it. */
function withTab(s: { tabs: AppTab[] }, tabId: number, fn: (t: AppTab) => AppTab) {
  return { tabs: s.tabs.map((t) => (t.id === tabId ? fn(t) : t)) };
}

export const useTabStore = create<TabState>((set, get) => ({
  tabs: initial.tabs,
  activeId: initial.activeId,
  closed: initial.closed,

  addTab: (path, state) => {
    const id = nextId++;
    const tab: AppTab = { ...pane(id, path), side: null, focus: "main" };
    if (state !== undefined) tab.entryState = state;
    set((s) => ({ tabs: [...s.tabs, tab], activeId: id }));
    return id;
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
          const side = t.side!;
          return {
            ...t,
            side: {
              ...side,
              items: side.items.map((i) =>
                i.id === paneId ? { ...i, path, ...history } : i,
              ),
            },
          };
        }),
      };
    }),

  openSide: (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab) return;
    if (tab.side) get().focusPane(tabId, "side");
    else get().pushSide(tabId, HOME);
  },

  closeSide: (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab?.side) return;
    for (const item of tab.side.items) releaseItem(item);
    set((s) => withTab(s, tabId, (t) => ({ ...t, side: null, focus: "main" })));
  },

  toggleSide: (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab) return;
    if (tab.side) get().closeSide(tabId);
    else get().openSide(tabId);
  },

  pushSide: (tabId, path, state) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab) return;
    const { side, hit, dropped } = pushItem(tab.side, path, () => nextId++, state);
    for (const item of dropped) releaseItem(item);
    if (tab.side) leaveFront(tab.side, side);
    // A hit with a router is navigated once the store holds it; one never
    // mounted yet is pointed at `path` for when its router is built.
    const live = hit != null && hasTabRouter(hit.id);
    const next = hit && !live ? retargetItem(side, hit.id, path, state) : side;
    set((s) => withTab(s, tabId, (t) => ({ ...t, side: next, focus: "side" })));
    // `replace` when already there, so re-citing the same file doesn't pile
    // up history.
    if (live) navigateInTab(hit.id, path, { state, replace: hit.path === path });
  },

  frontSide: (tabId, itemId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab?.side) return;
    const side = frontItem(tab.side, itemId);
    if (side === tab.side) return;
    leaveFront(tab.side, side);
    set((s) => withTab(s, tabId, (t) => ({ ...t, side })));
  },

  removeSide: (tabId, itemId, opts) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    const item = tab?.side?.items.find((i) => i.id === itemId);
    if (!tab?.side || !item) return;
    releaseItem(item, opts?.handover);
    const side = removeItem(tab.side, itemId);
    set((s) =>
      withTab(s, tabId, (t) => ({ ...t, side, focus: side ? t.focus : "main" })),
    );
  },

  setSideRatio: (tabId, ratio) => {
    const r = clampRatio(ratio);
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab?.side || tab.side.ratio === r) return;
    set((s) =>
      withTab(s, tabId, (t) => (t.side ? { ...t, side: { ...t.side, ratio: r } } : t)),
    );
  },

  focusPane: (tabId, side) =>
    set((s) => {
      const tab = s.tabs.find((t) => t.id === tabId);
      if (!tab || tab.focus === side || (side === "side" && !tab.side)) return s;
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
      side: sideFromPaths(entry.side ?? [], () => nextId++),
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
    if (browseId(tab.path) == null && !(tab.path === HOME && !tab.side)) {
      const side = (tab.side?.items ?? [])
        .map((i) => i.path)
        .filter((p) => browseId(p) == null);
      get().remember({
        index: idx,
        path: tab.path,
        url: null,
        side: side.length > 0 ? side : null,
      });
    }
    // A lecture keeps playing across tab switches, so closing its tab is what
    // stops it (the prompt was already answered — see `confirmLeavingLecture`).
    if (panesOf(tab).some((p) => ownsPlayback(p.id))) stopLecturePlayback();
    // The last tab stays but goes back to the new-tab page — the one place
    // this store steers a router, since no neighbour comes forward.
    if (tabs.length <= 1) {
      if (tab.side) {
        for (const item of tab.side.items) releaseItem(item);
        set({ tabs: [{ ...tab, side: null, focus: "main" }] });
      }
      if (tab.path !== HOME) navigateInTab(id, HOME);
      return;
    }
    // The main pane's browser page is the strip's to close (`TopTabBar`).
    dropTabRouter(tab.id);
    for (const item of tab.side?.items ?? []) releaseItem(item);
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

/** The focused pane of the tab in front. Every navigation from outside a
 *  router resolves through this. */
export function activePane(): PaneState | undefined {
  const tab = activeTab();
  return tab && focusedPane(tab);
}

/** `activePane`'s id, reactively. `0` is no pane — the id no tab ever has. */
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
        side: t.side && storeSide(t.side),
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
