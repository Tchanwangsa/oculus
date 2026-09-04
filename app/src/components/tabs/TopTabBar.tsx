import { Fragment, useEffect, useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import {
  ArrowsClockwise,
  BookOpen,
  CaretLeft,
  CaretRight,
  Chat,
  GearSix,
  Plus,
  Sidebar,
  X,
} from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useTabStore } from "@/stores/tabStore";
import { useSubjects } from "@/hooks/useSubjects";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, humanizeSlug } from "@/lib/format";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { Subject } from "@/lib/db";

const SECTION_LABELS: Record<string, string> = {
  modules: "Modules",
  downloads: "Downloads",
  lectures: "Lectures",
  announcements: "Announcements",
  assignments: "Assignments",
  discussion: "Discussion",
};

interface TabInfo {
  title: string;
  icon: React.ReactNode;
}

function tabInfo(path: string, subjects: Subject[]): TabInfo {
  const [pathname, search = ""] = path.split("?");
  if (pathname.startsWith("/chat")) return { title: "Chat", icon: <Chat size={13} /> };
  if (pathname.startsWith("/sync"))
    return { title: "Sync", icon: <ArrowsClockwise size={13} /> };
  if (pathname.startsWith("/settings"))
    return { title: "Settings", icon: <GearSix size={13} /> };
  const m = /^\/subjects\/(\d+)(?:\/([\w-]+))?/.exec(pathname);
  if (m) {
    const subject = subjects.find((s) => String(s.id) === m[1]);
    // Anything inside a subject carries the subject's identity glyph.
    const icon = subject ? (
      <SubjectIcon code={subject.code} size={13} />
    ) : (
      <BookOpen size={13} />
    );
    // Full-page documents are titled by themselves, like Notion pages.
    if (m[2] === "file") {
      const rel = new URLSearchParams(search).get("path");
      const base = rel?.split("/").pop();
      if (base) return { title: humanizeSlug(base.replace(/\.pdf$/i, "")), icon };
    }
    if (m[2] === "lecture") {
      return { title: new URLSearchParams(search).get("t") ?? "Lecture", icon };
    }
    const code = subject ? displayCode(subject.code) : "Subject";
    const section = m[2] ? SECTION_LABELS[m[2]] : null;
    return { title: section ? `${code} · ${section}` : code, icon };
  }
  if (pathname.startsWith("/subjects"))
    return { title: "Subjects", icon: <BookOpen size={13} /> };
  return { title: "Oculus", icon: null };
}

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
  const location = useLocation();
  const navigate = useNavigate();
  const { tabs, activeId, trackNavigation, addTab, setActive, closeTab } =
    useTabStore();
  const { subjects } = useSubjects();
  const [historyIdx, setHistoryIdx] = useState(0);
  const [hoveredId, setHoveredId] = useState<number | null>(null);
  const tabRefs = useRef(new Map<number, HTMLDivElement>());
  /** Live drag: the grabbed tab follows the pointer (`dx`), the others animate
      towards where they'd land if it were dropped at `target`. */
  const [drag, setDrag] = useState<{
    id: number;
    dx: number;
    from: number;
    target: number;
    width: number;
  } | null>(null);

  // Keep the active tab pointed at wherever the router actually is.
  useEffect(() => {
    trackNavigation(location.pathname + location.search);
    // React Router stores its position on history.state — the only reliable
    // way to know whether back/forward have anywhere to go.
    setHistoryIdx((window.history.state?.idx as number) ?? 0);
  }, [location.pathname, location.search, trackNavigation]);

  const canGoBack = historyIdx > 0;
  const canGoForward = historyIdx < window.history.length - 1;

  const switchTo = (id: number, path: string) => {
    if (id === activeId) return;
    setActive(id);
    navigate(path);
  };

  const close = (id: number) => {
    const nextPath = closeTab(id);
    if (nextPath != null) navigate(nextPath);
  };

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
    switchTo(tab.id, tab.path);
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
    "flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent transition-colors";

  return (
    <div
      data-tauri-drag-region
      /* The bottom border is an inset shadow, not a real border, so the active
         tab (an opaque child) covers it and merges into the content below. */
      className="h-10 shrink-0 flex items-center gap-1 pr-2 bg-sidebar shadow-[inset_0_-1px_0_var(--color-sidebar-border)]"
      /* Native traffic lights overlay this strip on macOS. They sit at a
         fixed device-pixel position, so the gap they need is measured in
         device pixels too — divide out the window's page zoom. */
      style={{ paddingLeft: "calc(84px / var(--app-zoom, 1))" }}
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
        <TooltipContent side="bottom" className="flex flex-col items-start gap-0.5">
          {sidebarCollapsed ? "Open sidebar" : "Close sidebar"}
          <span className="text-[11px] text-background/60">⌘\</span>
        </TooltipContent>
      </Tooltip>

      {/* History */}
      <button
        onClick={() => navigate(-1)}
        disabled={!canGoBack}
        aria-label="Go back"
        className={barButton}
      >
        <CaretLeft size={16} />
      </button>
      <button
        onClick={() => navigate(1)}
        disabled={!canGoForward}
        aria-label="Go forward"
        className={cn(barButton, "mr-1")}
      >
        <CaretRight size={16} />
      </button>

      {/* Tabs — full bar height, browser-style. */}
      <div className="flex select-none items-stretch self-stretch overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {tabs.map((tab, i) => {
          const active = tab.id === activeId;
          const hovered = tab.id === hoveredId;
          const { title, icon } = tabInfo(tab.path, subjects);
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
              const shift = drag.width + 1; // +1 for the separator column
              if (drag.from < i && i <= drag.target)
                dragStyle = { transform: `translateX(-${shift}px)` };
              else if (drag.target <= i && i < drag.from)
                dragStyle = { transform: `translateX(${shift}px)` };
            }
          }
          return (
            <Fragment key={tab.id}>
              <span
                className={cn(
                  "self-center h-4 w-px shrink-0 rounded-full transition-colors",
                  i > 0 ? (showSeparator ? "bg-border" : "bg-transparent") : "hidden",
                )}
              />
              <Tooltip>
                <TooltipTrigger asChild>
                  <div
                    ref={(node) => {
                      if (node) tabRefs.current.set(tab.id, node);
                      else tabRefs.current.delete(tab.id);
                    }}
                    style={dragStyle}
                    className={cn(
                      "group relative flex items-center rounded-t-[6px] px-5 max-w-60 overflow-hidden cursor-pointer",
                      active
                        ? "bg-background text-foreground border-x border-t border-border"
                        : "text-muted-foreground hover:text-foreground",
                      grabbed
                        ? // Lifted: a floating card above its neighbours,
                          // tracking the pointer with no easing lag.
                          "z-10 rounded-b-[6px] border-b shadow-md"
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
                          "absolute inset-x-1 inset-y-[6px] rounded-lg transition-colors",
                          hovered && "bg-sidebar-item-hover",
                        )}
                      />
                    )}
                    {icon && (
                      <span className="relative mr-1.5 flex shrink-0 items-center">
                        {icon}
                      </span>
                    )}
                    {/* No ellipsis: long titles run under the × overlay's
                        fade, Notion-style. */}
                    <span className="relative text-[12px] whitespace-nowrap overflow-hidden [text-overflow:clip] py-1">
                      {title}
                    </span>
                    {tabs.length > 1 && (
                      /* The × doesn't take layout space — it fades in over the
                         right edge on hover, with a gradient masking the title
                         beneath it. */
                      <span
                        className={cn(
                          "absolute flex items-center pl-5 opacity-0 group-hover:opacity-100 transition-opacity",
                          active
                            ? "inset-y-0 right-0 pr-1 bg-gradient-to-l from-background via-background/90 to-transparent"
                            : "inset-y-[6px] right-1 pr-0.5 rounded-r-lg bg-gradient-to-l from-sidebar-item-hover via-sidebar-item-hover/90 to-transparent",
                        )}
                      >
                        <button
                          onPointerDown={(e) => e.stopPropagation()}
                          onClick={(e) => {
                            e.stopPropagation();
                            close(tab.id);
                          }}
                          aria-label="Close tab"
                          className="rounded p-0.5 text-muted-foreground hover:text-foreground hover:bg-sidebar-item-active transition-colors"
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

      <button
        onClick={() => {
          addTab("/subjects");
          navigate("/subjects");
        }}
        aria-label="New tab"
        className={barButton}
      >
        <Plus size={15} />
      </button>

      {/* Remaining space stays draggable. */}
      <div data-tauri-drag-region className="flex-1 h-full" />
    </div>
  );
}
