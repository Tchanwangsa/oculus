import { useCallback, useEffect, useRef, useState } from "react";
import {
  Chat,
  CalendarBlank,
  ArrowsClockwise,
  CircleNotch,
  Kanban,
  MagnifyingGlass,
  SidebarSimple,
  GearSix,
  House,
} from "@phosphor-icons/react";
import { anyRunning, useHarnessStore } from "@/stores/harnessStore";
import { usePaletteStore } from "@/stores/paletteStore";
import { cn } from "@/lib/utils";
import NavItem from "./NavItem";
import SubjectsNavGroup from "./SubjectsNavGroup";
import RecentNavGroup from "./RecentNavGroup";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

const WIDTH = 212;

/**
 * Collapsed means gone: the sidebar animates to zero width (no icon rail), and
 * the way back in is the sidebar button in the title bar — Notion's pattern.
 *
 * It carries no fill or divider of its own — it sits directly on the window's
 * warm ground, and the content card's border is what separates the two.
 *
 * Only the subject list scrolls. The logo, the top-level nav and the footer are
 * pinned, and inset hairlines mark where the scrolling middle begins and ends.
 */
export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const width = collapsed ? 0 : WIDTH;
  const scrollRef = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState({ top: false, bottom: false });
  // An agent at work is the one background job that shows here, bb's way:
  // a spinner on the row, nothing else.
  const agentBusy = useHarnessStore((s) => anyRunning(s.live));

  const measure = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const top = el.scrollTop > 1;
    const bottom = el.scrollTop + el.clientHeight < el.scrollHeight - 1;
    setEdges((prev) =>
      prev.top === top && prev.bottom === bottom ? prev : { top, bottom },
    );
  }, []);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    measure();
    el.addEventListener("scroll", measure, { passive: true });
    // The list grows and shrinks as past subjects unfold, so watch the content
    // box too — a scroll event alone would miss it.
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    if (el.firstElementChild) ro.observe(el.firstElementChild);
    return () => {
      el.removeEventListener("scroll", measure);
      ro.disconnect();
    };
  }, [measure]);

  return (
    <aside
      /* width/min/max are pinned together so flexbox can never clamp the box to
         its min-content size mid-transition. */
      style={{ width, minWidth: width, maxWidth: width }}
      className={cn(
        "group/sidebar flex flex-col h-full shrink-0 grow-0 overflow-hidden",
        "transition-[width,min-width,max-width] duration-200 ease-out",
      )}
    >
      {/* Inner keeps its full width during the slide so content doesn't reflow,
          it just gets clipped. */}
      <div className="flex flex-col h-full" style={{ width: WIDTH, minWidth: WIDTH }}>
        {/* Header: logo + collapse toggle */}
        <div className="relative flex items-center h-10 pl-3 pr-2 shrink-0">
          <img src="/oculus-mark.svg" alt="" className="w-[18px] h-[18px] shrink-0" />
          <span className="ml-2 flex-1 min-w-0 overflow-hidden whitespace-nowrap font-display font-semibold text-foreground tracking-tight text-[13px]">
            Oculus
          </span>

          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={onToggle}
                aria-label="Close sidebar"
                className={cn(
                  "flex h-6 w-6 shrink-0 items-center justify-center rounded-md",
                  "text-muted-foreground hover:text-foreground hover:bg-sidebar-item-hover active:bg-sidebar-item-active",
                  "opacity-0 focus-visible:opacity-100 group-hover/sidebar:opacity-100 transition-[opacity,color,background-color] duration-150",
                )}
              >
                <SidebarSimple size={15} />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom" align="end" className="flex flex-col items-start gap-0.5">
              Close sidebar
              <span className="text-[11px] text-background/60">⌘B</span>
            </TooltipContent>
          </Tooltip>
        </div>

        {/* Pinned top-level nav — outside the scroller, so it never slides away. */}
        <div className="px-2 pb-1.5 shrink-0">
          <SearchItem />
          <NavItem to="/" icon={House} label="Home" />
          <NavItem
            to="/chat"
            icon={Chat}
            label="Chat"
            badge={agentBusy ? <CircleNotch size={12} className="shrink-0 animate-spin text-muted-foreground" /> : null}
          />
          <NavItem to="/calendar" icon={CalendarBlank} label="Calendar" />
          <NavItem to="/projects" icon={Kanban} label="Projects" />
        </div>

        <Rule />

        {/* The one scrolling region. Its scrollbar is hidden: a bar that appears
            on overflow takes its gutter out of the width and jogs every row
            sideways, so the fades below carry the affordance instead. */}
        <div className="relative flex-1 min-h-0">
          <div
            ref={scrollRef}
            className="h-full overflow-y-auto overflow-x-hidden px-2 py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
          >
            {/* One wrapper, not two children: the ResizeObserver below watches
                the scroller's first child to catch the list growing. */}
            <div className="space-y-3">
              <RecentNavGroup />
              <SubjectsNavGroup />
            </div>
          </div>
          <ScrollFade side="top" show={edges.top} />
          <ScrollFade side="bottom" show={edges.bottom} />
        </div>

        <Rule />

        {/* Bottom nav */}
        <div className="pt-1.5 pb-2 px-2 shrink-0">
          <NavItem to="/settings" icon={GearSix} label="Settings" />
          <NavItem to="/sync" icon={ArrowsClockwise} label="Sync" />
        </div>
      </div>
    </aside>
  );
}

/**
 * The ⌘K palette's visible handle, shaped like a nav row but a button: search
 * is the one thing at the top of this list that is not a place. The shortcut
 * rides on the row rather than in a tooltip — it is what the row is teaching.
 */
function SearchItem() {
  const setOpen = usePaletteStore((s) => s.setOpen);
  return (
    <button
      type="button"
      onClick={() => setOpen(true)}
      className={cn(
        "flex w-full items-center gap-2.5 rounded-md px-2 py-1.5 text-[12.5px]",
        "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground transition-colors",
      )}
    >
      <MagnifyingGlass size={16} className="shrink-0" />
      <span className="flex-1 min-w-0 overflow-hidden whitespace-nowrap text-clip text-left">
        Search
      </span>
      <span className="shrink-0 text-[11px] tabular-nums text-muted-foreground/60">
        ⌘K
      </span>
    </button>
  );
}

/** Inset hairline: it stops short of both edges so it reads as a seam in the
 *  list rather than a panel border. */
function Rule() {
  return <div aria-hidden className="mx-3 h-px shrink-0 bg-sidebar-border/70" />;
}

/**
 * Fade over a scroll edge, shown only while there is more content that way.
 *
 * Deliberately a plain gradient and not `backdrop-filter`: a backdrop layer
 * inside this aside — which clips its overflow and transitions its width —
 * makes WebKit drop the whole compositing layer when the layer is torn down,
 * blanking the entire sidebar until it remounts. Over a flat ground the
 * gradient is indistinguishable from a blur anyway.
 */
function ScrollFade({ side, show }: { side: "top" | "bottom"; show: boolean }) {
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-x-0 h-8 transition-opacity duration-150",
        side === "top"
          ? "top-0 bg-gradient-to-b from-sidebar via-sidebar/85 to-transparent"
          : "bottom-0 bg-gradient-to-t from-sidebar via-sidebar/85 to-transparent",
        show ? "opacity-100" : "opacity-0",
      )}
    />
  );
}
