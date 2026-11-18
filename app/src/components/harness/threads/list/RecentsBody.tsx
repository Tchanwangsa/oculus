import { useMemo, type ReactNode } from "react";
import { SIDE_NAV_FOLDS, SIDE_NAV_ROW, SIDE_NAV_ROW_IDLE, SideNavGroupLabel } from "@/components/ui/layout/SideNav";
import type { HarnessThread } from "@/lib/harness";
import { sqliteUtcToMs } from "@/lib/format/format";
import { cn } from "@/lib/utils";

/** Newest first; equal times keep the list's order (`sort` is stable). */
export function byNewest(threads: HarnessThread[]): HarnessThread[] {
  return threads.slice().sort((a, b) => (sqliteUtcToMs(b.updated_at) ?? 0) - (sqliteUtcToMs(a.updated_at) ?? 0));
}

/**
 * The column's list: "Recents", then every thread the store has read,
 * newest first with no sections, then Show more while the database holds
 * older ones (`harnessStore.showMoreThreads`).
 */
export function RecentsBody({
  threads,
  more,
  onMore,
  collapsed,
  row,
}: {
  threads: HarnessThread[];
  /** Older threads exist beyond those read. */
  more: boolean;
  onMore: () => void;
  collapsed: boolean;
  row: (thread: HarnessThread) => ReactNode;
}) {
  const sorted = useMemo(() => byNewest(threads), [threads]);
  if (!sorted.length) return null;
  return (
    <section aria-label="Recents">
      <div className={SIDE_NAV_FOLDS}>
        <SideNavGroupLabel>Recents</SideNavGroupLabel>
      </div>
      <div className="flex flex-col gap-0.5">
        {sorted.map(row)}
        {more && (
          <button
            type="button"
            tabIndex={collapsed ? -1 : undefined}
            onClick={onMore}
            className={cn(
              SIDE_NAV_ROW,
              SIDE_NAV_ROW_IDLE,
              SIDE_NAV_FOLDS,
              "px-2.5 text-[11.5px] group-data-[collapsed=true]/nav:pointer-events-none",
            )}
          >
            Show more
          </button>
        )}
      </div>
    </section>
  );
}
