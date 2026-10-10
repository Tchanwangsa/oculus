import { sqliteUtcToMs } from "@/lib/format/format";
import { relativeTime } from "@/lib/activity/recents";
import type { DbFile } from "@/lib/db";

/**
 * The recency slot on a file row: an indigo dot while the file is new and
 * unopened or changed since last opened, otherwise its last-opened time.
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
        className="shrink-0 size-1.5 rounded-full bg-brand"
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
