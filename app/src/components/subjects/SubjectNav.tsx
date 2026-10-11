import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";

import { NewCountBadge } from "@/components/NewCountBadge";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { SubjectIconPicker } from "@/components/subjects/SubjectIconPicker";
import { SubjectSearchResults } from "@/components/subjects/nav/SubjectSearchResults";
import { SubjectSwitcher } from "@/components/subjects/nav/SubjectSwitcher";
import { TABS } from "@/components/subjects/nav/tabs";
import {
  SideNav,
  SideNavLink,
  SideNavSearch,
  SIDE_NAV_FOLDS,
} from "@/components/ui/layout/SideNav";
import { ResizeHandle } from "@/components/ui/layout/ResizeHandle";
import { Skeleton } from "@/components/ui/skeleton";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { useResizablePanel } from "@/hooks/gestures/useResizablePanel";
import { useTabActive } from "@/components/tabs/TabContext";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format/format";
import { cn } from "@/lib/utils";
import { newCountForTab, useNewFilesStore } from "@/stores/sync/newFilesStore";

/** One width and fold for every subject: they are layout choices. */
export const SUBJECT_NAV_PANEL = {
  defaultWidth: 208,
  minWidth: 168,
  maxWidth: 320,
  collapsedWidth: 0,
  storageKey: "oculus-subject-nav-width",
};

type Panel = ReturnType<typeof useResizablePanel>;

/** The tab open at `pathname`, for the page header's title. */
export function subjectTabLabel(pathname: string): string {
  const tab = pathname.split("/")[3]; // ["", "subjects", ":id", tab, …]
  return TABS.find((t) => t.to === (tab || "."))?.label ?? "Overview";
}

/** The subject page's nav column: who it is, a switcher, a search over the
 *  subject, and its tabs — replaced by the results while the search has text.
 *  Its width and fold come from `panel` (`SubjectLayout` owns it); ⌘⌥B or the
 *  header row's toggle folds it away completely, as Chat's column does. */
export function SubjectNav({
  subject,
  current,
  past,
  panel,
}: {
  subject: Subject;
  current: Subject[];
  past: Subject[];
  panel: Panel;
}) {
  const newCounts = useNewFilesStore(useShallow((s) =>
    TABS.map((tab) => newCountForTab(s.bySubject, subject.id, tab.to)),
  ));
  const name = displayName(subject.name, subject.code);
  const [query, setQuery] = useState("");
  // The results own their list; Enter in the field opens its first row.
  const openFirst = useRef<() => void>(() => {});
  useEffect(() => setQuery(""), [subject.id]);
  const { collapsed } = panel;
  // Every tab stays mounted, so only the one in front answers the chord.
  // `e.code`, not `e.key`: on macOS ⌥B arrives as `∫`.
  const active = useTabActive();
  useWindowEvent("keydown", (e) => {
    const k = e as KeyboardEvent;
    if (!active || !k.altKey || !(k.metaKey || k.ctrlKey) || k.code !== "KeyB") return;
    k.preventDefault();
    panel.toggle();
  });

  return (
    <>
      <SideNav
        aria-label={displayCode(subject.code)}
        width={panel.width}
        collapsed={collapsed}
        animate={!panel.dragging}
        header={
          <>
            <div className="px-4 pt-4 pb-3">
              {/* The switcher spans the row, icon included; the icon's own
                  trigger sits over its slot, since triggers can't nest. */}
              <div className="relative -mx-2">
                <SubjectSwitcher subject={subject} current={current} past={past} />
                <SubjectIconPicker code={subject.code}>
                  <button
                    type="button"
                    aria-label="Change subject icon"
                    className="absolute top-1/2 left-1 -translate-y-1/2 rounded-md p-1 transition-colors hover:bg-sidebar-item-hover"
                  >
                    <SubjectIcon code={subject.code} size={16} />
                  </button>
                </SubjectIconPicker>
              </div>
              <p className={cn("mt-1.5 truncate text-[12px] leading-snug text-muted-foreground", SIDE_NAV_FOLDS)} title={name}>
                {name}
              </p>
            </div>
            <SideNavSearch
              value={query}
              onChange={setQuery}
              onEnter={() => openFirst.current()}
              placeholder={`Search ${displayCode(subject.code)}`}
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
          />
        ))}
      </SideNav>
      {/* On the seam: negative margins cost no layout width. Folded away the
          seam is the page's own left edge, where a side panel's handle lives. */}
      {!collapsed && (
        <ResizeHandle
          onMouseDown={panel.onMouseDown}
          dragging={panel.dragging}
          label="Resize subject sidebar"
          className="-mx-0.5"
        />
      )}
    </>
  );
}

/** The column's shape while the subject loads. */
export function SubjectNavSkeleton() {
  const panel = useResizablePanel(SUBJECT_NAV_PANEL);
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
