import type { DataRouter } from "react-router-dom";
import { browseId, browsePath, browser, openExternal } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lectures/playback";
import { lecturePageId } from "@/lib/lectures";
import { claimPlayback } from "@/lib/lectures/playbackOwner";
import { confirmLeavingLecture } from "@/stores/shell/leaveLectureStore";
import { cancelRecentTab, recordRecentTab } from "@/stores/shell/recentTabsStore";
import {
  activeTab,
  activePane,
  focusedPane,
  sideFront,
  useTabStore,
} from "@/stores/shell/tabStore";

/**
 * Every pane's memory router, so the shell — sidebar, tab strip, ⌘K — which
 * sits outside all of them, can navigate a pane by id. A router is built when
 * its pane first mounts (`app/src/components/tabs/TabPane.tsx`) and lives
 * until the pane goes (`dropTabRouter`), not just while it is mounted: a side
 * panel item sent to the back keeps its history, and still reports where it
 * goes. Not reactive: what the strip renders from is reported into `tabStore`.
 */

interface Entry {
  router: DataRouter;
  unsubscribe: () => void;
}

/** Keyed by **pane** id: a tab with a side panel has a router per item. */
const routers = new Map<number, Entry>();

/** The pane's router, built by `create` the first time it is asked for. */
export function routerFor(id: number, create: () => DataRouter): DataRouter {
  const known = routers.get(id);
  if (known) return known.router;
  const router = create();
  routers.set(id, { router, unsubscribe: track(id, router) });
  return router;
}

/**
 * Reports a router's moves to `tabStore` and the Recent trail. Keeps the
 * pane's history (location keys + cursor), since a memory router has no
 * `window.history` index; that yields `canBack`/`canForward`.
 */
function track(id: number, router: DataRouter): () => void {
  let stack = [router.state.location.key];
  let at = 0;
  let seen = stack[0];
  return router.subscribe((state) => {
    // `subscribe` also fires for navigation state; only a new key is a move.
    const key = state.location.key;
    if (key === seen) return;
    seen = key;
    if (state.historyAction === "PUSH") {
      stack = [...stack.slice(0, at + 1), key];
      at = stack.length - 1;
    } else if (state.historyAction === "POP") {
      const i = stack.indexOf(key);
      if (i !== -1) at = i;
    } else {
      stack[at] = key;
    }
    const path = state.location.pathname + state.location.search;
    useTabStore.getState().setPath(id, path, {
      canBack: at > 0,
      canForward: at < stack.length - 1,
    });
    // Keyed by pane id so a page this pane only passed through is dropped.
    recordRecentTab(id, path);
  });
}

export function hasTabRouter(id: number): boolean {
  return routers.has(id);
}

/** For a pane that is gone for good: a closed tab, a removed side item. */
export function dropTabRouter(id: number): void {
  routers.get(id)?.unsubscribe();
  routers.delete(id);
  cancelRecentTab(id);
}

export function navigateInTab(
  paneId: number,
  path: string,
  opts?: { state?: unknown; replace?: boolean },
): void {
  routers.get(paneId)?.router.navigate(path, opts);
}

/**
 * Opens `path` beside the page, in the side panel of the tab in front
 * (`pushSide`). What file rows, citations and lecture rows do; a plain `Link`
 * navigates its own pane instead. From inside the side panel it still pushes,
 * so the page there stays put.
 */
export function openBeside(path: string, state?: unknown): void {
  const { tabs, activeId, addTab, pushSide } = useTabStore.getState();
  if (!tabs.some((t) => t.id === activeId)) {
    addTab(path, state);
    return;
  }
  pushSide(activeId, path, state);
}

/**
 * The side panel's expand: its front item becomes the tab's main page and
 * leaves the stack, keeping its location state (a file's `locate`), its
 * lecture's playback and a browser page — the item is removed with
 * `handover`, so the page stays open. A lecture it owned is claimed for the
 * new pane (`claimPlayback`), so whatever comes to the side panel's front
 * next cannot adopt the elements first; until the new player shows that
 * lecture they play on, parked. `newTab` (⌘-click) opens it as a tab of its
 * own, as does a main pane on a browser page, which holds only that page.
 */
