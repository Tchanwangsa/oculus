import { createContext, useContext } from "react";
import type { PaneSide } from "@/stores/tabStore";

/**
 * Which pane a page renders in and whether its tab is in front. Every tab is
 * mounted at once, so on-screen-only work (polling, measuring) must check
 * `active`. `id` is the pane id (the tab's id when unsplit); `tabId` is the
 * strip tab, for closing or splitting it.
 */
export interface TabContextValue {
  id: number;
  tabId: number;
  side: PaneSide;
  active: boolean;
}

/** Outside any pane: active, with id 0 (never a real tab or pane). */
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
