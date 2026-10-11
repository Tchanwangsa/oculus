import { useEffect, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { SearchGlyph } from "@/components/search/SearchList";
import { SIDE_NAV_ROW, SIDE_NAV_ROW_IDLE, SideNavGroupLabel } from "@/components/ui/layout/SideNav";
import { useSearch } from "@/hooks/find/useSearch";
import type { Subject } from "@/lib/db";
import { displayCode } from "@/lib/format/format";
import { openSearchItem, type SearchFilter, type SearchItem, type SearchSection } from "@/lib/search";
import { matchesAll } from "@/lib/search/filters";
import { openBeside, openUrlInFocusedPane } from "@/lib/shell/tabRouters";
import { cn } from "@/lib/utils";
import { useTabStore } from "@/stores/shell/tabStore";
import { TABS } from "@/components/subjects/nav/tabs";

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
export function SubjectSearchResults({
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
                // ⌘-click opens a new tab (`app/src/lib/shell/newTabClicks.ts`).
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
