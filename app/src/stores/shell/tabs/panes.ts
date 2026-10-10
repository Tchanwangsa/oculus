import { frontOf, type SideItem, type SidePanel } from "@/lib/shell/sideStack";

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
  /** The part the shell drives — the sidebar, ⌘K, breadcrumbs, the strip's
   *  arrows. */
  focus: PaneSide;
}

export function pane(id: number, path: string): PaneState {
  return { id, path, canBack: false, canForward: false };
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

/** The store update replacing tab `tabId` with `fn` of it. */
export function withTab(s: { tabs: AppTab[] }, tabId: number, fn: (t: AppTab) => AppTab) {
  return { tabs: s.tabs.map((t) => (t.id === tabId ? fn(t) : t)) };
}
