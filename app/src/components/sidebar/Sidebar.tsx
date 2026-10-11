import {
  Chat,
  ChatsCircle,
  CalendarBlank,
  ArrowsClockwise,
  BookOpen,
  CircleNotch,
  ListChecks,
  MagnifyingGlass,
  GearSix,
  House,
} from "@phosphor-icons/react";
import { anyRunning, useHarnessStore } from "@/stores/chat/harnessStore";
import { useIndexStore } from "@/stores/sync/indexStore";
import { useJobAttention } from "@/stores/agents/jobAttentionStore";
import { newCountForSubject, useNewFilesStore } from "@/stores/sync/newFilesStore";
import { usePaletteStore } from "@/stores/shell/paletteStore";
import { useRailOrder, type RailSection } from "@/stores/shell/railOrderStore";
import { useDockPrefsStore } from "@/stores/shell/dockPrefsStore";
import { totalUnread, useDockStore } from "@/stores/shell/dockStore";
import { NewCountBadge } from "@/components/NewCountBadge";
import { DOCK_ROOT } from "@/lib/dock";
import { DRAG_SURFACE, useStripReorder } from "@/hooks/gestures/usePointerDrag";
import { cn } from "@/lib/utils";
import RailItem, { railButton, railIdle } from "./RailItem";
import { MusicTile, useMusicTileShown } from "./MusicTile";
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
 * An icon rail: search, the sections, Messaging while a service is on and
 * Spotify's tile while music is on, and Sync and Settings pinned to the foot.
 * The sections drag into any order, kept in `railOrderStore`. Collapsed
 * animates to zero width; the title bar's button is the only toggle. No fill
 * or divider of its own — see docs/ui.md (UI system).
 */
export default function Sidebar({ collapsed }: SidebarProps) {
  const width = collapsed ? 0 : WIDTH;
  // Background jobs surface only in the sidebar (see docs/ui.md): a spinner.
  const indexing = useIndexStore((s) => s.running);
  // A job's model left its agent's list or its CLI is missing (`useJobAudit`).
  const jobsNeedAttention = useJobAttention((s) => s.attention);

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

        <Sections />

        <AppItems />

        <div className="flex-1" />

        <Rule />

        <RailItem
          to="/sync"
          icon={ArrowsClockwise}
          label="Sync"
          indicator={indexing ? <Spinner /> : null}
        />
        <RailItem
          to="/settings"
          icon={GearSix}
          label="Settings"
          indicator={
            jobsNeedAttention ? <span className="size-1.5 rounded-full bg-destructive" /> : null
          }
        />
      </div>
    </aside>
  );
}

/** The reorderable middle of the rail. */
function Sections() {
  const order = useRailOrder((s) => s.order);
  const setOrder = useRailOrder((s) => s.setOrder);
  const agentBusy = useHarnessStore((s) => anyRunning(s.live));
  const anyNew = useNewFilesStore((s) =>
    Object.keys(s.bySubject).some((id) => newCountForSubject(s.bySubject, Number(id)) > 0),
  );
  const strip = useStripReorder({
    keys: order,
    axis: "y",
    // Equal-sized items: a clamped centre never passes the end ones.
    swapOn: "edge",
    onDrop: ({ order }) => setOrder(order),
  });

  const items: Record<RailSection, React.ReactNode> = {
    home: <RailItem to="/" icon={House} label="Home" />,
    chat: (
      <RailItem
        to="/chat"
        icon={Chat}
        label="Chat"
        indicator={agentBusy ? <Spinner /> : null}
      />
    ),
    calendar: <RailItem to="/calendar" icon={CalendarBlank} label="Calendar" />,
    // One item for Projects and Tasks: lands on `/projects`, lights on `/tasks` too.
    tasks: <RailItem to="/projects" match={["/tasks"]} icon={ListChecks} label="Tasks" />,
    subjects: (
      <RailItem
        to="/subjects"
        icon={BookOpen}
        label="Subjects"
        indicator={anyNew ? <span className="size-1.5 rounded-full bg-brand" /> : null}
      />
    ),
  };

  return (
    <div className={cn("mt-2 flex flex-col items-center gap-1", DRAG_SURFACE)}>
      {order.map((key, i) => {
        const grabbed = strip.drag?.key === key;
        return (
          <div
            key={key}
            ref={strip.itemRef(key)}
            onPointerDown={(e) => strip.onPointerDown(e, key)}
            // Swallow the click a drag's release raises, before the button hears it.
            onClickCapture={(e) => {
              if (strip.didDrag()) e.stopPropagation();
            }}
            style={strip.styleFor(key, i)}
            className={cn(
              "flex",
              grabbed
                ? "relative z-10"
                : strip.drag && "transition-transform duration-200 ease-out",
            )}
          >
            {items[key]}
          </div>
        );
      })}
    </div>
  );
}

/** Messaging while any service is on (its page switches between them),
 *  badged with the unread total across them, then Spotify's tile, under one
 *  seam shown while either is. */
function AppItems() {
  const enabled = useDockPrefsStore((s) => s.enabled);
  const unread = useDockStore((s) => totalUnread(s.pages, enabled));
  const music = useMusicTileShown();
  const messaging = enabled.length > 0;
  if (!messaging && !music) return null;
  return (
    <>
      <Rule />
      {messaging && (
        <RailItem
          to={DOCK_ROOT}
          icon={ChatsCircle}
          label="Messaging"
          indicator={
            unread > 0 ? (
              // Out to the button's corner: a count is wider than the slot's dots.
              <span className="-mt-1 -mr-1 flex">
                <NewCountBadge count={unread} />
              </span>
            ) : null
          }
        />
      )}
      <MusicTile />
    </>
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