export function expandSide(tabId: number, newTab: boolean): void {
  const { tabs, addTab, setPath, removeSide, focusPane } = useTabStore.getState();
  const tab = tabs.find((t) => t.id === tabId);
  const item = tab && sideFront(tab);
  if (!tab || !item) return;
  // Read before the item's router goes, and once any prompt is answered.
  const where = () => {
    const at = routers.get(item.id)?.router.state.location;
    return at
      ? { path: at.pathname + at.search, state: at.state }
      : { path: item.path, state: item.entryState };
  };
  const handOver = (paneId: number, path: string) => {
    const lectureId = lecturePageId(path);
    if (lectureId && ownsPlayback(item.id)) claimPlayback(paneId, paneId, lectureId);
  };
  if (newTab || browseId(tab.path) != null) {
    const { path, state } = where();
    handOver(addTab(path, state), path);
    removeSide(tabId, item.id, { handover: true });
    return;
  }
  const go = () => {
    const { path, state } = where();
    navigateInTab(tabId, path, { state });
    // The route may load lazily; until it commits, the store says where the
    // main pane is going, or a `browser-state` reconcile would adopt the page.
    if (browseId(path) != null) setPath(tabId, path, { canBack: true, canForward: false });
    handOver(tabId, path);
    removeSide(tabId, item.id, { handover: true });
    focusPane(tabId, "main");
  };
  // The main page's own lecture is left as `navigateActive` leaves it.
  if (ownsPlayback(tabId)) {
    confirmLeavingLecture(() => {
      stopLecturePlayback();
      go();
    });
    return;
  }
  go();
}

/**
 * How the shell (including `components/subjects/SubjectCrumbs.tsx`) navigates:
 * the focused pane of the tab in front goes to `path`. The side panel stays
 * open.
 *
 * A playing lecture is asked about here, before anything is committed, rather
 * than with a router `useBlocker`: an unanswered blocker drops the navigation
 * with nothing on screen and strands the router.
 */
export function navigateActive(path: string): void {
  const { tabs, activeId, addTab } = useTabStore.getState();
  const tab = tabs.find((t) => t.id === activeId);
  if (!tab) {
    addTab(path);
    return;
  }
  const pane = focusedPane(tab);
  // A browser tab holds only its page, so anywhere else opens a new tab. A
  // side panel item navigates in place: `useBrowserTabs` re-adopts the
  // orphaned page as a tab of its own.
  if (browseId(pane.path) != null && browseId(path) == null && pane.id === tab.id) {
    addTab(path);
    return;
  }
  const go = () => navigateInTab(pane.id, path);
  // The pane's lecture leaves with the route; a paused one passes straight
  // through.
  if (ownsPlayback(pane.id)) {
    confirmLeavingLecture(() => {
      stopLecturePlayback();
      go();
    });
    return;
  }
  go();
}

/** The strip's history arrows for an app tab (a browser tab's go through
 *  Rust). They act on the focused pane, whose own lecture prompts. */
export function goInActiveTab(delta: 1 | -1): void {
  const pane = activePane();
  if (!pane) return;
  const go = () => routers.get(pane.id)?.router.navigate(delta);
  if (ownsPlayback(pane.id)) {
    confirmLeavingLecture(() => {
      stopLecturePlayback();
      go();
    });
    return;
  }
  go();
}

/** Opens a web page in the side panel's front item when that is focused, or
 *  as a tab of its own. In the side panel the item takes the page's route and
 *  any strip tab the reconcile already made for it is dropped; either order
 *  works. */
export async function openUrlInFocusedPane(url: string): Promise<void> {
  const tab = activeTab();
  const pane = tab && focusedPane(tab);
  let pageId: number;
  try {
    pageId = await browser.open(url);
  } catch (e) {
    console.error(`[oculus] in-app tab failed for ${url}`, e);
    await openExternal(url, true);
    return;
  }
  // The main pane: the tab the reconcile makes is the answer.
  if (!tab || !pane || pane.id === tab.id) return;
  navigateInTab(pane.id, browsePath(pageId));
  const stray = useTabStore
    .getState()
    .tabs.find((t) => t.id !== tab.id && browseId(t.path) === pageId);
  // `closeTab`, not `browser.close`: only the stand-in strip tab goes.
  if (stray) useTabStore.getState().closeTab(stray.id);
}
