import type { DataRouter } from "react-router-dom";
import { browseId } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { confirmLeavingLecture } from "@/stores/leaveLectureStore";
import { closeActivePanel } from "@/stores/sidePanelStore";
import { useTabStore } from "@/stores/tabStore";

/**
 * Every tab's router, addressed by tab id.
 *
 * Each tab owns a memory router of its own so that leaving a tab no longer
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

const routers = new Map<number, DataRouter>();

export function registerTabRouter(id: number, router: DataRouter): void {
  routers.set(id, router);
}

export function unregisterTabRouter(id: number): void {
  routers.delete(id);
}

/** Navigates one tab, wherever it is in the strip. */
export function navigateInTab(id: number, path: string): void {
  routers.get(id)?.navigate(path);
}

/**
 * How the shell navigates: the active tab goes to `path`.
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
  const active = tabs.find((t) => t.id === activeId);
  if (!active || (browseId(active.path) != null && browseId(path) == null)) {
    addTab(path);
    return;
  }
  const go = () => {
    closeActivePanel();
    navigateInTab(active.id, path);
  };
  // Switching tabs leaves a lecture playing behind a tab you can come back to;
  // navigating the tab it is *in* strands it, so that one asks first. Either
  // player counts here: the page's goes off screen with the route, and the
  // peek's is shut by the `closeActivePanel` above, which stops it.
  // `confirmLeavingLecture` goes straight through unless a lecture is actually
  // playing, which is every other navigation in the app — a paused lecture has
  // never prompted and still doesn't.
  if (ownsPlayback(active.id)) {
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
  const { activeId } = useTabStore.getState();
  const go = () => routers.get(activeId)?.navigate(delta);
  if (ownsPlayback(activeId, "page")) {
    confirmLeavingLecture(() => {
      stopLecturePlayback();
      go();
    });
    return;
  }
  go();
}
