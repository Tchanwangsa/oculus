import { create } from "zustand";
import { browser, browseId } from "@/lib/browser";
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
 * A tab can be **split**: two panes side by side inside the one tab, the
 * second opened with ⌥⌘T (`app/src-tauri/src/menu.rs`). A pane, not a tab —
 * the strip still shows one tab, titled by its main half — because the split
 * is a place to put something *beside* what you are reading, and promoting it
 * to a tab of its own is exactly what you were avoiding by splitting.
 *
 * The unit everything below a tab is keyed by is therefore the **pane id**,
 * not the tab id: a router, a side-panel peek, a playing lecture and a Recent
 * entry all belong to one pane. A tab's main pane uses the tab's own id — so
 * an unsplit tab is indistinguishable from what it was — and a split gets an
 * id of its own from the same counter, which is why `AppTab extends PaneState`
 * rather than holding a pane.
 *
 * Closing one is not quite final: the store keeps a short stack of what was
 * closed (`closed`, `reopenTab`) so ⇧⌘T can put the last one back at the
 * index it had. A browser tab is the exception to the shape of that — see
 * `ClosedTab` — because its route names a page Rust has already destroyed.
 *
 * A browser tab is a tab whose path is `/browse/<id>`: it stands for a native
 * page WebView that Rust holds, and its path never changes — the page
 * navigates, the route does not. What keeps it pinned to its page is
 * `navigateActive` in `app/src/lib/tabRouters.ts`, on the way in. A split
 * pane can hold one too (that is what the split is *for*, half the time), and
 * `useBrowserTabs` reconciles pages against both halves.
 */

/** Which half of a split tab. An unsplit tab is all `"main"`. */
export type PaneSide = "main" | "split";

/** One mounted pane: a router, a page, and how far its history reaches. */
export interface PaneState {
  /** Unique across every pane in the window, never reused. A tab's main pane
   *  carries the tab's own id. */
  id: number;
  path: string;
  /** Its own history's reach, maintained by its pane — a memory router has no
   *  `window.history` for the strip's arrows to read. */
  canBack: boolean;
  canForward: boolean;
}

export interface AppTab extends PaneState {
  /** The pane beside the main one, or null when the tab is not split. */
  split: PaneState | null;
  /** The half the shell drives — sidebar rows, ⌘K, breadcrumbs, the strip's
   *  arrows. Set by clicking into a pane; see `focusPane`. */
  focus: PaneSide;
}

/** Where a tab's history stands, as its pane sees it. */
export interface TabHistory {
  canBack: boolean;
  canForward: boolean;
}

/** Where the last tab goes when it is closed — the new-tab page, which is
 *  what a tab with nothing to show is (`app/src/pages/NewTabPage.tsx`). It is
 *  also where a fresh split opens, for the same reason: a half you just
 *  opened has nothing in it yet and should ask where it is going. */
const HOME = "/new";
/** Where a first-run strip opens. */
const FIRST = "/chat";

/**
 * The strip survives a reload. It used to come back from the hash URL — one
 * path, but at least a real one — and a memory router restores nothing at all,
 * so without this a dev reload dropped every tab on the floor. Only the ids
 * and paths are worth keeping; a restored pane's history starts empty because
 * it is.
 *
 * `/browse/<id>` panes are written out with the rest and left to
 * `useBrowserTabs` to reconcile: it asks Rust for the live page list on mount
 * and drops the panes whose pages are gone.
 */
const STORE_KEY = "oculus-tabs";

/** How far back ⇧⌘T reaches. Deep enough to undo a tidy-up, shallow enough
 *  that the far end is not a tab from another sitting. */
const CLOSED_LIMIT = 10;

/**
 * A tab that was closed, kept so ⇧⌘T can put it back where it was.
 *
 * A browser tab is remembered by **URL**, not by route: its `/browse/<id>`
 * path names a native page Rust destroyed on the way out, so restoring the
 * path would restore a tab with nothing behind it — the same reason
 * `recentKey` refuses those paths in `app/src/stores/recentTabsStore.ts`.
 * Which half of the entry is filled says which kind it is.
 */
