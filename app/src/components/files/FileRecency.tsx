import { sqliteUtcToMs } from "@/lib/format";
import { relativeTime } from "@/lib/recents";
import type { DbFile } from "@/lib/db";

/**
 * The recency slot at the right edge of every file row: an indigo dot while
 * the file holds something the user hasn't seen — new since recency tracking
 * began and never opened, or re-scraped with changed bytes since it was last
 * opened (a module row's page updating brings its dot back). Otherwise a
 * "3d ago"-style last-opened time; files from before tracking existed show
 * "never" until first opened.
 */
export function FileRecency({ file }: { file: DbFile }) {
  const openedMs = sqliteUtcToMs(file.last_accessed_at);
  const changedMs = sqliteUtcToMs(file.content_changed_at);
  const isNew = !!file.first_seen_at && openedMs == null;
  const isUpdated = changedMs != null && (openedMs == null || changedMs > openedMs);
  if (isNew || isUpdated) {
    return (
      <span
        title={isNew ? "New since last sync" : "Updated since last opened"}
        className="shrink-0 size-1.5 rounded-full bg-primary"
      />
    );
  }
  const ms = openedMs;
  return (
    <span className="shrink-0 text-[11px] text-muted-foreground/70 tabular-nums">
      {ms == null ? "never" : relativeTime(ms)}
    </span>
  );
}
