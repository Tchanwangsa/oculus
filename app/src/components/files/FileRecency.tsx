import { sqliteUtcToMs } from "@/lib/format/format";
import { relativeTime } from "@/lib/activity/recents";
import type { DbFile } from "@/lib/db";

/** `"new"` while the file has never been opened, `"updated"` once it changed
 *  since it was last opened, else null. */
export function fileRecency(file: DbFile): "new" | "updated" | null {
  const openedMs = sqliteUtcToMs(file.last_accessed_at);
  const changedMs = sqliteUtcToMs(file.content_changed_at);
  // Never opened is new to the reader, including rows scraped before
  // first_seen_at existed that a later sync rewrote.
  if (openedMs == null && (!!file.first_seen_at || changedMs != null)) return "new";
  if (changedMs != null && openedMs != null && changedMs > openedMs) return "updated";
  return null;
}

/** The dot alone, for a row that shows the recency elsewhere: indigo while
 *  new, amber once changed since last opened. */
export function RecencyDot({ file }: { file: DbFile }) {
  const recency = fileRecency(file);
  if (!recency) return null;
  return (
    <span
      title={recency === "new" ? "New since last sync" : "Updated since last opened"}
      className={`size-1.5 shrink-0 rounded-full ${recency === "new" ? "bg-brand" : "bg-warning"}`}
    />
  );
}

/**
 * The recency slot on a file row: an indigo dot and "New" while the file has
 * never been opened, the same "New" in amber once it changed since last
 * opened, otherwise its last-opened time.
 */
export function FileRecency({ file }: { file: DbFile }) {
  const recency = fileRecency(file);
  const openedMs = sqliteUtcToMs(file.last_accessed_at);
  if (recency === "new") return <RecencyMark label="New" title="New since last sync" tone="text-brand" dot="bg-brand" />;
  if (recency === "updated") {
    return <RecencyMark label="New" title="Updated since last opened" tone="text-warning" dot="bg-warning" />;
  }
  return (
    <span className="shrink-0 text-[11px] text-muted-foreground/70 tabular-nums">
      {openedMs == null ? "never" : relativeTime(openedMs)}
    </span>
  );
}

function RecencyMark({ label, title, tone, dot }: { label: string; title: string; tone: string; dot: string }) {
  return (
    <span title={title} className="flex shrink-0 items-center gap-1">
      <span className={`size-1.5 rounded-full ${dot}`} />
      <span className={`text-[10px] leading-none font-semibold uppercase tracking-wide ${tone}`}>{label}</span>
    </span>
  );
}
