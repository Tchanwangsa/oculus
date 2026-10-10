import { memo } from "react";
import { CaretRight, Play, SidebarSimple, SkipForward } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { displayCode, fmtAgo } from "@/lib/format/format";
import { fileIconFor } from "@/lib/files/fileTypes";
import { filePagePath } from "@/lib/files/openFile";
import { embedsIn, statusOf, type PipelineItem } from "@/stores/sync/pipelineStore";
import { actionsOf, openItem } from "@/components/sync/pipeline/actions";
import { CaptionText, RateLimitedCaption, captionOf } from "@/components/sync/pipeline/captions";
import { COLS, SKIP_HINT } from "@/components/sync/pipeline/constants";
import { Detail } from "@/components/sync/pipeline/Detail";
import { IconAction } from "@/components/sync/pipeline/IconAction";
import { isRateLimited, latestStageAt } from "@/components/sync/pipeline/facts";
import { Track } from "@/components/sync/pipeline/Track";

export const Row = memo(function Row({
  item,
  embedStage,
  expanded,
  now,
  onToggle,
  onResume,
  onSkip,
}: {
  item: PipelineItem;
  embedStage: boolean;
  expanded: boolean;
  /** The table's ticking clock; a new value re-renders the memoised row so
   *  its relative time and upload estimate don't go stale. */
  now: number;
  onToggle: (path: string) => void;
  onResume?: (item: PipelineItem) => void;
  onSkip?: (item: PipelineItem) => void;
}) {
  // A spreadsheet has no embed stage whatever the app's setting.
  const stages = embedsIn(item, embedStage);
  const s = statusOf(item, stages, now);
  const actions = actionsOf(item, s, stages, !!onResume, !!onSkip);
  const run = onResume && (() => onResume(item));
  const skip = onSkip && (() => onSkip(item));
  const latest = latestStageAt(item);
  const href = filePagePath(item.subjectId, item.relativePath);
  const Icon = fileIconFor(item.filename);

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        onClick={() => onToggle(item.relativePath)}
        onKeyDown={(e) => {
          // Keys on the controls inside are theirs.
          if (e.target !== e.currentTarget) return;
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onToggle(item.relativePath);
          }
        }}
        className={cn(COLS, "group/row cursor-pointer py-2 transition-colors hover:bg-surface/60")}
      >
        <div className="flex min-w-0 items-center gap-2">
          <CaretRight
            size={9}
            className={cn(
              "shrink-0 text-muted-foreground/50 transition-transform will-change-transform",
              expanded && "rotate-90",
            )}
          />
          <Icon size={13} className="shrink-0 text-muted-foreground/70" />
          <button
            type="button"
            data-tab-href={href}
            onClick={(e) => {
              e.stopPropagation();
              openItem(item);
            }}
            className="min-w-0 truncate rounded-sm text-left text-xs text-foreground decoration-muted-foreground/50 underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            {item.filename}
          </button>
          {item.code && (
            <span className="shrink-0 text-[11px] text-muted-foreground/70 @max-lg:hidden">
              {displayCode(item.code)}
            </span>
          )}
        </div>

        <div className="flex min-w-0 items-center gap-3">
          <Track item={item} s={s} embedStage={stages} />
          {isRateLimited(item) ? (
            <RateLimitedCaption item={item} embedStage={stages} />
          ) : (
            <CaptionText text={captionOf(item, s)} tone={s.phase === "failed" ? "bad" : undefined} />
          )}
        </div>

        {/* The time, and on hover or keyboard focus the row's actions in its place. */}
        <div className="relative flex h-6 items-center justify-end">
          <span
            className="text-[11px] tabular-nums text-muted-foreground transition-opacity will-change-[opacity] group-focus-within/row:opacity-0 group-hover/row:opacity-0"
          >
            {latest ? fmtAgo(latest) : "—"}
          </span>
          <div className="absolute inset-y-0 right-0 flex items-center gap-0.5 opacity-0 transition-opacity will-change-[opacity] group-focus-within/row:opacity-100 group-hover/row:opacity-100">
            {actions.run && run && (
              <IconAction hint={actions.run.hint} onClick={run}>
                <Play size={11} weight="fill" />
              </IconAction>
            )}
            {actions.skip && skip && (
              <IconAction hint={SKIP_HINT} onClick={skip}>
                <SkipForward size={12} />
              </IconAction>
            )}
            <IconAction hint="Open beside" onClick={() => openItem(item)}>
              <SidebarSimple size={12} className="scale-x-[-1]" />
            </IconAction>
          </div>
        </div>
      </div>

      {expanded && (
        <Detail item={item} embedStage={stages} actions={actions} onRun={run} onSkip={skip} />
      )}
    </div>
  );
});
