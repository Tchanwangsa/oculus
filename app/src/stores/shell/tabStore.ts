import { create } from "zustand";
import { browser, browseId } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lectures/playback";
import { hasTabRouter, dropTabRouter, navigateInTab } from "@/lib/shell/tabRouters";
import {
  clampRatio,
  frontItem,
  pushItem,
  removeItem,
  retargetItem,
  sideFromPaths,
} from "@/lib/shell/sideStack";
import { leaveFront, releaseItem } from "./tabs/lifecycle";
import { panesOf, pane, withTab } from "./tabs/panes";
import type { AppTab, PaneSide, PaneState } from "./tabs/panes";
import { CLOSED_LIMIT, restore, saveStrip, type ClosedTab } from "./tabs/persist";

export type { AppTab, PaneSide, PaneState } from "./tabs/panes";
export { focusedPane, panesOf, sideFront } from "./tabs/panes";
export { activePane, activeTab, useActivePaneId, useActivePath } from "./tabs/selectors";

/**
 * The top tab strip. Each tab is a navigation context of its own (a router
 * and page in `app/src/components/tabs/TabPane.tsx`); this store holds which
 * exist, their order, which is in front, and each pane's path and history
 * reach, as the panes report them.
 *
 * A tab can have a side panel on its right half: a stack of panes, one in
 * front (`app/src/lib/shell/sideStack.ts`). Everything below a tab — router,
 * playing lecture, Recent entry — is keyed by **pane id**; a tab's main pane
 * carries the tab's own id, which is why `AppTab extends PaneState`.
 *
 * A browser tab's path is `/browse/<id>` and never changes: it stands for a
 * native WebView Rust holds (see `navigateActive` in
 * `app/src/lib/shell/tabRouters.ts`).
 */

interface TabHistory {
  canBack: boolean;
  canForward: boolean;
}

/** The new-tab page: where the last closed tab and a fresh side panel land. */
export const HOME = "/new";

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
  /** The ratio a new side panel opens at: the last drag of any tab's seam. */
  sideRatio: number;
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

export const useTabStore = create<TabState>((set, get) => ({
  tabs: initial.tabs,
  activeId: initial.activeId,
  closed: initial.closed,
  sideRatio: initial.sideRatio,

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
    const { side, hit, dropped } = pushItem(
      tab.side,
      path,
      () => nextId++,
      state,
      get().sideRatio,
    );
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
    if (!tab?.side || (tab.side.ratio === r && get().sideRatio === r)) return;
    set((s) => ({
      ...withTab(s, tabId, (t) => (t.side ? { ...t, side: { ...t.side, ratio: r } } : t)),
      sideRatio: r,
    }));
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
      side: sideFromPaths(entry.side ?? [], () => nextId++, get().sideRatio),
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

useTabStore.subscribe(saveStrip);
