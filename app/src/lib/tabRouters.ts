import type { DataRouter } from "react-router-dom";
import { browseId } from "@/lib/browser";
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
 */
export function navigateActive(path: string): void {
  const { tabs, activeId, addTab } = useTabStore.getState();
  const active = tabs.find((t) => t.id === activeId);
  if (!active || (browseId(active.path) != null && browseId(path) == null)) {
    addTab(path);
    return;
  }
  navigateInTab(active.id, path);
}

/** The strip's history arrows, for an app tab. A browser tab's arrows drive
 *  the page's own history through Rust and never come here. */
export function goInActiveTab(delta: 1 | -1): void {
  const { activeId } = useTabStore.getState();
  routers.get(activeId)?.navigate(delta);
}
