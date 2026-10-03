import { Fragment, useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { listen } from "@tauri-apps/api/event";
import {
  CaretLeft,
  CaretRight,
  Plus,
  Sidebar,
  SidebarSimple,
  X,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useStripReorder } from "@/hooks/usePointerDrag";
import { focusedPane, useTabStore } from "@/stores/tabStore";
import { useBrowserStore } from "@/stores/browserStore";
import { browser, browseId } from "@/lib/browser";
import { useSubjects } from "@/hooks/useSubjects";
import { tabInfo } from "@/components/tabs/tabInfo";
import { goInActiveTab } from "@/lib/tabRouters";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useWindowFullscreen } from "@/hooks/useWindowFullscreen";
import { ownsPlayback } from "@/lib/lecturePlayback";
import { confirmLeavingLecture } from "@/stores/leaveLectureStore";

/** Where + and ⌘T open (`app/src/pages/NewTabPage.tsx`). */
const NEW_TAB_PATH = "/new";

/** Column between two tabs; also part of the distance a swapped tab travels. */
const SEPARATOR_W = 6;

/** Uniform Chrome-style widths: TAB_W each until the strip is full, then an
 *  equal share down to TAB_MIN_W, then the strip scrolls. Computed rather than
 *  left to `flex-shrink`: a scrolling flex container in WebKit sizes itself to
 *  its content, so the tabs would hug their titles. */
