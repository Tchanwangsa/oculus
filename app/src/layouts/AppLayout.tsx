import { useState, useEffect, useCallback } from "react";
import Sidebar from "@/components/sidebar/Sidebar";
import TopTabBar from "@/components/tabs/TopTabBar";
import TabPane from "@/components/tabs/TabPane";
import CommandPalette from "@/components/palette/CommandPalette";
import { TooltipProvider } from "@/components/ui/tooltip";
import { LeaveLectureDialog } from "@/components/lectures/LeaveLectureDialog";
import { EventDialog } from "@/components/calendar/EventDialog";
import {
  browser,
  browseId,
  isWebUrl,
  stepZoom,
} from "@/lib/browser";
import { useActivityPing } from "@/hooks/backend/useActivityPing";
import { useJobAudit } from "@/hooks/agents/useJobAudit";
import { useBrowserTabs } from "@/hooks/shell/useBrowserTabs";
import { useMusicState } from "@/hooks/music/useMusicState";
import { useDockPages } from "@/hooks/shell/useDockPages";
import { dock, dockService } from "@/lib/dock";
import { useTauriEvent, useWindowEvent } from "@/hooks/backend/useEvents";
import { routeEdit, routeSelectAll } from "@/lib/menu/editRouting";
import { routeFind } from "@/lib/menu/find";
import { containSelection } from "@/lib/ui/selectScope";
import { useNewTabClicks } from "@/lib/shell/newTabClicks";
import { openLink } from "@/lib/shell/tabRouters";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { activePane, useTabStore } from "@/stores/shell/tabStore";
import {
  applyZoom,
  DEFAULT_ZOOM,
  storedZoom,
  ZOOM_MAX,
  ZOOM_MIN,
} from "@/lib/ui/pageZoom";

const SIDEBAR_KEY = "oculus-sidebar-collapsed";

/**
 * The shell, and the one place every tab is mounted. Not a route element: each
 * tab has its own router (`TabPane.tsx`), so the chrome here navigates through
 * `lib/shell/tabRouters.ts`.
 */
