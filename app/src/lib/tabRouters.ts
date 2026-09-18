import type { DataRouter } from "react-router-dom";
import { browseId, browsePath, browser, openExternal } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { confirmLeavingLecture } from "@/stores/leaveLectureStore";
import { closeActivePanel } from "@/stores/sidePanelStore";
import { activeTab, focusedPane, useTabStore } from "@/stores/tabStore";

/**
 * Every pane's router, addressed by pane id.
 *
 * Each pane owns a memory router of its own so that leaving a tab no longer
 * unmounts its page (`app/src/components/tabs/TabPane.tsx`). That puts the
 * shell — the sidebar, the tab strip, the ⌘K palette — *outside* all of them,
 * with no router of its own to navigate: `useNavigate` there would have
 * nothing to resolve against. So the panes publish their routers here and the
 * shell reaches them by id.
 *
 * It is a module singleton for the same reason `lib/lecturePlayback.ts` is one:
 * what it holds outlives every component that touches it, and there is exactly
 * one of it per window. Nothing here is reactive — a router object never
 * changes for the life of its pane, and what the strip *does* need to re-render
 * on (the path, and whether history has anywhere to go) is reported into
 * `tabStore` by the pane instead.
 */

/** Keyed by **pane** id, not tab id: a split tab has two routers, and its
 *  main half carries the tab's own id (see `stores/tabStore.ts`). */
const routers = new Map<number, DataRouter>();

export function registerTabRouter(id: number, router: DataRouter): void {
  routers.set(id, router);
}

export function unregisterTabRouter(id: number): void {
  routers.delete(id);
}

/** Navigates one pane, wherever its tab is in the strip. */
export function navigateInTab(paneId: number, path: string): void {
  routers.get(paneId)?.navigate(path);
}

/**
 * How the shell navigates: the focused pane of the tab in front goes to
 * `path`. On an unsplit tab that is simply the tab.
 *
 * A browser tab can hold nothing but its page — the route names a native page
 * WebView Rust owns and never moves — so a shell click that would take it
 * somewhere else gets a tab of its own instead. This is the rule `tabStore`
 * used to apply from inside `trackNavigation`, which saw every move the one
 * router made; with a router per tab it belongs on the way *in*.
 *
 * Moving the tab in front also shuts its side panel: the peek belongs to the
 * page being left (see `closeActivePanel`). Opening a *new* tab does not,
 * because the tab that stays behind keeps its own page and its own peek.
 *
 * And it is where a playing lecture is asked about. That used to be a
 * `useBlocker` inside the player, which was the wrong shape twice over: a
 * router blocker is answered by whichever component happens to hold that
 * blocker's key, and if nothing answers, the router drops the navigation with
 * **nothing on screen** — a sidebar click that silently does nothing, and a
 * tab that can never navigate again. Asking here instead means the question is
 * posed before anything is committed: there is no blocked router to leave
 * stranded, and the one door the shell navigates through is the one place the
 * rule lives.
 *
 * "The shell" now includes the breadcrumb row, which is drawn inside a pane
 * but is doing the shell's job (`components/subjects/SubjectCrumbs.tsx`). It
 * used to be `Link`s onto the pane's own router, which is how a crumb became
 * the one way out of a lecture that never asked.
 */
export function navigateActive(path: string): void {
  const { tabs, activeId, addTab } = useTabStore.getState();
  const tab = tabs.find((t) => t.id === activeId);
  if (!tab) {
    addTab(path);
    return;
  }
  // The focused half is the one the shell drives, and on an unsplit tab that
  // is the tab itself — so this is the same rule it always was, asked of a
  // pane instead of a tab.
  const pane = focusedPane(tab);
  // A browser pane can hold nothing but its page — the route names a native
  // page WebView Rust owns and never moves. The main half therefore gets a
  // tab of its own for anywhere else. The *split* half does not: navigating it
  // away leaves its page in no pane at all, and `useBrowserTabs` puts that
  // page back in the strip as a tab — so nothing is lost, and the half you
  // were pointing at is the half you get. That is the whole point of having
  // split it.
  if (browseId(pane.path) != null && browseId(path) == null && pane.id === tab.id) {
    addTab(path);
    return;
  }
  const go = () => {
    closeActivePanel();
    navigateInTab(pane.id, path);
  };
  // Switching tabs leaves a lecture playing behind a tab you can come back to;
  // navigating the pane it is *in* strands it, so that one asks first. Either
  // player counts here: the page's goes off screen with the route, and the
  // peek's is shut by the `closeActivePanel` above, which stops it.
  // `confirmLeavingLecture` goes straight through unless a lecture is actually
  // playing, which is every other navigation in the app — a paused lecture has
  // never prompted and still doesn't.
  if (ownsPlayback(pane.id)) {
    confirmLeavingLecture(() => {
      stopLecturePlayback();
      go();
    });
    return;
  }
  go();
}

/**
 * The strip's history arrows, for an app tab. A browser tab's arrows drive
 * the page's own history through Rust and never come here.
 *
 * Guarded like `navigateActive`, and the worry that used to argue against it —
 * a prompt firing on the way *toward* the lecture rather than away from it —
 * turns out not to exist. `ownsPlayback` is only true while the elements are
 * in a player mounted in this tab, which means the tab is *showing* the
 * lecture: both arrows lead away from it, whichever one you press.
 *
 * Asked for the **page's** player alone. The side panel is deliberately left
 * open by an arrow — a step within a page's own history is not a departure
 * from it — so a peek is still on screen and playing afterwards, and a prompt
 * for a lecture that never went anywhere would be the false alarm this guard
 * is meant to avoid.
 */
export function goInActiveTab(delta: 1 | -1): void {
  const { tabs, activeId } = useTabStore.getState();
  const tab = tabs.find((t) => t.id === activeId);
  if (!tab) return;
  const pane = focusedPane(tab);
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

/**
 * Open a web page where the user is looking: in the focused half if the tab is
 * split, and otherwise as a tab of its own, which is what every external link
 * in the app already does.
 *
 * The split case is the whole reason this exists. A page is a native WebView
 * Rust owns and `useBrowserTabs` mirrors into the strip, so left alone it
 * always becomes a *tab* — which is precisely what you were avoiding by
 * splitting. Here the pane navigates to the page's route and the strip tab
 * that may already have been made for it is dropped. Either order works: if
 * the snapshot has not landed yet, the reconcile finds the page held by this
 * pane and makes no tab at all.
 */
export async function openUrlInFocusedPane(url: string): Promise<void> {
  const tab = activeTab();
  const pane = tab && focusedPane(tab);
  let pageId: number;
  try {
    pageId = await browser.open(url);
  } catch (e) {
    // The in-app tab is the preference, not the requirement — the link always
    // goes somewhere.
    console.error(`[oculus] in-app tab failed for ${url}`, e);
    await openExternal(url, true);
    return;
  }
  // The main half: the tab the reconcile makes *is* the answer, already in
  // front. Nothing more to do.
  if (!tab || !pane || pane.id === tab.id) return;
  navigateInTab(pane.id, browsePath(pageId));
  const stray = useTabStore
    .getState()
    .tabs.find((t) => t.id !== tab.id && browseId(t.path) === pageId);
  // `closeTab`, not `browser.close`: the page is not going anywhere, only the
  // strip tab that was standing in for it.
  if (stray) useTabStore.getState().closeTab(stray.id);
}
