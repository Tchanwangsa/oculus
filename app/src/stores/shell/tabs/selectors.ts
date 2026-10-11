import { useTabStore } from "../tabStore";
import { focusedPane, type AppTab, type PaneState } from "./panes";

export function activeTab(): AppTab | undefined {
  const { tabs, activeId } = useTabStore.getState();
  return tabs.find((t) => t.id === activeId);
}

/** The focused pane of the tab in front. Every navigation from outside a
 *  router resolves through this. */
export function activePane(): PaneState | undefined {
  const tab = activeTab();
  return tab && focusedPane(tab);
}

/** `activePane`'s id, reactively. `0` is no pane — the id no tab ever has. */
export function useActivePaneId(): number {
  return useTabStore((s) => {
    const tab = s.tabs.find((t) => t.id === s.activeId);
    return tab ? focusedPane(tab).id : 0;
  });
}

/** The focused pane's path — what the sidebar highlights against. */
export function useActivePath(): string {
  return useTabStore((s) => {
    const tab = s.tabs.find((t) => t.id === s.activeId);
    return tab ? focusedPane(tab).path : "";
  });
}
