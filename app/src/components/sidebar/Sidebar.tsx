import { useRef } from "react";
import {
  Chat,
  CalendarBlank,
  ArrowsClockwise,
  CircleNotch,
  ListChecks,
  MagnifyingGlass,
  GearSix,
  House,
} from "@phosphor-icons/react";
import { anyRunning, useHarnessStore } from "@/stores/harnessStore";
import { useIndexStore } from "@/stores/indexStore";
import { usePaletteStore } from "@/stores/paletteStore";
import { cn } from "@/lib/utils";
import { useScrollFade } from "@/hooks/useScrollFade";
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
}

const WIDTH = 212;

/**
 * Collapsed animates to zero width (no icon rail); the title bar's button is
 * the only toggle. No fill or divider of its own — see docs/frontend.md (UI
 * system). Only the subject list scrolls; logo, nav and footer are pinned.
 */
export default function Sidebar({ collapsed }: SidebarProps) {
  const width = collapsed ? 0 : WIDTH;
  const scrollRef = useRef<HTMLDivElement>(null);
  // Background jobs surface only in the sidebar (see docs/frontend.md): a spinner.
  const agentBusy = useHarnessStore((s) => anyRunning(s.live));
  const indexing = useIndexStore((s) => s.running);

  // The list grows and shrinks as past subjects unfold; the hook watches the
  // content box (its one wrapper child) as well as the scroller.
  useScrollFade(scrollRef);

  return (
    <aside
      /* Pinned together so flexbox can't clamp to min-content mid-transition. */
      style={{ width, minWidth: width, maxWidth: width }}
      className={cn(
        "flex flex-col h-full shrink-0 grow-0 overflow-hidden",
        "transition-[width,min-width,max-width] duration-200 ease-out",
      )}
    >
      {/* Full width during the slide, so content clips rather than reflows. */}
      <div className="flex flex-col h-full" style={{ width: WIDTH, minWidth: WIDTH }}>
        <div className="relative flex items-center h-10 pl-3 pr-2 shrink-0">
          <img src="/oculus-mark.svg" alt="" className="w-[18px] h-[18px] shrink-0" />
          <span className="ml-2 flex-1 min-w-0 overflow-hidden whitespace-nowrap font-display font-semibold text-foreground tracking-tight text-[13px]">
            Oculus
          </span>
          <SearchButton />
        </div>

        <div className="px-2 pb-1.5 shrink-0">
          <NavItem to="/" icon={House} label="Home" />
          <NavItem
            to="/chat"
            icon={Chat}
            label="Chat"
            badge={agentBusy ? <CircleNotch size={12} className="shrink-0 animate-spin text-muted-foreground" /> : null}
          />
          <NavItem to="/calendar" icon={CalendarBlank} label="Calendar" />
          {/* One row for Projects and Tasks: lands on `/projects`, lights on
              `/tasks` too. */}
          <NavItem
            to="/projects"
            match={["/tasks"]}
            icon={ListChecks}
            label="Tasks"
          />
        </div>

        <Rule />

        {/* Scrollbar hidden: an overflow bar would jog every row sideways, so
            the fades carry the affordance. */}
        <div className="relative flex-1 min-h-0">
          <div
            ref={scrollRef}
            className="h-full overflow-y-auto overflow-x-hidden px-2 py-2 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
          >
            {/* One wrapper: the ResizeObserver watches the first child. */}
            <div className="space-y-3">
              <RecentNavGroup />
              <SubjectsNavGroup />
            </div>
          </div>
        </div>

        <Rule />

        <div className="pt-1.5 pb-2 px-2 shrink-0">
          <NavItem to="/settings" icon={GearSix} label="Settings" />
          <NavItem
            to="/sync"
            icon={ArrowsClockwise}
            label="Sync"
            badge={
              indexing ? (
                <CircleNotch size={12} className="shrink-0 animate-spin text-muted-foreground" />
              ) : null
            }
          />
        </div>
      </div>
    </aside>
  );
}

/** The ⌘K palette's handle, always visible in the header. */
function SearchButton() {
  const setOpen = usePaletteStore((s) => s.setOpen);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-label="Search"
          className={cn(
            "flex h-6 w-6 shrink-0 items-center justify-center rounded-md",
            "text-muted-foreground hover:text-foreground hover:bg-sidebar-item-hover active:bg-sidebar-item-active",
            "transition-colors duration-150",
          )}
        >
          <MagnifyingGlass size={15} />
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" align="end" className="flex flex-col items-start gap-0.5">
        Search
        <span className="text-[11px] text-background/60">⌘K</span>
      </TooltipContent>
    </Tooltip>
  );
}

/** Inset hairline, short of both edges so it reads as a seam. */
function Rule() {
  return <div aria-hidden className="mx-3 h-px shrink-0 bg-sidebar-border/70" />;
}
