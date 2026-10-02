import type { DataRouter } from "react-router-dom";
import { browseId, browsePath, browser, openExternal } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { confirmLeavingLecture } from "@/stores/leaveLectureStore";
import { closeActivePanel } from "@/stores/sidePanelStore";
import { activeTab, activePane, focusedPane, useTabStore } from "@/stores/tabStore";

/**
 * Every pane's memory router (`app/src/components/tabs/TabPane.tsx`), so the
 * shell — sidebar, tab strip, ⌘K — which sits outside all of them, can
 * navigate a pane by id. Not reactive: what the strip renders from is
 * reported into `tabStore` by the pane.
 */

/** Keyed by **pane** id: a split tab has two routers. */
const routers = new Map<number, DataRouter>();

export function registerTabRouter(id: number, router: DataRouter): void {
  routers.set(id, router);
}

export function unregisterTabRouter(id: number): void {
  routers.delete(id);
}

export function navigateInTab(paneId: number, path: string): void {
  routers.get(paneId)?.navigate(path);
}

/**
 * How the shell (including `components/subjects/SubjectCrumbs.tsx`) navigates:
 * the focused pane of the tab in front goes to `path`, closing its side-panel
 * peek.
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
  // split half navigates in place: `useBrowserTabs` re-adopts the orphaned
  // page as a tab of its own.
  if (browseId(pane.path) != null && browseId(path) == null && pane.id === tab.id) {
    addTab(path);
    return;
  }
  const go = () => {
    closeActivePanel();
    navigateInTab(pane.id, path);
  };
  // Either player counts: the page's leaves with the route, the peek's with
  // `closeActivePanel`. A paused lecture passes straight through.
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
 *  Rust). Only the **page's** player prompts: an arrow leaves the side panel,
 *  and so a peek's lecture, open. */
export function goInActiveTab(delta: 1 | -1): void {
  const pane = activePane();
  if (!pane) return;
  const go = () => routers.get(pane.id)?.navigate(delta);
  if (ownsPlayback(pane.id, "page")) {
    confirmLeavingLecture(() => {
      stopLecturePlayback();
      go();
    });
    return;
  }
  go();
}

/** Opens a web page in the focused split half, or as a tab of its own. In
 *  the split case the pane takes the page's route and any strip tab the
 *  reconcile already made for it is dropped; either order works. */
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
  // The main half: the tab the reconcile makes is the answer.
  if (!tab || !pane || pane.id === tab.id) return;
  navigateInTab(pane.id, browsePath(pageId));
  const stray = useTabStore
    .getState()
    .tabs.find((t) => t.id !== tab.id && browseId(t.path) === pageId);
  // `closeTab`, not `browser.close`: only the stand-in strip tab goes.
  if (stray) useTabStore.getState().closeTab(stray.id);
}
