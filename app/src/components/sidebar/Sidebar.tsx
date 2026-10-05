import {
  Chat,
  CalendarBlank,
  ArrowsClockwise,
  BookOpen,
  CircleNotch,
  ListChecks,
  MagnifyingGlass,
  GearSix,
  House,
} from "@phosphor-icons/react";
import { anyRunning, useHarnessStore } from "@/stores/harnessStore";
import { useIndexStore } from "@/stores/indexStore";
import { newCountForSubject, useNewFilesStore } from "@/stores/newFilesStore";
import { usePaletteStore } from "@/stores/paletteStore";
import { cn } from "@/lib/utils";
import RailItem, { railButton, railIdle } from "./RailItem";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface SidebarProps {
  collapsed: boolean;
}

const WIDTH = 52;

/**
 * An icon rail: search, the sections, then Sync and Settings pinned to the
 * foot. Collapsed animates to zero width; the title bar's button is the only
 * toggle. No fill or divider of its own — see docs/ui.md (UI system).
 */
export default function Sidebar({ collapsed }: SidebarProps) {
  const width = collapsed ? 0 : WIDTH;
  // Background jobs surface only in the sidebar (see docs/ui.md): a spinner.
  const agentBusy = useHarnessStore((s) => anyRunning(s.live));
  const indexing = useIndexStore((s) => s.running);
  const anyNew = useNewFilesStore((s) =>
    Object.keys(s.bySubject).some((id) => newCountForSubject(s.bySubject, Number(id)) > 0),
  );

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
      <div
        className="flex flex-col items-center h-full gap-1 pb-1"
        style={{ width: WIDTH, minWidth: WIDTH }}
      >
        <SearchButton />

        <div className="mt-2 flex flex-col items-center gap-1">
          <RailItem to="/" icon={House} label="Home" />
          <RailItem
            to="/chat"
            icon={Chat}
            label="Chat"
            indicator={agentBusy ? <Spinner /> : null}
          />
          <RailItem to="/calendar" icon={CalendarBlank} label="Calendar" />
          {/* One item for Projects and Tasks: lands on `/projects`, lights on
              `/tasks` too. */}
          <RailItem to="/projects" match={["/tasks"]} icon={ListChecks} label="Tasks" />
          <RailItem
            to="/subjects"
            icon={BookOpen}
            label="Subjects"
            indicator={anyNew ? <span className="size-1.5 rounded-full bg-brand" /> : null}
          />
        </div>

        <div className="flex-1" />

        <Rule />

        <RailItem
          to="/sync"
          icon={ArrowsClockwise}
          label="Sync"
          indicator={indexing ? <Spinner /> : null}
        />
        <RailItem to="/settings" icon={GearSix} label="Settings" />
      </div>
    </aside>
  );
}

function Spinner() {
  return <CircleNotch size={10} weight="bold" className="animate-spin text-brand" />;
}

/** The ⌘K palette's handle, at the top of the rail. */
function SearchButton() {
  const setOpen = usePaletteStore((s) => s.setOpen);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={() => setOpen(true)}
          aria-label="Search"
          className={cn(railButton, railIdle, "active:bg-sidebar-item-active")}
        >
          <MagnifyingGlass size={18} />
        </button>
      </TooltipTrigger>
      <TooltipContent side="right" className="flex flex-col items-start gap-0.5">
        Search
        <span className="text-[11px] text-background/60">⌘K</span>
      </TooltipContent>
    </Tooltip>
  );
}

/** Inset hairline, short of both edges so it reads as a seam. */
function Rule() {
  return <div aria-hidden className="w-6 h-px my-1 shrink-0 bg-sidebar-border/70" />;
}
