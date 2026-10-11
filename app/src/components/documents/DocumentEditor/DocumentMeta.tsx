import { Fragment, isValidElement, type ReactNode } from "react";

import { useNow } from "@/hooks/ui/useNow";
import { displayCode, displayName, fmtFullStamp, fmtRecent, sqliteUtcToMs } from "@/lib/format/format";
import { navigateActive } from "@/lib/shell/tabRouters";
import type { DbFile, Subject } from "@/lib/db";

/**
 * Subject · created · last updated · words, under the title. Created is the
 * row's first sighting (`first_seen_at`: the app's own create, or when
 * `reconcileDocuments` found a note written elsewhere). Updated is the later
 * of the row's `modified_at` — bumped on every save and on an outside edit —
 * and the session's last save; a note never edited has none.
 */
export function DocumentMeta({
  file,
  subject,
  savedAt,
  words,
}: {
  file: DbFile;
  subject: Subject | null;
  savedAt: number | null;
  words: number;
}) {
  const now = useNow();
  const created = sqliteUtcToMs(file.first_seen_at) ?? sqliteUtcToMs(file.scraped_at);
  const rowUpdated = sqliteUtcToMs(file.modified_at);
  const updated =
    savedAt != null && (rowUpdated == null || savedAt > rowUpdated) ? savedAt : rowUpdated;

  const items: ReactNode[] = [];
  if (subject) {
    const href = `/subjects/${subject.id}`;
    items.push(
      <button
        key="subject"
        type="button"
        data-tab-href={href}
        onClick={() => navigateActive(href)}
        title={displayCode(subject.code)}
        className="min-w-0 cursor-pointer truncate transition-colors hover:text-foreground"
      >
        {displayName(subject.name, subject.code)}
      </button>,
    );
  }
  if (created != null) {
    items.push(
      <span key="created" title={fmtFullStamp(created)} className="shrink-0">
        Created {fmtRecent(created, now)}
      </span>,
    );
  }
  if (updated != null) {
    items.push(
      <span key="updated" title={fmtFullStamp(updated)} className="shrink-0">
        Last updated {fmtRecent(updated, now)}
      </span>,
    );
  }
  items.push(
    <span key="words" className="shrink-0 tabular-nums">
      {words.toLocaleString()} {words === 1 ? "word" : "words"}
    </span>,
  );

  return (
    <div className="mt-1.5 flex min-w-0 items-center gap-1.5 text-[12px] text-muted-foreground">
      {items.map((item, i) => (
        <Fragment key={isValidElement(item) ? item.key : i}>
          {i > 0 && (
            <span aria-hidden className="shrink-0 text-muted-foreground/50">
              ·
            </span>
          )}
          {item}
        </Fragment>
      ))}
    </div>
  );
}
