import { useState, useEffect, useCallback } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import Sidebar from "@/components/sidebar/Sidebar";
import TopTabBar from "@/components/tabs/TopTabBar";
import TabPane from "@/components/tabs/TabPane";
import { SidePanel } from "@/components/panel/SidePanel";
import CommandPalette from "@/components/palette/CommandPalette";
import { TooltipProvider } from "@/components/ui/tooltip";
import { LeaveLectureDialog } from "@/components/lectures/LeaveLectureDialog";
import { browser, isWebUrl } from "@/lib/browser";
import { useBrowserTabs } from "@/hooks/useBrowserTabs";
import { useTabStore } from "@/stores/tabStore";

const SIDEBAR_KEY = "oculus-sidebar-collapsed";
const ZOOM_KEY = "oculus-zoom";
// The whole window renders at this scale by default — the UI was drawn a
// touch small for a desktop app.
const DEFAULT_ZOOM = 1.15;
const ZOOM_MIN = 0.7;
const ZOOM_MAX = 1.8;

/**
 * The shell: the furniture around the pages, and the one place every tab is
 * mounted. It is not a route element — there is a router per tab below it
 * (`app/src/components/tabs/TabPane.tsx`), so the sidebar, the strip and the
 * palette all sit outside every one of them and navigate through
 * `app/src/lib/tabRouters.ts`.
 */
export default function AppLayout() {
  const tabs = useTabStore((s) => s.tabs);
  const activeId = useTabStore((s) => s.activeId);
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

  // Browser tabs opened in Rust arrive in the tab strip through this.
  useBrowserTabs();

  // Every external link in the app opens in an in-app browser tab instead
  // of leaving for Safari — caught here, in the capture phase, so no call
  // site has to know: an `<a href="https://…">` anywhere, markdown included,
  // just works. ⌘-click still hands the URL to the real browser.
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.defaultPrevented || e.button !== 0) return;
      const anchor = (e.target as HTMLElement | null)?.closest?.("a[href]") as
        | HTMLAnchorElement
        | null;
      const href = anchor?.getAttribute("href");
      if (!isWebUrl(href)) return;
      e.preventDefault();
      if (e.metaKey || e.ctrlKey) {
        browser.external(href).catch(() => {});
        return;
      }
      browser.open(href).catch(() => {});
    };
    document.addEventListener("click", onClick, true);
    return () => document.removeEventListener("click", onClick, true);
  }, []);

  // ⌘B toggles the sidebar; ⌘+/⌘− zoom the whole window; ⌘0 resets to the
  // default scale.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      // ⌥ is not part of any of these, and saying so is what keeps ⌘⌥B — the
      // chat page's own panel, in `ChatPage` — from also closing the sidebar.
      if (e.altKey) return;
      if (!(e.metaKey || e.ctrlKey)) return;
      switch (e.key) {
        // Both cases: with caps lock on, `key` is the capital.
        case "b":
        case "B":
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
        {/* `gap-2` survives the sidebar collapsing to zero width, so the card
            keeps its left inset either way. */}
        <div className="flex flex-1 overflow-hidden gap-2 pb-2 pr-2">
          <Sidebar collapsed={collapsed} onToggle={toggle} />
          {/* The document floats: content is a rounded card inset from the
              ground the shell sits on, so the sidebar and tab strip read as
              furniture around the page rather than panels beside it. That
              also means the sidebar needs no divider of its own — this card's
              border is the separation.
              The card is a row: the panes on the left, the side panel docked
              against its right edge. `relative` sits on the pane stack rather
              than here, because that is what every tab is positioned against —
              all of them mounted and filling it, with only the one in front
              visible. */}
          <main className="flex-1 overflow-hidden min-w-0 flex rounded-xl border border-border bg-card shadow-panel">
            <div className="relative flex-1 min-w-0">
              {tabs.map((tab) => (
                <TabPane key={tab.id} tab={tab} active={tab.id === activeId} />
              ))}
            </div>
            <SidePanel />
          </main>
        </div>
        {/* Raised from the tab strip and from the player alike, so it hangs
            here rather than in either of them. */}
        <LeaveLectureDialog />
        {/* ⌘K, over everything. It listens for the menu event itself; the
            sidebar's Search row is the other way in. */}
        <CommandPalette />
      </div>
    </TooltipProvider>
  );
}
