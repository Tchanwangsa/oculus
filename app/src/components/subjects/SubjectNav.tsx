import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { useLocation, useNavigate } from "react-router-dom";
import {
  CaretRight,
  CaretUpDown,
  ChatsCircle,
  Folder,
  House,
  Kanban,
  Megaphone,
  PencilLine,
  Stack,
  VideoCamera,
} from "@phosphor-icons/react";

import { NewCountBadge } from "@/components/NewCountBadge";
import { SearchGlyph } from "@/components/search/SearchList";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SubjectIconPicker } from "@/components/subjects/SubjectIconPicker";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_IDLE,
  SideNav,
  SideNavCollapseToggle,
  SideNavGroupLabel,
  SideNavLink,
  SideNavSearch,
  SideNavTip,
  SIDE_NAV_COLLAPSED_WIDTH,
  SIDE_NAV_FOLDS,
} from "@/components/ui/SideNav";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import { Skeleton } from "@/components/ui/skeleton";
import { useScrollFade } from "@/hooks/useScrollFade";
import { useSearch } from "@/hooks/useSearch";
import { useResizablePanel } from "@/hooks/useResizablePanel";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format";
import { openSearchItem, type SearchFilter, type SearchItem, type SearchSection } from "@/lib/search";
import { matchesAll } from "@/lib/searchFilters";
import { openBeside, openUrlInFocusedPane } from "@/lib/tabRouters";
import { cn } from "@/lib/utils";
import { useTabStore } from "@/stores/tabStore";
import { newCountForSubject, newCountForTab, useNewFilesStore } from "@/stores/newFilesStore";

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

/** One width and fold for every subject: they are layout choices. */
const PANEL = {
  defaultWidth: 208,
  minWidth: 168,
  maxWidth: 320,
  collapsedWidth: SIDE_NAV_COLLAPSED_WIDTH,
  storageKey: "oculus-subject-nav-width",
};

/**
 * `id`'s page for the tab open at `pathname`. Only the top-level tab carries
 * over; anything deeper (a Files sub-tab) lands on that tab's default.
 */
function switchPath(pathname: string, id: number): string {
  const tab = pathname.split("/")[3]; // ["", "subjects", ":id", tab, …]
  return tab && TAB_PATHS.has(tab) ? `/subjects/${id}/${tab}` : `/subjects/${id}`;
}

/** The subject page's nav column: who it is, a switcher, a search over the
 *  subject, and its tabs — replaced by the results while the search has text.
 *  Collapsed, it keeps the switcher and tabs as icons. */
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
  const [query, setQuery] = useState("");
  // The results own their list; Enter in the field opens its first row.
  const openFirst = useRef<() => void>(() => {});
  useEffect(() => setQuery(""), [subject.id]);
  const panel = useResizablePanel(PANEL);
  const { collapsed, setCollapsed } = panel;
  const searchRef = useRef<HTMLInputElement>(null);
  // Set by the folded search pill: focus the field once it unfolds.
  const focusSearch = useRef(false);
  useEffect(() => {
    if (collapsed || !focusSearch.current) return;
    focusSearch.current = false;
    searchRef.current?.focus();
  }, [collapsed]);

  const toggle = () => {
    setQuery("");
    panel.toggle();
  };

  return (
    <>
      <SideNav
        aria-label={displayCode(subject.code)}
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        footer={<SideNavCollapseToggle collapsed={collapsed} onToggle={toggle} />}
        header={
          <>
            <div className="px-4 pt-4 pb-3">
              {/* The switcher spans the row, icon included; the icon's own
                  trigger sits over its slot, since triggers can't nest. Folded,
                  the switcher takes the whole icon. */}
              <div className="relative -mx-2">
                <SubjectSwitcher subject={subject} current={current} past={past} collapsed={collapsed} />
                <SubjectIconPicker code={subject.code}>
                  <button
                    type="button"
                    aria-label="Change subject icon"
                    className="absolute top-1/2 left-1 -translate-y-1/2 rounded-md p-1 transition-colors hover:bg-sidebar-item-hover group-data-[collapsed=true]/nav:pointer-events-none"
                  >
                    <SubjectIcon code={subject.code} size={16} />
                  </button>
                </SubjectIconPicker>
              </div>
              <p className={cn("mt-1.5 truncate text-[12px] leading-snug text-muted-foreground", SIDE_NAV_FOLDS)} title={name}>
                {name}
              </p>
              {!subject.is_current && (
                <span className={cn("mt-2 inline-block whitespace-nowrap rounded bg-surface-raised px-2 py-0.5 text-[10px] uppercase tracking-wide text-muted-foreground", SIDE_NAV_FOLDS)}>
                  {subject.term_name ?? "Past"}
                </span>
              )}
            </div>
            <SideNavSearch
              value={query}
              onChange={setQuery}
              onEnter={() => openFirst.current()}
              onExpand={() => {
                focusSearch.current = true;
                setCollapsed(false);
              }}
              placeholder={`Search ${displayCode(subject.code)}`}
              inputRef={searchRef}
            />
          </>
        }
      >
        {query.trim() ? (
          // Mounted only while searching, so an idle column runs no queries.
          <SubjectSearchResults
            subject={subject}
            current={current}
            query={query}
            openFirst={openFirst}
            onOpened={() => setQuery("")}
          />
        ) : TABS.map((tab, index) => (
          <SideNavLink
            key={tab.to}
            to={tab.to}
            end={tab.end}
            icon={tab.icon}
            label={tab.label}
            trailing={<NewCountBadge count={newCounts[index]} />}
            dot={newCounts[index] > 0}
          />
        ))}
      </SideNav>
      {/* On the seam: negative margins cost no layout width. */}
      <ResizeHandle
        onMouseDown={panel.onMouseDown}
        dragging={panel.dragging}
        label="Resize subject sidebar"
        className="-mx-0.5"
      />
    </>
  );
}

