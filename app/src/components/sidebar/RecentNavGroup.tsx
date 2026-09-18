import { useEffect, useState } from "react";
import { CaretRight, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { navigateActive } from "@/lib/tabRouters";
import { useSubjects } from "@/hooks/useSubjects";
import { recentKey, useRecentTabsStore } from "@/stores/recentTabsStore";
import { useActivePath } from "@/stores/tabStore";
import { tabInfo } from "@/components/tabs/tabInfo";

const OPEN_KEY = "oculus-recent-nav-open";
/** The sidebar is a short column shared with the subjects — five is as much
 *  of the trail as earns its place. */
const SHOWN = 5;

/**
 * The Recent group: the last few pages you settled on, named exactly as their
 * tabs are (`tabInfo`). Clicking one goes there in the current tab; ⌘-click
 * opens it in a new one, as the tab strip's own affordances do.
 *
 * Rows do not move while you work — the ordering rules live in
 * `recentTabsStore`.
 */
export default function RecentNavGroup() {
  const recents = useRecentTabsStore((s) => s.recents);
  const forget = useRecentTabsStore((s) => s.forget);
  const here = useActivePath();
  const hereKey = recentKey(here);
  const { subjects } = useSubjects();
  const [open, setOpen] = useState(
    () => localStorage.getItem(OPEN_KEY) !== "false",
  );

  useEffect(() => {
    localStorage.setItem(OPEN_KEY, String(open));
  }, [open]);

  // Nothing visited yet is nothing to say: the group appears with the trail.
  if (recents.length === 0) return null;

  const shown = recents.slice(0, SHOWN);

  return (
    <div>
      {/* Same section header as Subjects, minus the link: Recent is a list,
          not a place you can go. */}
      <div className="group/row flex items-center justify-between pl-2 pr-1 mb-0.5">
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          className="flex-1 min-w-0 truncate py-1 text-left text-[11px] font-medium tracking-wide text-muted-foreground hover:text-foreground transition-colors"
        >
          Recent
        </button>
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          aria-label={open ? "Collapse recent" : "Expand recent"}
          aria-expanded={open}
          className="flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground transition-colors"
        >
          <CaretRight
            size={11}
            className={cn(
              "hidden group-hover/row:block transition-transform",
              open && "rotate-90",
            )}
          />
          <span className="text-[10px] tabular-nums opacity-60 group-hover/row:hidden">
            {shown.length}
          </span>
        </button>
      </div>

      {open && (
        <div className="space-y-0.5">
          {shown.map((entry) => {
            // Browser tabs never enter the trail, so there is none to pass.
            const { title, icon } = tabInfo(entry.path, subjects, [], 15);
            return (
              <div key={entry.key} className="group/recent relative">
                <button
                  type="button"
                  title={title}
                  data-tab-href={entry.path}
                  onClick={() => navigateActive(entry.path)}
                  className={cn(
                    "flex w-full items-center gap-2.5 rounded-md pl-2 pr-7 py-1.5 text-[12.5px] transition-colors",
                    // Matched on the entry's identity, not its path: one
                    // row stands for a whole subject, so it lights from any
                    // of its tabs — while two files of that subject, which
                    // differ only in `?path=`, stay two rows and light one at
                    // a time.
                    entry.key === hereKey
                      ? "bg-sidebar-item-active text-foreground font-medium"
                      : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
                  )}
                >
                  <span className="shrink-0">{icon}</span>
                  <span className="truncate flex-1 text-left">{title}</span>
                </button>
                {/* Drops the entry, the way a tab's × drops the tab. Absolute
                    so appearing on hover can't re-lay out the row. */}
                <button
                  type="button"
                  onClick={() => forget(entry.key)}
                  aria-label={`Remove ${title} from recent`}
                  className={cn(
                    "absolute right-1 top-1/2 -translate-y-1/2 flex h-5 w-5 items-center justify-center rounded-md",
                    "text-muted-foreground hover:text-foreground hover:bg-sidebar-item-hover",
                    "opacity-0 focus-visible:opacity-100 group-hover/recent:opacity-100 transition-opacity",
                  )}
                >
                  <X size={11} weight="bold" />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
