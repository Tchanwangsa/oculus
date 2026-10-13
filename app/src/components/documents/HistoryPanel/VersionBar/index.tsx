import type { RefObject } from "react";
import {
  ArrowCounterClockwise,
  ArrowLeft,
  CircleNotch,
  CopySimple,
  ListBullets,
  PencilSimple,
  Trash,
  X,
} from "@phosphor-icons/react";

import type { VersionViewHandle, VersionViewInfo } from "@/components/documents/VersionView";
import { Button } from "@/components/ui/button";
import type { WidthTier } from "@/hooks/ui/useWidthTier";
import { fmtClock, sqliteUtcToMs } from "@/lib/format/format";
import { displayLocale } from "@/lib/format/locale";
import { versionTitle, type DocumentVersion } from "@/lib/notes/documentVersions";
import { cn } from "@/lib/utils";

import { SNAPSHOT_HINT } from "../constants";
import type { CompareWith } from "../useCompare";
import { ChangeNav } from "./ChangeNav";
import { ComparePill } from "./ComparePill";
import { wordStats } from "./labels";

/**
 * Over the version on the page: which version it is, its actions, and the
 * changes — stepped through, counted, compared with the previous version or
 * the note now, or hidden (one pill, `ComparePill`). Two rows: who and what
 * to do; then the changes, always one line. At narrow the actions but Restore
 * as copy go icon-only and the title truncates, so the bar stays two rows;
 * wider, the first row may wrap its actions under the title.
 */
export function VersionBar({
  version: v,
  tier,
  onBack,
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
  tier: WidthTier;
  /** Narrow: back to the list. */
  onBack: () => void;
  /** Medium: the floating list, shown or not. */
  listShown: boolean;
  onToggleList: () => void;
  /** Set when no list header (and its close) is on screen. */
  onClose?: () => void;
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

  const narrow = tier === "narrow";

  return (
    <div className="shrink-0 border-b border-border-subtle bg-card px-3 pt-2 pb-1.5">
      <div className={cn("flex items-center gap-x-2 gap-y-1.5", narrow ? "flex-nowrap" : "flex-wrap")}>
        <div className={cn("flex min-w-0 flex-1 items-center gap-1.5", !narrow && "min-w-[12rem]")}>
          {narrow && (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Back to versions"
              title="Back to versions"
              className="-ml-1 text-muted-foreground"
              onClick={onBack}
            >
              <ArrowLeft size={14} />
            </Button>
          )}
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
            title="Open this version as a new note"
          >
            {busy === "copy" ? <CircleNotch className="animate-spin" /> : <CopySimple />}
            Restore as copy
          </Button>
          {narrow ? (
            <Button
              variant="ghost"
              size="icon-xs"
              disabled={busy !== null}
              onClick={onReplace}
              aria-label="Replace current"
              title="Replace the note's text with this version"
              className="text-muted-foreground"
            >
              <ArrowCounterClockwise size={13} />
            </Button>
          ) : (
            <Button
              variant="outline"
              size="xs"
              disabled={busy !== null}
              onClick={onReplace}
              title="Replace the note's text with this version"
            >
              <ArrowCounterClockwise />
              Replace current
            </Button>
          )}
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
          {tier === "medium" && (
            <Button
              variant="ghost"
              size="xs"
              aria-pressed={listShown}
              onClick={onToggleList}
              className={cn(listShown ? "text-foreground" : "text-muted-foreground")}
            >
              <ListBullets />
              Versions
            </Button>
          )}
          {onClose && (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Close history"
              title="Close history"
              className="text-muted-foreground"
              onClick={onClose}
            >
              <X size={13} />
            </Button>
          )}
        </div>
      </div>

      {/* One line at every width: the count truncates before anything wraps. */}
      <div className="mt-1.5 flex min-h-7 min-w-0 items-center gap-3">
        {showChanges && <ChangeNav info={info} handle={handle} />}
        {words && (
          <span className="shrink-0 text-[12px] whitespace-nowrap tabular-nums">
            <span className="text-success">{words.inserted}</span>{" "}
            <span className="text-destructive">{words.deleted}</span>
            {!narrow && <span className="text-muted-foreground"> words</span>}
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