/** The subject's tabs a search names, as rows of the same kind as results. */
function tabItems(subjectId: number, query: string): SearchSection {
  return {
    heading: "Go to",
    items: TABS.filter((tab) => matchesAll(tab.label, query)).map((tab) => ({
      key: `tab:${tab.to}`,
      icon: { kind: "glyph", icon: tab.icon },
      label: tab.label,
      target: {
        kind: "route",
        path: tab.to === "." ? `/subjects/${subjectId}` : `/subjects/${subjectId}/${tab.to}`,
      },
    })),
  };
}

/** A row's second line, less the subject code every row here shares. */
function rowDetail(item: SearchItem, code: string): string {
  const meta = item.meta ?? "";
  const rest = meta === code ? "" : meta.startsWith(`${code} · `) ? meta.slice(code.length + 3) : meta;
  const snippet = item.snippet?.map((p) => p.text).join("") ?? "";
  return [rest, snippet].filter(Boolean).join(" · ");
}

/** A document opens beside the page, as a file row does; anything else
 *  replaces it. */
const BESIDE = /^\/subjects\/\d+\/(file|lecture)\b/;

/** `runSearch` under an `in:` filter for this subject, after its matching tabs. */
function SubjectSearchResults({
  subject,
  current,
  query,
  openFirst,
  onOpened,
}: {
  subject: Subject;
  current: Subject[];
  query: string;
  openFirst: React.RefObject<() => void>;
  onOpened: () => void;
}) {
  const navigate = useNavigate();
  const addTab = useTabStore((s) => s.addTab);
  // Stable, or `useSearch` re-runs every render.
  const filters = useMemo<SearchFilter[]>(() => [{ key: "in", subject }], [subject]);
  const found = useSearch(query, { subjects: current, current, filters, noWeb: true });
  const sections = useMemo(
    () => [tabItems(subject.id, query), ...found].filter((s) => s.items.length > 0),
    [subject.id, query, found],
  );
  const code = displayCode(subject.code);

  const open = (item: SearchItem) => {
    openSearchItem(item, {
      newTab: false,
      navigate: (path) => (BESIDE.test(path) ? openBeside(path) : navigate(path)),
      addTab,
      openUrl: (url) => void openUrlInFocusedPane(url),
    });
    onOpened();
  };

  const first = sections[0]?.items[0];
  useEffect(() => {
    openFirst.current = () => {
      if (first) open(first);
    };
  });

  if (sections.length === 0) {
    return <p className="px-2 py-1.5 text-[12.5px] text-muted-foreground">No matches in {code}</p>;
  }

  return (
    <div className="space-y-3">
      {sections.map((section) => (
        <div key={section.heading}>
          <SideNavGroupLabel>{section.heading}</SideNavGroupLabel>
          {section.items.map((item) => {
            const detail = rowDetail(item, code);
            return (
              <button
                key={item.key}
                type="button"
                // ⌘-click opens a new tab (`app/src/lib/newTabClicks.ts`).
                data-tab-href={item.target.kind === "route" ? item.target.path : undefined}
                onClick={() => open(item)}
                className={cn(SIDE_NAV_ROW, SIDE_NAV_ROW_IDLE, "items-start text-left")}
              >
                <span className="mt-px shrink-0">
                  <SearchGlyph spec={item.icon} size={16} />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate">{item.label}</span>
                  {detail && (
                    <span className="block truncate text-[11px] text-muted-foreground">{detail}</span>
                  )}
                </span>
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}

/** The column's shape while the subject loads. */
export function SubjectNavSkeleton() {
  const panel = useResizablePanel(PANEL);
  return (
    <SideNav
      width={panel.width}
      collapsed={panel.collapsed}
      header={
        <>
          <div className="space-y-2 px-4 pt-4 pb-3">
            <Skeleton className="h-4 w-28" />
            <Skeleton className="h-3 w-32" />
          </div>
          <div className="px-2 pb-2">
            <Skeleton className="h-8 w-full rounded-full" />
          </div>
        </>
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
  collapsed = false,
}: {
  subject: Subject;
  current: Subject[];
  past: Subject[];
  /** Folded to the icon: the list opens to the right instead. */
  collapsed?: boolean;
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
      <SideNavTip label={displayCode(subject.code)}>
        <PopoverTrigger asChild>
          <button
            type="button"
            title={collapsed ? undefined : "Switch subject"}
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
      </SideNavTip>
      {/* As wide as the trigger, which spans the header row; collapsed, the
          width that row has when expanded. */}
      <PopoverContent
        side={collapsed ? "right" : "bottom"}
        align="start"
        sideOffset={8}
        className={cn(collapsed ? "w-48" : "w-(--radix-popover-trigger-width)", "p-1.5")}
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
