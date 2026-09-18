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
}

function pane(id: number, path: string): PaneState {
  return { id, path, canBack: false, canForward: false };
}

/** Generic so filtering a `StoredTab[]` does not narrow its elements down to
 *  the pane fields they share. */
function validPane<T extends StoredPane>(p: T | null | undefined): p is T {
  return !!p && Number.isInteger(p.id) && typeof p.path === "string";
}

function restore(): { tabs: AppTab[]; activeId: number } {
  try {
    const raw = localStorage.getItem(STORE_KEY);
    const saved = raw ? (JSON.parse(raw) as StoredStrip) : null;
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
      return { tabs, activeId };
    }
  } catch {
    /* corrupt or unavailable — a fresh strip is a fine fallback */
  }
  return { tabs: [{ ...pane(1, FIRST), split: null, focus: "main" }], activeId: 1 };
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

  closeTab: (id) => {
    const { tabs, activeId } = get();
    const idx = tabs.findIndex((t) => t.id === id);
    if (idx === -1) return;
    const tab = tabs[idx];
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
    };
    localStorage.setItem(STORE_KEY, JSON.stringify(strip));
  } catch {
    /* quota or private mode — the strip is expendable */
  }
});
