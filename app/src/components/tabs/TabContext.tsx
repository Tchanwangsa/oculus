import { createContext, useContext } from "react";

/**
 * Which tab a page is rendering in, and whether that tab is the one in front.
 *
 * Every tab is mounted at once, so a page can no longer assume that being
 * rendered means being looked at: work that only makes sense on screen —
 * polling, placing a native browser page, measuring — has to ask. The pane
 * provides this; pages deep in the tree consume it.
 */
export interface TabContextValue {
  id: number;
  active: boolean;
}

/** Rendered outside any pane there is nothing in front of you, so `active`.
 *  The id is 0, which no tab ever has. */
const DETACHED: TabContextValue = { id: 0, active: true };

export const TabContext = createContext<TabContextValue>(DETACHED);

export function useTabId(): number {
  return useContext(TabContext).id;
}

export function useTabActive(): boolean {
  return useContext(TabContext).active;
}
