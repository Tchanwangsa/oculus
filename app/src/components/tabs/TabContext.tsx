import { createContext, useContext } from "react";
import type { PaneSide } from "@/stores/tabStore";

/**
 * Which pane a page is rendering in, and whether that pane's tab is the one in
 * front.
 *
 * Every tab is mounted at once, so a page can no longer assume that being
 * rendered means being looked at: work that only makes sense on screen —
 * polling, placing a native browser page, measuring — has to ask. The pane
 * provides this; pages deep in the tree consume it.
 *
 * `id` is the **pane** id, which is what a router, a side-panel peek, a
 * playing lecture and a Recent entry are all keyed by. On an unsplit tab it is
 * the tab's own id, so nothing that only ever knew about tabs had to change.
 * `tabId` is for the handful of callers that mean the strip's tab — closing
 * it, splitting it — rather than the page they are drawn in.
 */
export interface TabContextValue {
  id: number;
  tabId: number;
  side: PaneSide;
  active: boolean;
}

/** Rendered outside any pane there is nothing in front of you, so `active`.
 *  The ids are 0, which no tab and no pane ever has. */
const DETACHED: TabContextValue = { id: 0, tabId: 0, side: "main", active: true };

export const TabContext = createContext<TabContextValue>(DETACHED);

/** This pane's id — the key for anything scoped to one page's context. */
export function useTabId(): number {
  return useContext(TabContext).id;
}

/** The strip tab this pane belongs to, and which half of it this is. */
export function usePaneTab(): { tabId: number; side: PaneSide } {
  const { tabId, side } = useContext(TabContext);
  return { tabId, side };
}

export function useTabActive(): boolean {
  return useContext(TabContext).active;
}
