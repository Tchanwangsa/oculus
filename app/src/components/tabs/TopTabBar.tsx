import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import {
  CaretLeft,
  CaretRight,
  Plus,
  Sidebar,
  SidebarSimple,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useStripReorder } from "@/hooks/gestures/usePointerDrag";
import { focusedPane, panesOf, useTabStore } from "@/stores/shell/tabStore";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { browser, browseId } from "@/lib/browser";
import { useTabInfo } from "@/components/tabs/tabInfo";
import { stepItem } from "@/lib/shell/sideStack";
import { goInActiveTab } from "@/lib/shell/tabRouters";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useWindowFullscreen } from "@/hooks/shell/useWindowFullscreen";
import { ownsPlayback } from "@/lib/lectures/playback";
import { confirmLeavingLecture } from "@/stores/shell/leaveLectureStore";
import {
  NEW_TAB_PATH,
  SEPARATOR_W,
  TAB_MIN_W,
  TAB_W,
  TRAILING_W,
} from "@/components/tabs/strip/constants";
import { StripTab } from "@/components/tabs/strip/StripTab";
import { useTabMenuEvents } from "@/components/tabs/strip/useTabMenuEvents";

interface TopTabBarProps {
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}

/**
 * The window's title bar, Notion-style: macOS traffic lights (native,
 * overlaid — hence the left inset), the sidebar toggle, history back/forward,
 * then the tab strip. The whole bar is a drag region.
 */
