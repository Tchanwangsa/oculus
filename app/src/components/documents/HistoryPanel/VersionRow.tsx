import { BookmarkSimple, ClockCounterClockwise } from "@phosphor-icons/react";

import { fmtFullStamp, fmtTime, sqliteUtcToMs } from "@/lib/format/format";
import { versionTitle, type DocumentVersion } from "@/lib/notes/documentVersions";
import { cn } from "@/lib/utils";

import { SNAPSHOT_HINT } from "./constants";
import { fmtDelta } from "./listModel";

/** One version under its day: snapshots read as their time and why they were
 *  taken, checkpoints — the student's own — as "v3 · label" and their time,
 *  stronger. On the right, the characters gained over the next-older row. */
export function VersionRow({
  version: v,
  delta,
  selected,
  onSelect,
}: {
  version: DocumentVersion;
  delta: number | null;
  selected: boolean;
  onSelect: () => void;
}) {
  const at = sqliteUtcToMs(v.created_at);
  const checkpoint = v.kind === "checkpoint";
  const time = at != null ? fmtTime(at) : null;
  const title = checkpoint || !time ? versionTitle(v) : time;
  const detail = v.kind === "checkpoint" ? time : (v.label ?? SNAPSHOT_HINT[v.kind]);
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      // Roving: only the selected row is in the tab order; arrows move it.
      tabIndex={selected ? 0 : -1}
      data-version-id={v.id}
      onClick={onSelect}
      title={at != null ? fmtFullStamp(at) : undefined}
      className={cn(
        "flex w-full cursor-pointer items-start gap-2 px-3 py-1.5 text-left transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:ring-inset",
        selected ? "bg-accent" : "hover:bg-surface",
      )}
    >
      {checkpoint ? (
        <BookmarkSimple size={12} weight="fill" className="mt-0.5 shrink-0 text-brand" aria-hidden />
      ) : (
        <ClockCounterClockwise size={12} className="mt-0.5 shrink-0 text-muted-foreground/60" aria-hidden />
      )}
      <span className="min-w-0 flex-1">
        <span
          className={cn(
            "block truncate text-[12.5px]",
            checkpoint ? "font-medium text-foreground" : "text-foreground/90",
          )}
        >
          {title}
        </span>
        {detail && <span className="mt-0.5 block truncate text-[11px] text-muted-foreground">{detail}</span>}
      </span>
      {delta != null && (
        <span
          className={cn(
            "mt-px shrink-0 text-[11px] tabular-nums",
            delta > 0 ? "text-success/80" : delta < 0 ? "text-destructive/80" : "text-muted-foreground/70",
          )}
          title={`${fmtDelta(delta)} characters since the version before`}
        >
          {fmtDelta(delta)}
        </span>
      )}
    </button>
  );
}
