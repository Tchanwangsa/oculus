import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { CaretRight, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useSubjects } from "@/hooks/useSubjects";
import { useRecentTabsStore } from "@/stores/recentTabsStore";
import { useTabStore } from "@/stores/tabStore";
import { tabInfo } from "@/components/tabs/tabInfo";

const OPEN_KEY = "oculus-recent-nav-open";
/** The sidebar is a short column shared with the subjects — five is as much
 *  of the trail as earns its place. */
const SHOWN = 5;

/**
 * The Recent group: the last few pages you had open, named exactly as their
 * tabs are (`tabInfo`). Clicking one goes there in the current tab; ⌘-click
 * opens it in a new one, as the tab strip's own affordances do.
 */
export default function RecentNavGroup() {
  const recents = useRecentTabsStore((s) => s.recents);
  const forget = useRecentTabsStore((s) => s.forget);
  const addTab = useTabStore((s) => s.addTab);
  const navigate = useNavigate();
  const location = useLocation();
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
  // Matched on path *and* query: two files of one subject share a pathname
  // and differ only in `?path=`, so a pathname match would light both rows.
  const here = location.pathname + location.search;

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
              <div key={entry.path} className="group/recent relative">
                <Link
                  to={entry.path}
                  title={title}
                  onClick={(e) => {
                    if (!(e.metaKey || e.ctrlKey)) return;
                    e.preventDefault();
                    addTab(entry.path);
                    navigate(entry.path);
                  }}
                  className={cn(
                    "flex items-center gap-2.5 rounded-md pl-2 pr-7 py-1.5 text-[12.5px] transition-colors",
                    entry.path === here
                      ? "bg-sidebar-item-active text-foreground font-medium"
                      : "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground",
                  )}
                >
                  <span className="shrink-0">{icon}</span>
                  <span className="truncate flex-1">{title}</span>
                </Link>
                {/* Drops the entry, the way a tab's × drops the tab. Absolute
                    so appearing on hover can't re-lay out the row. */}
                <button
                  type="button"
                  onClick={() => forget(entry.path)}
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