const TAB_W = 200;
const TAB_MIN_W = 76;
/** What the new-tab button and its two gaps take out of the tab region. */
const TRAILING_W = 36;

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
  const { tabs, activeId, addTab, setActive, closeTab, toggleSplit, reopenTab } =
    useTabStore(useShallow((s) => ({
      tabs: s.tabs,
      activeId: s.activeId,
      addTab: s.addTab,
      setActive: s.setActive,
      closeTab: s.closeTab,
      toggleSplit: s.toggleSplit,
      reopenTab: s.reopenTab,
    })));
  const { subjects } = useSubjects();
  const browserTabs = useBrowserStore((s) => s.tabs);
  const favicons = useBrowserStore((s) => s.favicons);
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

  // History arrows act on the focused half: a browser page's own history
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
  const close = (id: number) => {
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
          split: null,
        });
      browser.close(bid).catch(() => {});
      return;
    }
    // Closing the tab that owns lecture playback asks first.
    if (ownsPlayback(id)) confirmLeavingLecture(() => closeTab(id));
    else closeTab(id);
  };

  const newTab = () => addTab(NEW_TAB_PATH);
  const split = () => toggleSplit(activeId);
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
  const splitOpen = !!activeTab?.split;

  // Tab shortcuts arrive as menu events (`app/src-tauri/src/menu.rs`): macOS
  // gives the menu bar every ⌘-key first, which also makes them work while a
  // browser page's native WebView holds focus.
  const menuActions = useRef({
    newTab,
    split,
    reopenTab,
    closeActive: () => {},
    selectTab: (_: number) => {},
    selectLastTab: () => {},
    go: (_: 1 | -1) => {},
  });
  menuActions.current = {
    newTab,
    split,
    go,
    reopenTab,
    selectTab,
    selectLastTab,
    closeActive: () => {
      const tab = tabs.find((t) => t.id === activeId);
      // Same rule as the ×: a sole app tab can't be closed.
      if (!tab || (tabs.length === 1 && browseId(tab.path) == null)) return;
      close(tab.id);
    },
  };

  useEffect(() => {
    const pending = [
      listen("menu-new-tab", () => menuActions.current.newTab()),
      listen("menu-close-tab", () => menuActions.current.closeActive()),
      listen("menu-reopen-tab", () => menuActions.current.reopenTab()),
      listen<number>("menu-select-tab", (e) =>
        menuActions.current.selectTab(e.payload),
      ),
      listen("menu-last-tab", () => menuActions.current.selectLastTab()),
      listen("menu-split", () => menuActions.current.split()),
      listen("menu-back", () => menuActions.current.go(-1)),
      listen("menu-forward", () => menuActions.current.go(1)),
    ];
    return () => {
      for (const p of pending) p.then((un) => un()).catch(() => {});
    };
  }, []);

  const barButton =
    "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

  return (
    <div
      data-tauri-drag-region
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
          const active = tab.id === activeId;
          const hovered = tab.id === hoveredId;
          const { title, icon } = tabInfo(
            tab.path,
            subjects,
            browserTabs,
            13,
            favicons,
          );
          // Separator only between two inactive, unhovered neighbours.
          const prev = tabs[i - 1];
          const showSeparator =
            i > 0 &&
            !active &&
            prev.id !== activeId &&
            !hovered &&
            prev.id !== hoveredId &&
            !drag;
          const grabbed = drag?.key === tab.id;
          return (
            <Fragment key={tab.id}>
              {/* Always SEPARATOR_W wide: the drag maths counts on it. */}
              <span
                className={cn(
                  "flex shrink-0 items-center justify-center",
                  i === 0 && "hidden",
                )}
                style={{ width: SEPARATOR_W }}
              >
                <span
                  className={cn(
                    "h-3.5 w-px rounded-full transition-colors",
                    showSeparator ? "bg-border" : "bg-transparent",
                  )}
                />
              </span>
              <Tooltip>
                <TooltipTrigger asChild>
                  <div
                    ref={reorder.itemRef(tab.id)}
                    style={{ flex: "none", width: tabW, ...reorder.styleFor(tab.id, i) }}
                    className={cn(
                      "group relative flex h-7 items-center rounded-lg px-3 overflow-hidden cursor-pointer",
                      active
                        ? "bg-card text-foreground border border-border shadow-xs"
                        : "text-muted-foreground hover:text-foreground",
                      grabbed
                        ? "z-10 shadow-md"
                        : drag
                          ? "transition-transform duration-200 ease-out"
                          : "transition-colors",
                    )}
                    onPointerDown={(e) => {
                      if (e.button !== 0) return;
                      switchTo(tab.id);
                      reorder.onPointerDown(e, tab.id);
                    }}
                    onAuxClick={(e) => {
                      if (e.button === 1) close(tab.id);
                    }}
                    onMouseEnter={() => setHoveredId(tab.id)}
                    onMouseLeave={() =>
                      setHoveredId((h) => (h === tab.id ? null : h))
                    }
                  >
                    {!active && (
                      <span
                        aria-hidden
                        className={cn(
                          "absolute inset-0 rounded-lg transition-colors",
                          hovered && "bg-sidebar-item-hover",
                        )}
                      />
                    )}
                    {icon && (
                      <span className="relative mr-1.5 flex shrink-0 items-center">
                        {icon}
                      </span>
                    )}
                    {/* Long titles fade out at the edge instead of an ellipsis. */}
                    <span
                      style={{
                        maskImage:
                          "linear-gradient(to right, #000 calc(100% - 22px), transparent)",
                      }}
                      className="relative min-w-0 flex-1 text-[12px] whitespace-nowrap overflow-hidden [text-overflow:clip] py-1"
                    >
                      {title}
                    </span>
                    {(tabs.length > 1 || browseId(tab.path) != null) && (
                      /* The × overlays the right edge on hover; its gradient
                         stays soft since the title mask already fades the text. */
                      <span
                        className={cn(
                          "absolute flex items-center pl-6 opacity-0 group-hover:opacity-100 transition-opacity",
                          active
                            ? "inset-y-px right-px pr-1.5 rounded-r-lg bg-gradient-to-l from-card from-40% via-card/70 via-75% to-transparent"
                            : "inset-y-0 right-0 pr-1.5 rounded-r-lg bg-gradient-to-l from-sidebar-item-hover from-40% via-sidebar-item-hover/70 via-75% to-transparent",
                        )}
                      >
                        <button
                          onPointerDown={(e) => e.stopPropagation()}
                          onClick={(e) => {
                            e.stopPropagation();
                            close(tab.id);
                          }}
                          aria-label="Close tab"
                          className="rounded-md p-0.5 text-muted-foreground hover:text-foreground hover:bg-sidebar-item-active transition-colors"
                        >
                          <X size={11} />
                        </button>
                      </span>
                    )}
                  </div>
                </TooltipTrigger>
                <TooltipContent side="bottom">{title}</TooltipContent>
              </Tooltip>
            </Fragment>
          );
        })}
        </div>

        <button onClick={newTab} aria-label="New tab" className={barButton}>
          <Plus size={15} />
        </button>

        {/* Remaining space stays draggable. */}
        <div data-tauri-drag-region className="flex-1 h-full" />
      </div>

      {/* Split toggle: at the far end, outside the measured strip — it acts on
          the tab in front, not the strip. */}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={split}
            aria-label={splitOpen ? "Close sidepanel" : "Open sidepanel"}
            aria-pressed={splitOpen}
            className={cn(
              barButton,
              splitOpen && "bg-sidebar-item-active text-foreground",
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
          {splitOpen ? "Close sidepanel" : "Open sidepanel"}
          <span className="text-[11px] text-background/60">⌥⌘T</span>
        </TooltipContent>
      </Tooltip>
    </div>
  );
}
