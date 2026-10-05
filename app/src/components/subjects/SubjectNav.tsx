import { useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { useLocation, useNavigate } from "react-router-dom";
import {
  CaretUpDown,
  ChatsCircle,
  Check,
  Folder,
  House,
  Kanban,
  Megaphone,
  PencilLine,
  Stack,
  VideoCamera,
} from "@phosphor-icons/react";

import { NewCountBadge } from "@/components/NewCountBadge";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SubjectIconPicker } from "@/components/subjects/SubjectIconPicker";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { SideNav, SideNavGroupLabel, SideNavLink } from "@/components/ui/SideNav";
import { Skeleton } from "@/components/ui/skeleton";
import { useScrollFade } from "@/hooks/useScrollFade";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format";
import { cn } from "@/lib/utils";
import { newCountForTab, useNewFilesStore } from "@/stores/newFilesStore";

const TABS = [
  { to: ".",             label: "Overview",      icon: House,          end: true },
  { to: "modules",       label: "Modules",       icon: Stack,          end: false },
  { to: "lectures",      label: "Lectures",      icon: VideoCamera,    end: false },
  { to: "files",         label: "Files",         icon: Folder,         end: false },
  { to: "announcements", label: "Announcements", icon: Megaphone,      end: false },
  { to: "assignments",   label: "Assignments",   icon: PencilLine,     end: false },
  { to: "discussion",    label: "Discussion",    icon: ChatsCircle,    end: false },
  { to: "projects",      label: "Projects",      icon: Kanban,         end: false },
] as const;

const TAB_PATHS = new Set<string>(TABS.map((tab) => tab.to));

/**
 * `id`'s page for the tab open at `pathname`. Only the top-level tab carries
 * over; anything deeper (a Files sub-tab) lands on that tab's default.
 */
function switchPath(pathname: string, id: number): string {
  const tab = pathname.split("/")[3]; // ["", "subjects", ":id", tab, …]
  return tab && TAB_PATHS.has(tab) ? `/subjects/${id}/${tab}` : `/subjects/${id}`;
}

/** The subject page's nav column: who it is, a switcher, and its tabs. */
export function SubjectNav({
  subject,
  current,
  past,
}: {
  subject: Subject;
  current: Subject[];
  past: Subject[];
}) {
  const newCounts = useNewFilesStore(useShallow((s) =>
    TABS.map((tab) => newCountForTab(s.bySubject, subject.id, tab.to)),
  ));
  const name = displayName(subject.name, subject.code);

  return (
    <SideNav
      aria-label={displayCode(subject.code)}
      header={
        <div className="px-4 pt-4 pb-3">
          {/* Siblings, never nested: each is its own popover's trigger. */}
          <div className="flex items-center gap-2.5">
            <SubjectIconPicker code={subject.code}>
              <button
                type="button"
                aria-label="Change subject icon"
                className="-m-1 shrink-0 rounded-md p-1 transition-colors hover:bg-sidebar-item-hover"
              >
                <SubjectIcon code={subject.code} size={16} />
              </button>
            </SubjectIconPicker>
            <SubjectSwitcher subject={subject} current={current} past={past} />
          </div>
          <p title={name} className="mt-1.5 line-clamp-2 pl-[26px] text-[12px] leading-snug text-muted-foreground">
            {name}
          </p>
          {!subject.is_current && (
            <span className="mt-2 ml-[26px] inline-block rounded bg-surface-raised px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground">
              {subject.term_name ?? "Past"}
            </span>
          )}
        </div>
      }
    >
      {TABS.map((tab, index) => (
        <SideNavLink
          key={tab.to}
          to={tab.to}
          end={tab.end}
          icon={tab.icon}
          label={tab.label}
          trailing={<NewCountBadge count={newCounts[index]} />}
        />
      ))}
    </SideNav>
  );
}

/** The column's shape while the subject loads. */
export function SubjectNavSkeleton() {
  return (
    <SideNav
      header={
        <div className="space-y-2 px-4 pt-4 pb-3">
          <Skeleton className="h-4 w-28" />
          <Skeleton className="ml-[26px] h-3 w-32" />
        </div>
      }
    >
      <div className="space-y-1">
        {TABS.map((tab) => (
          <Skeleton key={tab.to} className="h-7 w-full" />
        ))}
      </div>
    </SideNav>
  );
}

/** The code as a button that opens every subject, to swap this page's subject. */
function SubjectSwitcher({
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
          className="-mx-1 flex min-w-0 items-center gap-1 rounded-md px-1 py-0.5 transition-colors hover:bg-sidebar-item-hover"
        >
          <span className="truncate font-display text-[16px] font-semibold leading-none tracking-tight text-foreground">
            {displayCode(subject.code)}
          </span>
          <CaretUpDown size={12} className="shrink-0 text-muted-foreground/60" />
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" sideOffset={8} className="w-60 p-1.5">
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
  "flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] transition-colors";
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
        {selected && <Check size={12} weight="bold" className="shrink-0 text-brand" />}
      </button>
    );
  };

  return (
    <div ref={scrollRef} className="max-h-80 overflow-y-auto">
      {/* One wrapper: the fade's ResizeObserver watches the first child. */}
      <div>
        {current.map(row)}
        {past.length > 0 && (
          <div className={cn(current.length > 0 && "mt-2")}>
            <SideNavGroupLabel>Past subjects</SideNavGroupLabel>
            {past.map(row)}
          </div>
        )}
      </div>
    </div>
  );
}