export default function AppLayout() {
  const tabs = useTabStore((s) => s.tabs);
  const activeId = useTabStore((s) => s.activeId);
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    return localStorage.getItem(SIDEBAR_KEY) === "true";
  });
  const [zoom, setZoom] = useState<number>(storedZoom);

  useEffect(() => {
    localStorage.setItem(SIDEBAR_KEY, String(collapsed));
  }, [collapsed]);

  useEffect(() => applyZoom(zoom), [zoom]);

  const toggle = useCallback(() => setCollapsed((c) => !c), []);

  useBrowserTabs();

  useMusicState();
  useDockPages();

  useNewTabClicks();

  useActivityPing();

  useJobAudit();

  useEffect(() => containSelection(), []);

  // Capture-phase net: every external `<a href>` opens as an in-app page in
  // the side panel; ⌘-click opens it as a tab of its own (`openLink`).
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.defaultPrevented || e.button !== 0) return;
      const anchor = (e.target as HTMLElement | null)?.closest?.("a[href]") as
        | HTMLAnchorElement
        | null;
      const href = anchor?.getAttribute("href");
      if (!isWebUrl(href)) return;
      e.preventDefault();
      void openLink(href, e.metaKey || e.ctrlKey);
    };
    document.addEventListener("click", onClick, true);
    return () => document.removeEventListener("click", onClick, true);
  }, []);

  /** ⌘= / ⌘− / ⌘0: with a browser page in the focused pane they zoom the
   *  page; anywhere else, the window. */
  const zoomBy = useCallback((direction: 1 | -1 | 0) => {
    const page = browseId(activePane()?.path);
    if (page != null) {
      const current =
        useBrowserStore.getState().tabs.find((t) => t.id === page)?.zoom ?? 1;
      const next = direction === 0 ? 1 : stepZoom(current, direction);
      browser.setZoom(page, next).catch(() => {});
      return;
    }
    if (direction === 0) {
      setZoom(DEFAULT_ZOOM);
      return;
    }
    setZoom((z) =>
      direction > 0
        ? Math.min(ZOOM_MAX, Math.round((z + 0.1) * 100) / 100)
        : Math.max(ZOOM_MIN, Math.round((z - 0.1) * 100) / 100),
    );
  }, []);

  /** ⌘R / ⇧⌘R (ignoring cache) on a browser tab; either reloads a messaging
   *  service. Deliberately a no-op anywhere else. */
  const reload = useCallback((hard: boolean) => {
    const path = activePane()?.path;
    const page = browseId(path);
    if (page != null) browser.reload(page, hard).catch(() => {});
    const service = dockService(path);
    if (service) dock.reload(service.id).catch(() => {});
  }, []);

  // Menu events, not key presses: a browser page's native WebView takes every
  // ⌘-key, so the app's webview never sees them (`app/src-tauri/src/shell/menu.rs`).
  useTauriEvent("menu-zoom-in", () => zoomBy(1));
  useTauriEvent("menu-zoom-out", () => zoomBy(-1));
  useTauriEvent("menu-zoom-reset", () => zoomBy(0));
  useTauriEvent("menu-reload", () => reload(false));
  useTauriEvent("menu-hard-reload", () => reload(true));

  // Edit ▸ Undo / Redo / Select All. The menu owns ⌘Z and ⌘A and emits only
  // while the app's webview has focus (a browser page handles its own).
  useTauriEvent("menu-undo", () => routeEdit(false));
  useTauriEvent("menu-redo", () => routeEdit(true));
  useTauriEvent("menu-select-all", () => routeSelectAll(activePane()?.id));

  // Edit ▸ Find, Find Next, Find Previous: one mounted find answers.
  useTauriEvent("menu-find", () => routeFind("open", activePane()?.id));
  useTauriEvent("menu-find-next", () => routeFind("next", activePane()?.id));
  useTauriEvent("menu-find-prev", () => routeFind("prev", activePane()?.id));

  // ⌘B has no menu item. ⌘+ is here because muda binds the physical key and
  // ⌘+ is ⇧⌘=, which the menu's ⌘= does not match; the unshifted zoom keys
  // never reach here on macOS (the menu bar takes them first). A key an
  // editor already took (the note editor's ⌘B is bold) is left alone.
  useWindowEvent("keydown", (ev) => {
    const e = ev as KeyboardEvent;
    if (e.defaultPrevented) return;
    // Keeps ⌘⌥B (Chat's conversations column) from also toggling the sidebar.
    if (e.altKey) return;
    if (!(e.metaKey || e.ctrlKey)) return;
    switch (e.key) {
      // Caps lock makes `key` the capital.
      case "b":
      case "B":
        e.preventDefault();
        toggle();
        break;
      case "=":
      case "+":
        e.preventDefault();
        zoomBy(1);
        break;
      case "-":
        e.preventDefault();
        zoomBy(-1);
        break;
      case "0":
        e.preventDefault();
        zoomBy(0);
        break;
    }
  });

  return (
    /* shadcn's 0ms flashes tooltips on icons the pointer merely crosses. */
    <TooltipProvider delayDuration={500}>
      {/* No fill: `#root` paints the frame, which may be see-through. */}
      <div className="flex flex-col h-full w-full overflow-hidden">
        <TopTabBar sidebarCollapsed={collapsed} onToggleSidebar={toggle} />
        {/* `gap-2` keeps the card's left inset when the sidebar collapses. */}
        <div className="flex flex-1 overflow-hidden gap-2 pb-2 pr-2">
          <Sidebar collapsed={collapsed} />
          {/* The floating card (see docs/ui.md: UI system). `relative` is
              on the pane stack: every tab is mounted and fills it. */}
          <main className="flex-1 overflow-hidden min-w-0 flex rounded-xl border border-border bg-card shadow-panel">
            <div className="relative flex-1 min-w-0">
              {tabs.map((tab) => (
                <TabPane key={tab.id} tab={tab} active={tab.id === activeId} />
              ))}
            </div>
          </main>
        </div>
        {/* Shell-level dialogs: each is raised from several places. */}
        <LeaveLectureDialog />
        <EventDialog />
        {/* ⌘K; listens for its menu event itself. */}
        <CommandPalette />
      </div>
    </TooltipProvider>
  );
}
