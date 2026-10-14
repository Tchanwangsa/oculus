import type { RefObject } from "react";
import { ArrowCounterClockwise, CircleNotch, CopySimple, PencilSimple, Trash } from "@phosphor-icons/react";

import type { VersionViewHandle, VersionViewInfo } from "@/components/documents/VersionView";
import { Button } from "@/components/ui/button";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { displayLocale } from "@/lib/format/locale";
import { versionTitle, type DocumentVersion } from "@/lib/notes/documentVersions";

import { SNAPSHOT_HINT } from "../constants";
import type { CompareWith } from "../useCompare";
import { BackButton, VersionsToggle } from "./BarButtons";
import { ChangeNav } from "./ChangeNav";
import { ComparePill } from "./ComparePill";
import { wordStats } from "./labels";

export { HistoryBar } from "./HistoryBar";

/**
 * Over the version on the page: Back to the note, which version it is, its
 * actions, and the changes — stepped through, counted, compared with the
 * previous version or the note now, or hidden (one pill, `ComparePill`).
 * Always two rows: who and what to do, then the changes. Nothing wraps: the
 * title truncates, and as the bar's own width shrinks (container queries on
 * `bar`) the action labels drop to icons, Replace current first.
 */
export function VersionBar({
  version: v,
  listShown,
  onToggleList,
  onClose,
  info,
  handle,
  showChanges,
  onShowChanges,
  compare,
  onCompare,
  previousDisabled,
  busy,
  canCopy,
  onRestoreCopy,
  onReplace,
  onRename,
  onDelete,
}: {
  version: DocumentVersion;
  /** The versions sidebar, shown or not. */
  listShown: boolean;
  onToggleList: () => void;
  /** Leaves History. */
  onClose: () => void;
  info: VersionViewInfo | null;
  handle: RefObject<VersionViewHandle | null>;
  showChanges: boolean;
  onShowChanges: (on: boolean) => void;
  /** As applied: Current note for the oldest version whatever was chosen. */
  compare: CompareWith;
  onCompare: (value: CompareWith) => void;
  previousDisabled: boolean;
  busy: string | null;
  canCopy: boolean;
  onRestoreCopy: () => void;
  onReplace: () => void;
  onRename: () => void;
  onDelete: () => void;
}) {
  const at = sqliteUtcToMs(v.created_at);
  const checkpoint = v.kind === "checkpoint";
  const detail =
    v.kind === "checkpoint" ? ["Saved version", at != null ? fmtClock(at, true) : null].filter(Boolean).join(" · ")
    : (v.label ?? SNAPSHOT_HINT[v.kind]);
  const words = info && showChanges && info.hunks > 0 ? wordStats(info) : null;

  return (
    <div className="@container/bar shrink-0 border-b border-border-subtle bg-card px-3 pt-2 pb-1.5">
      <div className="flex items-center gap-2">
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          <BackButton onClick={onClose} />
          <div className="min-w-0 flex-1">
            <p className="truncate font-display text-[15px] leading-tight font-semibold text-foreground">
              {versionTitle(v)}
            </p>
            <p className="mt-0.5 truncate text-[11.5px] text-muted-foreground">
              {detail && <>{detail} · </>}
              <span className="tabular-nums">{v.length.toLocaleString(displayLocale)} chars</span>
            </p>
          </div>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          <Button
            size="xs"
            disabled={busy !== null || !canCopy}
            onClick={onRestoreCopy}
            aria-label="Restore as copy"
            title="Open this version as a new note"
          >
            {busy === "copy" ? <CircleNotch className="animate-spin" /> : <CopySimple />}
            <span className="@max-[28rem]/bar:hidden">Restore as copy</span>
          </Button>
          <Button
            variant="outline"
            size="xs"
            disabled={busy !== null}
            onClick={onReplace}
            aria-label="Replace current"
            title="Replace the note's text with this version"
          >
            <ArrowCounterClockwise />
            <span className="@max-[36rem]/bar:hidden">Replace current</span>
          </Button>
          {checkpoint && (
            <>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label="Rename version"
                title="Rename"
                disabled={busy !== null}
                className="text-muted-foreground"
                onClick={onRename}
              >
                <PencilSimple size={13} />
              </Button>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label="Delete version"
                title="Delete"
                disabled={busy !== null}
                className="text-muted-foreground hover:text-destructive"
                onClick={onDelete}
              >
                <Trash size={13} />
              </Button>
            </>
          )}
          <VersionsToggle shown={listShown} onToggle={onToggleList} />
        </div>
      </div>

      {/* One line at every width: the count truncates before anything wraps. */}
      <div className="mt-1.5 flex min-h-7 min-w-0 items-center gap-3">
        {showChanges && <ChangeNav info={info} handle={handle} />}
        {words && (
          <span className="shrink-0 text-[12px] whitespace-nowrap tabular-nums">
            <span className="text-success">{words.inserted}</span>{" "}
            <span className="text-destructive">{words.deleted}</span>
            <span className="text-muted-foreground @max-[28rem]/bar:hidden"> words</span>
          </span>
        )}
        <div className="ml-auto shrink-0">
          <ComparePill
            showChanges={showChanges}
            onShowChanges={onShowChanges}
            compare={compare}
            onCompare={onCompare}
            previousDisabled={previousDisabled}
          />
        </div>
      </div>
    </div>
  );
}
