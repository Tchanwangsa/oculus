import { BookmarkSimple, ClockCounterClockwise } from "@phosphor-icons/react";

import { fmtFullStamp, fmtRecent, sqliteUtcToMs } from "@/lib/format/format";
import { versionTitle, type DocumentVersion } from "@/lib/notes/documentVersions";
import { cn } from "@/lib/utils";

import { SNAPSHOT_HINT } from "./constants";

/** One version: its title, then when (checkpoints) or why (snapshots), and
 *  its length. Checkpoints — the student's own — read stronger. */
export function VersionRow({
  version: v,
  now,
  selected,
  onSelect,
}: {
  version: DocumentVersion;
  now: Date;
  selected: boolean;
  onSelect: () => void;
}) {
  const at = sqliteUtcToMs(v.created_at);
  const checkpoint = v.kind === "checkpoint";
  const detail =
    v.kind !== "checkpoint" ? (v.label ?? SNAPSHOT_HINT[v.kind])
    : at != null ? fmtRecent(at, now)
    : null;
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      title={at != null ? fmtFullStamp(at) : undefined}
      className={cn(
        "flex w-full cursor-pointer items-start gap-2 px-3 py-1.5 text-left transition-colors",
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
            "block truncate text-[12px]",
            checkpoint ? "font-medium text-foreground" : "text-muted-foreground",
          )}
        >
          {versionTitle(v)}
        </span>
        <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
          {detail && <span className="min-w-0 truncate">{detail}</span>}
          {detail && (
            <span aria-hidden className="shrink-0 text-muted-foreground/50">
              ·
            </span>
          )}
          <span className="shrink-0 tabular-nums">{v.length.toLocaleString()} chars</span>
        </span>
      </span>
    </button>
  );
}
