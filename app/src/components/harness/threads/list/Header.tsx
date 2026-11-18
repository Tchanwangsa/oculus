import { BookOpen, ClockCounterClockwise, NotePencil, type Icon } from "@phosphor-icons/react";
import { SIDE_NAV_FOLDS, SideNavSearch } from "@/components/ui/layout/SideNav";
import { cn } from "@/lib/utils";

export type ThreadView = "recents" | "subject";

const VIEWS: { value: ThreadView; label: string; icon: Icon }[] = [
  { value: "recents", label: "Recents", icon: ClockCounterClockwise },
  { value: "subject", label: "Subject", icon: BookOpen },
];

/** Anything but "subject" opens on Recents: unknown, missing, or
 *  "status". */
export const readThreadView = (raw: string | null): ThreadView => (raw === "subject" ? "subject" : "recents");

/**
 * The column's header: the search field with the new-chat button at its
 * right (lit while no thread is open), then the view switch under them,
 * which steps aside while a search shows its results.
 */
export function ListHeader({
  view,
  onView,
  query,
  onQuery,
  onSearchEnter,
  fresh,
  onNew,
  collapsed,
}: {
  view: ThreadView;
  onView: (view: ThreadView) => void;
  query: string;
  onQuery: (query: string) => void;
  /** Enter in the field: open the first result. */
  onSearchEnter: () => void;
  /** No thread is open: bare `/chat` is the new chat. */
  fresh: boolean;
  onNew: () => void;
  collapsed: boolean;
}) {
  const searching = query.trim() !== "";
  return (
    <div className="flex flex-col gap-2 pt-3 pr-2 pb-2 pl-2.5">
      <div className="flex items-center gap-1.5">
        <SideNavSearch
          value={query}
          onChange={onQuery}
          onEnter={onSearchEnter}
          placeholder="Search chats"
          className="min-w-0 flex-1 p-0"
        />
        <button
          type="button"
          data-side-nav-close
          aria-label="New chat"
          title="New chat"
          aria-current={fresh ? "page" : undefined}
          onClick={onNew}
          className={cn(
            "flex size-[30px] shrink-0 items-center justify-center rounded-lg transition-colors",
            fresh
              ? "bg-sidebar-item-active text-foreground"
              : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
          )}
        >
          <NotePencil size={17} />
        </button>
      </div>
      {!searching && (
        <div
          role="tablist"
          aria-label="Group threads by"
          className={cn("flex min-w-0 gap-0.5 rounded-[9px] border border-border-subtle bg-accent p-0.5", SIDE_NAV_FOLDS)}
        >
          {VIEWS.map(({ value, label, icon: IconC }) => {
            const on = value === view;
            return (
              <button
                key={value}
                type="button"
                role="tab"
                aria-selected={on}
                tabIndex={collapsed ? -1 : undefined}
                onClick={() => onView(value)}
                className={cn(
                  "flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-[7px] py-[3px] text-[12px] font-medium transition-colors",
                  on ? "bg-popover text-foreground shadow-xs" : "text-muted-foreground hover:text-foreground",
                )}
              >
                <IconC size={14} className="shrink-0" />
                <span className="truncate">{label}</span>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
