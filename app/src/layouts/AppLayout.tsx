import { useState, useEffect, useCallback } from "react";
import { Outlet } from "react-router-dom";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import Sidebar from "@/components/sidebar/Sidebar";
import TopTabBar from "@/components/tabs/TopTabBar";
import FilePeek from "@/components/peek/FilePeek";
import { TooltipProvider } from "@/components/ui/tooltip";

const SIDEBAR_KEY = "oculus-sidebar-collapsed";
const ZOOM_KEY = "oculus-zoom";
// The whole window renders at this scale by default — the UI was drawn a
// touch small for a desktop app.
const DEFAULT_ZOOM = 1.15;
const ZOOM_MIN = 0.7;
const ZOOM_MAX = 1.8;

export default function AppLayout() {
  const [collapsed, setCollapsed] = useState<boolean>(() => {
    return localStorage.getItem(SIDEBAR_KEY) === "true";
  });
  const [zoom, setZoom] = useState<number>(() => {
    const stored = Number(localStorage.getItem(ZOOM_KEY));
    return stored >= ZOOM_MIN && stored <= ZOOM_MAX ? stored : DEFAULT_ZOOM;
  });

  useEffect(() => {
    localStorage.setItem(SIDEBAR_KEY, String(collapsed));
  }, [collapsed]);

  // Zoom is the webview's own page zoom, not a CSS `zoom` on a container.
  // WebKit reports pointer coordinates in visual pixels but element rects in
  // layout pixels inside a CSS-zoomed subtree, so anything that mixes the two
  // — Radix popup collision/positioning, drag maths — lands off by the zoom
  // factor. Page zoom scales the viewport itself, so every measurement stays
  // in one space. The var is only for chrome that must stay at device size.
  useEffect(() => {
    localStorage.setItem(ZOOM_KEY, String(zoom));
    document.documentElement.style.setProperty("--app-zoom", String(zoom));
    getCurrentWebview()
      .setZoom(zoom)
      .catch(() => {});
  }, [zoom]);

  const toggle = useCallback(() => setCollapsed((c) => !c), []);

  // ⌘\ toggles the sidebar; ⌘+/⌘− zoom the whole window; ⌘0 resets to the
  // default scale.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      switch (e.key) {
        case "\\":
          e.preventDefault();
          toggle();
          break;
        case "=":
        case "+":
          e.preventDefault();
          setZoom((z) => Math.min(ZOOM_MAX, Math.round((z + 0.1) * 100) / 100));
          break;
        case "-":
          e.preventDefault();
          setZoom((z) => Math.max(ZOOM_MIN, Math.round((z - 0.1) * 100) / 100));
          break;
        case "0":
          e.preventDefault();
          setZoom(DEFAULT_ZOOM);
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [toggle]);

  return (
    /* 500ms so pointing at a toolbar icon on the way somewhere else doesn't
       flash a tooltip; shadcn's default of 0 is far too eager for a desktop
       app whose controls are mostly icon-only. */
    <TooltipProvider delayDuration={500}>
      <div className="flex flex-col h-full w-full overflow-hidden bg-background">
        {/* Window title bar: traffic lights + back/forward + tabs, full width. */}
        <TopTabBar sidebarCollapsed={collapsed} onToggleSidebar={toggle} />
        <div className="flex flex-1 overflow-hidden">
          <Sidebar collapsed={collapsed} onToggle={toggle} />
          {/* `relative` anchors the peek slide-overs (file peek here, the
              lecture peek rendered deeper) so they span the whole page. */}
          <main className="flex-1 overflow-hidden min-w-0 relative">
            <Outlet />
            <FilePeek />
          </main>
        </div>
      </div>
    </TooltipProvider>
  );
}
