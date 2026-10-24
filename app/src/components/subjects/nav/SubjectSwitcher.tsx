import { useRef, useState, type ReactNode } from "react";
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
  const { pathname } = useLocation();
  return (
    <SubjectSwitcherMenu
      label={displayCode(subject.code)}
      selected={String(subject.id)}
      current={current}
      past={past}
      hrefFor={(s) => switchPath(pathname, s.id)}
      newCounts
    />
  );
}

/**
 * The header's switcher: `label` as a button spanning the row that opens
 * current subjects, then past ones folded. Without
 * `icon` the slot stays empty for a trigger the caller draws over it. `field`
 * draws the trigger as the pill field that sits under a column's heading.
 */
export function SubjectSwitcherMenu({
  label,
  icon,
  selected,
  current,
  past,
  hrefFor,
  field = false,
  newCounts = false,
}: {
  label: string;
  icon?: ReactNode;
  /** The open subject's id. */
  selected: string;
  current: Subject[];
  past: Subject[];
  hrefFor: (s: Subject) => string;
  field?: boolean;
  /** Each subject's unopened-file count, as the Subjects section shows it. */
  newCounts?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();

  const go = (key: string, href: string) => {
    setOpen(false);
    if (key !== selected) navigate(href);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          title="Switch subject"
          aria-label="Switch subject"
          className={cn(
            "flex w-full min-w-0 items-center overflow-hidden text-left transition-colors",
            field
              ? cn("h-8 gap-2 rounded-full border border-input bg-card px-3 hover:border-ring/60", SIDE_NAV_FOLDS)
              : "gap-2.5 rounded-md px-2 py-1 hover:bg-sidebar-item-hover",
          )}
        >
          <span aria-hidden className="flex size-4 shrink-0 items-center justify-center">
            {icon}
          </span>
          <span
            className={cn(
              "min-w-0 flex-1 truncate text-foreground",
              field
                ? "text-[13px] font-medium"
                : "font-display text-[16px] font-semibold leading-none tracking-tight",
              SIDE_NAV_FOLDS,
            )}
          >
            {label}
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
          selected={selected}
          current={current}
          past={past}
          hrefFor={hrefFor}
          newCounts={newCounts}
          onPick={go}
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
  selected,
  current,
  past,
  hrefFor,
  newCounts,
  onPick,
}: {
  selected: string;
  current: Subject[];
  past: Subject[];
  hrefFor: (s: Subject) => string;
  newCounts: boolean;
  onPick: (key: string, href: string) => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  useScrollFade(scrollRef);
  const counts = useNewFilesStore((s) => s.bySubject);
  // Collapsed unless the open subject is itself a past one, so it stays visible.
  const [pastOpen, setPastOpen] = useState(() => past.some((s) => String(s.id) === selected));

  const option = (key: string, label: string, title: string, icon: ReactNode, href: string, trailing?: ReactNode) => (
    <button
      key={key}
      type="button"
      title={title}
      data-tab-href={href}
      onClick={() => onPick(key, href)}
      className={cn(SWITCH_ROW, key === selected ? "bg-accent text-foreground" : SWITCH_ROW_IDLE)}
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {trailing}
    </button>
  );
  const row = (s: Subject) =>
    option(
      String(s.id),
      displayCode(s.code),
      displayName(s.name, s.code),
      <SubjectIcon code={s.code} size={14} />,
      hrefFor(s),
      newCounts && <NewCountBadge count={newCountForSubject(counts, s.id)} />,
    );

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