export interface ClosedTab {
  /** Where it sat in the strip, so reopening puts it back rather than at the
   *  end. Clamped on the way out — the strip has moved on since. */
  index: number;
  /** The route its main pane held, or null for a browser tab. */
  path: string | null;
  /** The page it was showing, for a browser tab. */
  url: string | null;
  /** The split half's route, restored with it. A split holding a browser page
   *  is dropped — the tab comes back whole rather than with an empty half. */
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
  /** The reopen stack, kept with the strip: a dev reload that dropped it
   *  would make ⇧⌘T quietly reach one sitting less far than it says. */
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

/** Entries written before there was a stack, or half-written, are dropped:
 *  one of `path`/`url` has to be there or there is nothing to reopen. */
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
        // A focus pointing at a half that did not come back is no focus.
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

/** Ids are handed out to panes, not to tabs, and a split's id came from here
 *  too — so the next one has to clear both halves of every restored tab. */
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
  /** Opens a fresh tab at `path` and makes it active. This *is* navigation
   *  into a new context: the pane is created seeded at that path. */
  addTab: (path: string) => void;
  setActive: (id: number) => void;
  /** Moves a tab to `toIndex`, shifting the others. */
  moveTab: (id: number, toIndex: number) => void;
  /** A pane reporting where its router has landed, and how far its history
   *  now reaches either way. Addressed by **pane** id — either half. */
  setPath: (paneId: number, path: string, history: TabHistory) => void;
  /** Removes a tab, and the split half with it. The neighbour that comes
   *  forward is already sitting at its own path, so there is nothing for the
   *  caller to navigate to. */
  closeTab: (id: number) => void;
  /** Pushes a closed tab onto the reopen stack. `closeTab` calls this for an
   *  app tab; a **browser** tab is remembered by the strip instead, before
   *  Rust is asked to destroy the page — by the time the snapshot comes back
   *  and closes the pane, the URL worth keeping is already gone. */
  remember: (entry: ClosedTab) => void;
  /** ⇧⌘T: puts the most recently closed tab back where it was, in front. A
   *  browser tab reopens by URL, which Rust answers with a page the strip
   *  adopts on the next snapshot — so nothing is added here for that case. */
  reopenTab: () => void;
  /** Splits `tabId` and seeds the new half at `path`, or — already split —
   *  just moves the focus there. */
  openSplit: (tabId: number, path?: string) => void;
  /** Folds the split half away, taking its peek and its playback with it. */
  closeSplit: (tabId: number) => void;
  /** ⌥⌘T: split if whole, close the split if the split is what you are in,
   *  and otherwise take you to the half that is already open. */
  toggleSplit: (tabId: number) => void;
  /** Which half the shell drives. Called on the way into a pane — a pointer
   *  down or a focus landing inside it. */
  focusPane: (tabId: number, side: PaneSide) => void;
}

/** Both halves of a tab, in a list — what anything sweeping panes walks. */
export function panesOf(tab: AppTab): PaneState[] {
  return tab.split ? [tab, tab.split] : [tab];
}

/** The half of `tab` the shell drives. */
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
    // The split half is a pane like any other: a lecture playing in it is
    // stranded by this the way closing a tab strands one, and its peek is its
    // own. Both are keyed by the pane id, which is about to stop existing.
    if (ownsPlayback(tab.split.id)) stopLecturePlayback();
    useSidePanelStore.getState().dropTab(tab.split.id);
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.id === tabId ? { ...t, split: null, focus: "main" } : t,
      ),
    }));
  },

  toggleSplit: (tabId) => {
    const tab = get().tabs.find((t) => t.id === tabId);
    if (!tab) return;
    // Whole → split. Split but you are in the main half → hop across, which is
    // what you meant if you pressed it while looking at the left. Split and
    // already there → fold it away.
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
    // A page, not a route: Rust makes the WebView and `useBrowserTabs` gives
    // it a tab in front on the snapshot that follows. It cannot be put back at
    // its old index — the strip only hears about it once it exists.
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
    // Worth putting back? A browser tab was remembered by the strip on the way
    // in — its path names a page that no longer exists — and an empty new-tab
    // page is what ⌘T already makes, so neither joins the stack.
    if (browseId(tab.path) == null && !(tab.path === HOME && !tab.split)) {
      const split = tab.split;
      get().remember({
        index: idx,
        path: tab.path,
        url: null,
        split: split && browseId(split.path) == null ? split.path : null,
      });
    }
    // A lecture keeps playing when you switch away from its tab, so closing
    // that tab has to be what stops it — nothing downstream can tell the two
    // apart once the pane is gone. Either half can be the one playing. The
    // prompt, where there is one, has already been answered by the time this
    // runs (see `confirmLeavingLecture`).
    if (panesOf(tab).some((p) => ownsPlayback(p.id))) stopLecturePlayback();
    // Whatever this tab had open in the side panel goes with it — again per
    // pane. Ids are never reused, but leaving the entries behind would still
    // leak a row per pane closed for as long as the app runs.
    for (const p of panesOf(tab)) useSidePanelStore.getState().dropTab(p.id);
    // The last tab stays, but goes back to the new-tab page — a sole browser
    // tab whose page is gone has nothing else to show. This is the one place
    // the store steers a router: there is no neighbour to come forward, so the
    // tab has to move. It is also the safety net under `NewTabPage`'s browser
    // door, which closes its own tab: losing the race means this no-ops.
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

/** The tab in front, for the shell. */
export function activeTab(): AppTab | undefined {
  const { tabs, activeId } = useTabStore.getState();
  return tabs.find((t) => t.id === activeId);
}

/** The pane the shell drives: the focused half of the tab in front. Every
 *  navigation from outside a router resolves through this. */
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

/** The path the pane in front is showing — what the sidebar highlights
 *  against, now that it has no router to ask. It follows the *focused* half,
 *  so a sidebar row lights up for whichever pane you are working in, which is
 *  the same one it would navigate. */
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