export default function TopTabBar({
  sidebarCollapsed,
  onToggleSidebar,
}: TopTabBarProps) {
  const { tabs, activeId, addTab, setActive, closeTab, toggleSide, frontSide, reopenTab } =
    useTabStore(useShallow((s) => ({
      tabs: s.tabs,
      activeId: s.activeId,
      addTab: s.addTab,
      setActive: s.setActive,
      closeTab: s.closeTab,
      toggleSide: s.toggleSide,
      frontSide: s.frontSide,
      reopenTab: s.reopenTab,
    })));
  const tabInfo = useTabInfo();
  const browserTabs = useBrowserStore((s) => s.tabs);
  const fullscreen = useWindowFullscreen();
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const stripRef = useRef<HTMLDivElement>(null);
  const [stripW, setStripW] = useState(0);
  // Chrome-style reorder: activate on press, lift past the threshold, swap as
  // the centre crosses a neighbour's midpoint, write the order only on drop.
  const reorder = useStripReorder({
    keys: tabs.map((t) => t.id),
    swapOn: "centre",
    gap: SEPARATOR_W,
    onDrop: ({ key, target }) => useTabStore.getState().moveTab(key, target),
  });
  const drag = reorder.drag;

  // The tab region (right of the history arrows) is what the tabs divide up.
  useEffect(() => {
    const el = stripRef.current;
    if (!el) return;
    setStripW(el.clientWidth);
    const ro = new ResizeObserver(([entry]) => setStripW(entry.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Before the first measurement, full width, to avoid a flash of narrow tabs.
  const tabW =
    stripW === 0 || tabs.length === 0
      ? TAB_W
      : Math.max(
          TAB_MIN_W,
          Math.min(
            TAB_W,
            Math.floor(
              (stripW - TRAILING_W - (tabs.length - 1) * SEPARATOR_W) /
                tabs.length,
            ),
          ),
        );

  // History arrows act on the focused pane: a browser page's own history
  // (Rust reports `can_back`/`can_forward` in its snapshot), else the pane's
  // memory-router index.
  const activeTab = tabs.find((t) => t.id === activeId);
  const activePane = activeTab && focusedPane(activeTab);
  const activeBrowse = browseId(activePane?.path);
  const activeBrowseTab =
    activeBrowse != null
      ? browserTabs.find((t) => t.id === activeBrowse)
      : undefined;
  const canGoBack =
    activeBrowse != null ? !!activeBrowseTab?.can_back : !!activePane?.canBack;
  const canGoForward =
    activeBrowse != null
      ? !!activeBrowseTab?.can_forward
      : !!activePane?.canForward;
  const go = (delta: 1 | -1) => {
    if (activeBrowse != null)
      browser.history(activeBrowse, delta).catch(() => {});
    else goInActiveTab(delta);
  };

  // Every tab's pane stays mounted, so switching is just bringing it forward.
  const switchTo = (id: number) => {
    if (id === activeId) return;
    setActive(id);
  };

  // A browser tab closes through Rust, which owns its page; the strip hears
  // back through `browser-state` and drops the tab then.
  const shut = (id: number) => {
    const bid = browseId(tabs.find((t) => t.id === id)?.path);
    if (bid != null) {
      // Remember the URL now: by the time the snapshot closes the pane, Rust
      // has already dropped the tab and its URL with it.
      const url = browserTabs.find((t) => t.id === bid)?.url;
      if (url)
        useTabStore.getState().remember({
          index: tabs.findIndex((t) => t.id === id),
          path: null,
          url,
          side: null,
        });
      browser.close(bid).catch(() => {});
      return;
    }
    closeTab(id);
  };

  // Closing a tab whose page or side panel owns lecture playback asks first.
  const close = (id: number) => {
    const tab = tabs.find((t) => t.id === id);
    if (tab && panesOf(tab).some((p) => ownsPlayback(p.id)))
      confirmLeavingLecture(() => shut(id));
    else shut(id);
  };

  const newTab = () => addTab(NEW_TAB_PATH);
  const side = () => toggleSide(activeId);
  // ⌘1…⌘8 past the end of the strip do nothing (not the nearest tab).
  const selectTab = (index: number) => {
    const tab = tabs[index];
    if (tab) switchTo(tab.id);
  };
  // ⌘9 is the last tab, as in Chrome and Safari.
  const selectLastTab = () => {
    const tab = tabs[tabs.length - 1];
    if (tab) switchTo(tab.id);
  };
  const sideOpen = !!activeTab?.side;
  // ⌃Tab / ⌃⇧Tab walk the side panel's list, only while it has focus: the
  // strip's own keys are ⌘-keys.
  const stepSide = (delta: 1 | -1) => {
    if (!activeTab?.side || activeTab.focus !== "side") return;
    frontSide(activeTab.id, stepItem(activeTab.side, delta));
  };

  useTabMenuEvents({
    newTab,
    side,
    go,
    reopenTab,
    selectTab,
    selectLastTab,
    stepSide,
    closeActive: () => {
      const tab = tabs.find((t) => t.id === activeId);
      // Same rule as the ×: a sole app tab can't be closed.
      if (!tab || (tabs.length === 1 && browseId(tab.path) == null)) return;
      close(tab.id);
    },
  });

  const barButton =
    "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

  return (
    /* "deep": any empty spot in the bar drags the window — buttons block it
       on their own, tabs opt out since they reorder on pointerdown. */
    <div
      data-tauri-drag-region="deep"
      className="h-11 shrink-0 flex items-center gap-1 pr-2"
      /* The native traffic lights sit at a fixed device-pixel position, so
         their gap divides out page zoom (none in fullscreen). Their vertical
         spot is `trafficLightPosition.y` in `app/src-tauri/tauri.conf.json`,
         tuned to this bar's h-11 x DEFAULT_ZOOM — retune it if either changes. */
      style={{
        paddingLeft: fullscreen ? "0.5rem" : "calc(84px / var(--app-zoom, 1))",
      }}
    >
      {/* Sidebar toggle — before the arrows, like Notion. */}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={onToggleSidebar}
            aria-label={sidebarCollapsed ? "Open sidebar" : "Close sidebar"}
            className={barButton}
          >
            <Sidebar size={18} />
          </button>
        </TooltipTrigger>
        <TooltipContent
          side="bottom"
          className="flex flex-col items-start gap-0.5"
        >
          {sidebarCollapsed ? "Open sidebar" : "Close sidebar"}
          <span className="text-[11px] text-background/60">⌘B</span>
        </TooltipContent>
      </Tooltip>

      {/* History */}
      <button
        onClick={() => go(-1)}
        disabled={!canGoBack}
        aria-label="Go back"
        className={barButton}
      >
        <CaretLeft size={16} />
      </button>
      <button
        onClick={() => go(1)}
        disabled={!canGoForward}
        aria-label="Go forward"
        className={cn(barButton, "mr-1")}
      >
        <CaretRight size={16} />
      </button>

      {/* The wrapper claims the rest of the bar for the tabs to divide; the
          inner strip hugs its tabs so + sits beside the last one. */}
      <div ref={stripRef} className="flex flex-1 min-w-0 items-center gap-1">
        <div className="flex min-w-0 select-none items-center overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
          {tabs.map((tab, i) => {
            const { title, icon } = tabInfo(tab.path);
            const prev = tabs[i - 1];
            return (
              <StripTab
                key={tab.id}
                title={title}
                icon={icon}
                index={i}
                active={tab.id === activeId}
                hovered={tab.id === hoveredId}
                prevActiveOrHovered={!!prev && (prev.id === activeId || prev.id === hoveredId)}
                dragging={!!drag}
                grabbed={drag?.key === tab.id}
                width={tabW}
                closable={tabs.length > 1 || browseId(tab.path) != null}
                itemRef={reorder.itemRef(tab.id)}
                dragStyle={reorder.styleFor(tab.id, i)}
                onPress={(e) => {
                  switchTo(tab.id);
                  reorder.onPointerDown(e, tab.id);
                }}
                onClose={() => close(tab.id)}
                onHover={(on) => setHoveredId((h) => (on ? tab.id : h === tab.id ? null : h))}
              />
            );
          })}
        </div>

        <button onClick={newTab} aria-label="New tab" className={barButton}>
          <Plus size={15} />
        </button>
      </div>

      {/* Side panel toggle: at the far end, outside the measured strip — it
          acts on the tab in front, not the strip. */}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={side}
            aria-label={sideOpen ? "Close side panel" : "Open side panel"}
            aria-pressed={sideOpen}
            className={cn(
              barButton,
              sideOpen && "bg-sidebar-item-active text-foreground",
            )}
          >
            {/* Mirrored sidebar glyph; a two-column icon reads as "pause". */}
            <SidebarSimple size={17} className="scale-x-[-1]" />
          </button>
        </TooltipTrigger>
        <TooltipContent
          side="bottom"
          className="flex flex-col items-start gap-0.5"
        >
          {sideOpen ? "Close side panel" : "Open side panel"}
          <span className="text-[11px] text-background/60">⌥⌘T</span>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}
