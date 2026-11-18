import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { HarnessThread } from "@/lib/harness";
import { sqliteUtcToMs } from "@/lib/format/format";

/** Rows drawn at first, and how many each step of the scroll adds. */
export const RECENTS_FIRST = 5;
export const RECENTS_STEP = 10;

/** Newest first; equal times keep the list's order (`sort` is stable). */
export function byNewest(threads: HarnessThread[]): HarnessThread[] {
  return threads.slice().sort((a, b) => (sqliteUtcToMs(b.updated_at) ?? 0) - (sqliteUtcToMs(a.updated_at) ?? 0));
}

/**
 * The column's list: every thread newest first with no headings (the view
 * tab already says Recents). It draws `RECENTS_FIRST` rows and adds more as
 * a sentinel under the last one nears view; past the threads the store has
 * read it asks for older ones (`harnessStore.showMoreThreads`).
 */
export function RecentsBody({
  threads,
  more,
  onMore,
  row,
}: {
  threads: HarnessThread[];
  /** Older threads exist beyond those read. */
  more: boolean;
  onMore: () => void;
  row: (thread: HarnessThread) => ReactNode;
}) {
  const sorted = useMemo(() => byNewest(threads), [threads]);
  const [shown, setShown] = useState(RECENTS_FIRST);
  const end = useRef<HTMLDivElement>(null);
  const hasMore = shown < sorted.length || more;

  // A fresh observer per step reports at once, so the list keeps growing
  // until the sentinel leaves view, then waits for the scroll.
  useEffect(() => {
    const el = end.current;
    if (!el || !hasMore) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries.some((e) => e.isIntersecting)) return;
        observer.disconnect();
        if (shown < sorted.length) setShown((n) => n + RECENTS_STEP);
        else onMore();
      },
      { rootMargin: "0px 0px 200px 0px" },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [shown, sorted.length, hasMore, onMore]);

  if (!sorted.length) return null;
  return (
    <section aria-label="Recents" className="flex flex-col gap-0.5">
      {sorted.slice(0, shown).map(row)}
      {hasMore && <div ref={end} aria-hidden className="h-px shrink-0" />}
    </section>
  );
}
