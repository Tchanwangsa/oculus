import { Fragment, useEffect, useRef, useState } from "react";
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

/** Where a tab opened from the + button or ⌘T starts:
 *  `app/src/pages/NewTabPage.tsx`, which asks where you are going. */
const NEW_TAB_PATH = "/new";

/** Width of the column between two tabs: their visual gap, and the extra
 *  distance a tab travels when it swaps places with a neighbour. */
const SEPARATOR_W = 6;

/** Tabs are uniform and fixed-width, Chrome-style: every tab is TAB_W however
 *  long its title, until the strip is full — only then do they all shrink
 *  together to share the space, down to TAB_MIN_W, past which the strip
 *  scrolls rather than shrinking further.
 *
 *  The width is computed here rather than left to `flex-shrink` because a
 *  flex container that scrolls reports its *content* width as its intrinsic
 *  width in WebKit: the strip sized itself to the titles and the tabs then
 *  shrank to fit that, which is exactly the content-hugging this replaces.
 *  An explicit width also keeps the drag maths honest — every displaced tab
 *  travels the same distance as the grabbed one. */
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
    useTabStore();
  const { subjects } = useSubjects();
  const browserTabs = useBrowserStore((s) => s.tabs);
  const favicons = useBrowserStore((s) => s.favicons);
  const fullscreen = useWindowFullscreen();
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const tabRefs = useRef(new Map<number, HTMLDivElement>());
  const stripRef = useRef<HTMLDivElement>(null);
  const [stripW, setStripW] = useState(0);
  /** Live drag: the grabbed tab follows the pointer (`dx`), the others animate
      towards where they'd land if it were dropped at `target`. */
  const [drag, setDrag] = useState<{
    id: number;
    dx: number;
    from: number;
    target: number;
    width: number;
  } | null>(null);

  // The tab region is everything right of the history arrows. Its width is
  // what the tabs divide up, so it is measured rather than assumed.
  useEffect(() => {
    const el = stripRef.current;
    if (!el) return;
    setStripW(el.clientWidth);
    const ro = new ResizeObserver(([entry]) => setStripW(entry.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Full width each until they no longer fit, then an equal share of what
  // there is. Before the first measurement, full width — a tab that starts
  // at the floor and jumps out to TAB_W reads as a flash of the wrong layout.
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

  // While a browser page is in front, the arrows are the page's history, not
  // the router's — as they would be in a browser. Whether that page has
  // anywhere to go is a question only the page can answer, so Rust reads its
  // back/forward list after every load and the answer rides in the snapshot
  // (`can_back`/`can_forward`); before that landed the arrows simply stayed
  // lit, which meant the back arrow on a page you had just opened was an
  // offer the page could not keep.
  // For an app page the answer comes from the pane itself: each keeps its own
  // history index, since a memory router has no `window.history` to read.
  //
  // Asked of the *focused* half, which is the same half the sidebar highlights
  // and ⌘K navigates — one answer to "where am I", however the tab is split.
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

  // Switching tabs is no longer a navigation: the destination pane is already
  // mounted at its own path, and bringing it forward is all there is to do.
  const switchTo = (id: number) => {
    if (id === activeId) return;
    setActive(id);
  };

  // A browser tab closes through Rust, which owns its page; the strip hears
  // back through `browser-state` and drops the tab then.
  const close = (id: number) => {
    const bid = browseId(tabs.find((t) => t.id === id)?.path);
    if (bid != null) {
      // Remember the page here rather than in `closeTab`: the route is about
      // to name a WebView that no longer exists, and by the time the snapshot
      // comes back and closes the pane, Rust has already dropped the tab whose
      // URL is the only thing worth keeping.
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
    // Closing the tab a lecture is playing in is the one close that loses
    // something; it asks first, and goes ahead unprompted for every other tab.
    if (ownsPlayback(id)) confirmLeavingLecture(() => closeTab(id));
    else closeTab(id);
  };

  const newTab = () => addTab(NEW_TAB_PATH);
  const split = () => toggleSplit(activeId);
  // ⌘1…⌘8 are positions in the strip, and a position the strip does not have
  // is nothing — not the nearest tab, which would land you somewhere you did
  // not ask for and have to look at to find out where.
  const selectTab = (index: number) => {
    const tab = tabs[index];
    if (tab) switchTo(tab.id);
  };
  // ⌘9 is the *last* tab rather than the ninth, which is what Chrome and
  // Safari do with it: it is the one number that keeps its meaning once the
  // strip is longer than the keyboard counts.
  const selectLastTab = () => {
    const tab = tabs[tabs.length - 1];
    if (tab) switchTo(tab.id);
  };
  const splitOpen = !!activeTab?.split;

  // ⌘T, ⌘W and ⌥⌘T arrive as menu events rather than key presses: macOS hands
  // the menu bar every ⌘-key before a webview sees it, so they can only be
  // menu items (`app/src-tauri/src/menu.rs`) — which is also what makes them
  // work while a browser page's native WebView holds focus and the app's own
  // webview is getting no keys at all. That matters most for the split: a page
  // in one half is exactly when you reach for the other. The strip does the
  // work either way.
  // ⌘[ and ⌘] are here for the same reason and route through the same `go`
  // the arrows do, so "back" means one thing however it was asked for. So are
  // ⌘1…⌘9 and ⇧⌘T, and for them the browser case is the point twice over: a
  // page you are reading is exactly when you want the tab you came from back,
  // and a page you just closed is exactly what ⇧⌘T is for.
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
      // The same rule the × follows: a sole app tab doesn't offer one,
      // because there is nothing left to close back to.
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

  // Chrome-style drag reorder: the tab activates on pointer-down, then once
  // the pointer moves past a small threshold it lifts and follows the pointer.
  // The other tabs slide out of / into its way as its centre crosses their
  // midpoints; the store order only changes on drop. Nothing reflows during
  // the drag, so all positions come from rects captured when the lift starts.
  const onTabPointerDown = (
    e: React.PointerEvent<HTMLDivElement>,
    tab: { id: number; path: string },
  ) => {
    if (e.button !== 0) return;
    switchTo(tab.id);
    const el = e.currentTarget;
    const pointerId = e.pointerId;
    const startX = e.clientX;
    let rects: { left: number; mid: number; width: number }[] = [];
    let from = -1;
    let latest: { dx: number; target: number } | null = null;

    const onMove = (ev: PointerEvent) => {
      if (from === -1) {
        if (Math.abs(ev.clientX - startX) < 4) return;
        const { tabs: current } = useTabStore.getState();
        rects = current.map((t) => {
          const r = tabRefs.current.get(t.id)!.getBoundingClientRect();
          return { left: r.left, mid: r.left + r.width / 2, width: r.width };
        });
        from = current.findIndex((t) => t.id === tab.id);
      }
      const me = rects[from];
      const last = rects[rects.length - 1];
      // Keep the lifted tab inside the strip.
      const dx = Math.min(
        Math.max(ev.clientX - startX, rects[0].left - me.left),
        last.left + last.width - (me.left + me.width),
      );
      const center = me.mid + dx;
      let target = from;
      for (let i = from - 1; i >= 0; i--) if (center < rects[i].mid) target = i;
      for (let i = from + 1; i < rects.length; i++)
        if (center > rects[i].mid) target = i;
      latest = { dx, target };
      setDrag({ id: tab.id, dx, from, target, width: me.width });
    };
    const end = () => {
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
      if (el.hasPointerCapture(pointerId)) el.releasePointerCapture(pointerId);
      if (latest) useTabStore.getState().moveTab(tab.id, latest.target);
      setDrag(null);
    };
    el.setPointerCapture(pointerId);
    el.addEventListener("pointermove", onMove);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
  };

  const barButton =
    "flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

  return (
    <div
      data-tauri-drag-region
      /* No rule under the strip: the content is a card floating below it, so
         there is nothing for a tab to merge into. Tabs are pills on the same
         ground as the sidebar, and the active one is a scrap of the card
         lifted up here. */
      className="h-11 shrink-0 flex items-center gap-1 pr-2"
      /* Native traffic lights overlay this strip on macOS. They sit at a
         fixed device-pixel position, so the gap they need is measured in
         device pixels too — divide out the window's page zoom. Fullscreen
         hides them, and the gap with them: the bar then starts at the same
         inset as everything else.
         Their vertical placement is the other half of the same sum, and it
         lives in `trafficLightPosition.y` in `app/src-tauri/tauri.conf.json`
         because AppKit owns those buttons. tao insets the button group from
         the window top, which puts the circles' centre 2pt below the value;
         this bar's centre is `h-11` (44px) x DEFAULT_ZOOM / 2, so the config
         holds that centre minus 2. Change the bar height or the default
         zoom and the lights need retuning — nothing here can do it. */
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

      {/* Tabs — full bar height, browser-style. The wrapper claims the rest
          of the bar, giving the tabs a definite width to divide up; the strip
          inside it is sized by its (now explicitly sized) tabs, which is what
          keeps the new-tab button beside the last tab instead of out at the
          far right. */}
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
          // Chrome-style: a small vertical separator between two inactive
          // neighbours, hidden next to the active or hovered tab.
          const prev = tabs[i - 1];
          const showSeparator =
            i > 0 &&
            !active &&
            prev.id !== activeId &&
            !hovered &&
            prev.id !== hoveredId &&
            !drag;
          // During a drag the grabbed tab rides the pointer; every tab between
          // its old and prospective slot slides one tab-width the other way.
          const grabbed = drag?.id === tab.id;
          let dragStyle: React.CSSProperties | undefined;
          if (drag) {
            if (grabbed) {
              dragStyle = { transform: `translateX(${drag.dx}px)` };
            } else {
              const shift = drag.width + SEPARATOR_W;
              if (drag.from < i && i <= drag.target)
                dragStyle = { transform: `translateX(-${shift}px)` };
              else if (drag.target <= i && i < drag.from)
                dragStyle = { transform: `translateX(${shift}px)` };
            }
          }
          return (
            <Fragment key={tab.id}>
              {/* Always occupies SEPARATOR_W, coloured or not — the drag
                  maths measures neighbours in tab-width + this column, so it
                  cannot be allowed to collapse. */}
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
                    ref={(node) => {
                      if (node) tabRefs.current.set(tab.id, node);
                      else tabRefs.current.delete(tab.id);
                    }}
                    style={{ flex: "none", width: tabW, ...dragStyle }}
                    className={cn(
                      "group relative flex h-7 items-center rounded-lg px-3 overflow-hidden cursor-pointer",
                      active
                        ? "bg-card text-foreground border border-border shadow-xs"
                        : "text-muted-foreground hover:text-foreground",
                      grabbed
                        ? // Lifted: a floating card above its neighbours,
                          // tracking the pointer with no easing lag.
                          "z-10 shadow-md"
                        : drag
                          ? "transition-transform duration-200 ease-out"
                          : "transition-colors",
                    )}
                    onPointerDown={(e) => onTabPointerDown(e, tab)}
                    onAuxClick={(e) => {
                      // Middle-click closes, like a browser.
                      if (e.button === 1) close(tab.id);
                    }}
                    onMouseEnter={() => setHoveredId(tab.id)}
                    onMouseLeave={() =>
                      setHoveredId((h) => (h === tab.id ? null : h))
                    }
                  >
                    {/* Chrome's hover: a rounded pill hugging the label, not a
                        full-height fill. */}
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
                    {/* No ellipsis: a title too long for the tab fades out
                        at the edge instead of being chopped mid-glyph, and
                        runs under the × overlay's own fade on hover. The span
                        is flex-1, so for a title that fits, the masked strip
                        is empty and nothing fades. */}
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
                      /* The × doesn't take layout space — it fades in over
                         the right edge on hover. Its gradient only has to
                         clear ground for the button itself: the title's own
                         edge mask has already faded the text out, so a hard
                         or wide gradient here just doubles up and reads as a
                         chunk bitten out of the tab. */
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

      {/* Split toggle, at the far end of the bar rather than beside the +:
          it acts on the tab in front, not on the strip, and sitting among the
          tabs would read as making another one. Outside the measured strip,
          so the tabs divide up what is left of the bar without it. */}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            onClick={split}
            aria-label={splitOpen ? "Close sidepanel" : "Open sidepanel"}
            aria-pressed={splitOpen}
            className={cn(
              barButton,
              // Lit the way the sidebar lights an active row, not filled: a
              // solid glyph at this size reads as a pause button.
              splitOpen && "bg-sidebar-item-active text-foreground",
            )}
          >
            {/* The sidebar toggle's glyph, mirrored: the same idea on the
                other edge, and it reads as "a panel over there" where a
                two-column icon at 16px reads as a pause button. */}
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
