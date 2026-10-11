import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

export interface TabMenuActions {
  newTab: () => void;
  side: () => void;
  reopenTab: () => void;
  closeActive: () => void;
  selectTab: (index: number) => void;
  selectLastTab: () => void;
  go: (delta: 1 | -1) => void;
  stepSide: (delta: 1 | -1) => void;
}

/**
 * Tab shortcuts arrive as menu events (`app/src-tauri/src/shell/menu.rs`): macOS
 * gives the menu bar every ⌘-key first, which also makes them work while a
 * browser page's native WebView holds focus.
 */
export function useTabMenuEvents(actions: TabMenuActions) {
  const menuActions = useRef(actions);
  menuActions.current = actions;

  useEffect(() => {
    const pending = [
      listen("menu-new-tab", () => menuActions.current.newTab()),
      listen("menu-close-tab", () => menuActions.current.closeActive()),
      listen("menu-reopen-tab", () => menuActions.current.reopenTab()),
      listen<number>("menu-select-tab", (e) =>
        menuActions.current.selectTab(e.payload),
      ),
      listen("menu-last-tab", () => menuActions.current.selectLastTab()),
      listen("menu-side-panel", () => menuActions.current.side()),
      listen("menu-side-next", () => menuActions.current.stepSide(1)),
      listen("menu-side-prev", () => menuActions.current.stepSide(-1)),
      listen("menu-back", () => menuActions.current.go(-1)),
      listen("menu-forward", () => menuActions.current.go(1)),
    ];
    return () => {
      for (const p of pending) p.then((un) => un()).catch(() => {});
    };
  }, []);
}
