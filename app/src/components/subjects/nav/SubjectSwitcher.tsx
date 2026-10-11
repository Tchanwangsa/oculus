import { useRef, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { CaretRight, CaretUpDown } from "@phosphor-icons/react";
import { NewCountBadge } from "@/components/NewCountBadge";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SIDE_NAV_FOLDS } from "@/components/ui/layout/SideNav";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { newCountForSubject, useNewFilesStore } from "@/stores/sync/newFilesStore";
import { TAB_PATHS } from "@/components/subjects/nav/tabs";

/**
 * `id`'s page for the tab open at `pathname`. Only the top-level tab carries
 * over; anything deeper (a Files sub-tab) lands on that tab's default.
 */
function switchPath(pathname: string, id: number): string {
  const tab = pathname.split("/")[3]; // ["", "subjects", ":id", tab, …]
  return tab && TAB_PATHS.has(tab) ? `/subjects/${id}/${tab}` : `/subjects/${id}`;
}

/** The code as a button that opens every subject, to swap this page's subject. */
export function SubjectSwitcher({
  subject,
  current,
  past,
}: {
  subject: Subject;
  current: Subject[];
  past: Subject[];
}) {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const { pathname } = useLocation();

  const go = (path: string) => {
    setOpen(false);
    navigate(path);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          title="Switch subject"
          aria-label="Switch subject"
          className="flex w-full min-w-0 items-center gap-2.5 overflow-hidden rounded-md px-2 py-1 text-left transition-colors hover:bg-sidebar-item-hover"
        >
          {/* The icon's slot; its button is drawn over it by the header. */}
          <span aria-hidden className="size-4 shrink-0" />
          <span className={cn("min-w-0 flex-1 truncate font-display text-[16px] font-semibold leading-none tracking-tight text-foreground", SIDE_NAV_FOLDS)}>
            {displayCode(subject.code)}
          </span>
          <CaretUpDown size={12} className={cn("shrink-0 text-muted-foreground/60", SIDE_NAV_FOLDS)} />
        </button>
      </PopoverTrigger>
      {/* As wide as the trigger, which spans the header row. */}
      <PopoverContent
        side="bottom"
        align="start"
        sideOffset={8}
        className="w-(--radix-popover-trigger-width) p-1.5"
      >
        <SwitcherList
          subject={subject}
          current={current}
          past={past}
          hrefFor={(s) => switchPath(pathname, s.id)}
          onPick={(s) => (s.id === subject.id ? setOpen(false) : go(switchPath(pathname, s.id)))}
        />
      </PopoverContent>
    </Popover>
  );
}

const SWITCH_ROW =
  "flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors focus-visible:bg-accent focus-visible:outline-none";
const SWITCH_ROW_IDLE = "text-muted-foreground hover:bg-accent hover:text-foreground";

/** Its own component so the scroller's ref exists when the fade hook runs. */
function SwitcherList({
  subject,
  current,
  past,
  hrefFor,
  onPick,
}: {
  subject: Subject;
  current: Subject[];
  past: Subject[];
  hrefFor: (s: Subject) => string;
  onPick: (s: Subject) => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef);
  const counts = useNewFilesStore((s) => s.bySubject);
  // Collapsed unless the open subject is itself a past one, so it stays visible.
  const [pastOpen, setPastOpen] = useState(() => past.some((s) => s.id === subject.id));

  const row = (s: Subject) => {
    const selected = s.id === subject.id;
    return (
      <button
        key={s.id}
        type="button"
        title={displayName(s.name, s.code)}
        data-tab-href={hrefFor(s)}
        onClick={() => onPick(s)}
        className={cn(SWITCH_ROW, selected ? "bg-accent text-foreground" : SWITCH_ROW_IDLE)}
      >
        <SubjectIcon code={s.code} size={14} />
        <span className="min-w-0 flex-1 truncate">{displayCode(s.code)}</span>
        <NewCountBadge count={newCountForSubject(counts, s.id)} />
      </button>
    );
  };

  // `will-change` keeps the scroller composited from the start: WebKit only
  // promotes it once it overflows, which re-snaps every icon by half a pixel
  // at a fractional page zoom.
  return (
    <div ref={scrollRef} className="max-h-80 select-none overflow-y-scroll will-change-transform">
      {/* One wrapper: the fade's ResizeObserver watches the first child. */}
      <div>
        {current.map(row)}
        {past.length > 0 && (
          <div className={cn(current.length > 0 && "mt-2")}>
            <button
              type="button"
              aria-expanded={pastOpen}
              onClick={() => setPastOpen((o) => !o)}
              className="mb-0.5 flex w-full cursor-pointer items-center gap-1 px-2 py-1 text-left text-[11px] font-medium tracking-wide text-muted-foreground transition-colors hover:text-foreground focus-visible:text-foreground focus-visible:outline-none"
            >
              <CaretRight size={10} className={cn("shrink-0 transition-transform", pastOpen && "rotate-90")} />
              Past subjects ({past.length})
            </button>
            {pastOpen && past.map(row)}
          </div>
        )}
      </div>
    </div>
  );
}
