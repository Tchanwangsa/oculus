import { sqliteUtcToMs } from "@/lib/format/format";
import { relativeTime } from "@/lib/activity/recents";
import type { DbFile } from "@/lib/db";

/**
 * The recency slot on a file row: an indigo dot and "New" while the file has
 * never been opened, the dot alone once it changed since last opened, otherwise
 * its last-opened time.
 */
export function FileRecency({ file }: { file: DbFile }) {
  const openedMs = sqliteUtcToMs(file.last_accessed_at);
  const changedMs = sqliteUtcToMs(file.content_changed_at);
  // Never opened is new to the reader, including rows scraped before
  // first_seen_at existed that a later sync rewrote.
  const isNew = openedMs == null && (!!file.first_seen_at || changedMs != null);
  const isUpdated = changedMs != null && openedMs != null && changedMs > openedMs;
  if (isNew) {
    return (
      <span title="New since last sync" className="flex shrink-0 items-center gap-1">
        <span className="size-1.5 rounded-full bg-brand" />
        <span className="text-[10px] leading-none font-semibold uppercase tracking-wide text-brand">New</span>
      </span>
    );
  }
  if (isUpdated) {
    return <span title="Updated since last opened" className="shrink-0 size-1.5 rounded-full bg-brand" />;
  }
  const ms = openedMs;
  return (
    <span className="shrink-0 text-[11px] text-muted-foreground/70 tabular-nums">
      {ms == null ? "never" : relativeTime(ms)}
    </span>
  );
}
