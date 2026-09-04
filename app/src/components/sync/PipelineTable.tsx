import { useState } from "react";
import { CaretDown, CaretRight, FilePdf, Play } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { fmtAgo, fmtClock } from "@/lib/format";
import {
  isComplete,
  statusOf,
  type PipelineItem,
  type PipelinePhase,
  type StageState,
} from "@/stores/pipelineStore";

/**
 * The full ingest ledger: one row per PDF, walking Download → Fast parse →
 * Quality parse → Embed. A row shows the stage dots and, for whatever is
 * running, a single percentage; clicking it expands a timeline of when each
 * step finished. Completed rows drop into a collapsed group at the bottom so
 * the live work stays on top.
 */

const STAGES = [
  { key: "download", label: "Download" },
  { key: "fast", label: "Fast parse" },
  { key: "quality", label: "Quality parse" },
  { key: "embed", label: "Embed" },
] as const;

const DOT: Record<StageState, string> = {
  pending: "bg-muted-foreground/25",
  queued: "bg-warning",
  active: "bg-primary animate-pulse",
  done: "bg-success",
  error: "bg-destructive",
};

const STAGE_STATE_LABEL: Record<StageState, string> = {
  pending: "waiting",
  queued: "queued",
  active: "in progress",
  done: "done",
  error: "failed",
};

const BADGE_VARIANT: Record<
  PipelinePhase,
  "default" | "secondary" | "success" | "destructive" | "warning"
> = {
  active: "default",
  waiting: "secondary",
  paused: "warning",
  done: "success",
  failed: "destructive",
};

/** Column template shared by the header and every row. */
const COLS =
  "grid grid-cols-[minmax(0,1fr)_100px_118px_80px_minmax(120px,160px)] items-center gap-4 px-4";

function StageDots({ item }: { item: PipelineItem }) {
  return (
    <div className="flex items-center">
      {STAGES.map((s, i) => {
        const state = item[s.key];
        return (
          <div key={s.key} className="flex items-center">
            {i > 0 && (
              <span
                className={cn(
                  "w-3.5 h-px",
                  item[STAGES[i - 1].key] === "done" ? "bg-success/50" : "bg-border",
                )}
              />
            )}
            <Tooltip>
              <TooltipTrigger asChild>
                <span className={cn("w-2 h-2 rounded-full shrink-0", DOT[state])} />
              </TooltipTrigger>
              <TooltipContent>
                {s.label} — {STAGE_STATE_LABEL[state]}
              </TooltipContent>
            </Tooltip>
          </div>
        );
      })}
    </div>
  );
}

// ── Expanded timeline ─────────────────────────────────────────────────────────

interface TimelineStep {
  label: string;
  state: StageState;
  /** Completion wall-clock time, when the step is done. */
  time?: number;
  /** Live detail for a step still moving (pages, queue position, error). */
  detail?: string;
}

function timelineSteps(item: PipelineItem): TimelineStep[] {
  const qualityDetail =
    item.quality === "active"
      ? item.totalPages > 0
        ? `${item.pagesDone}/${item.totalPages} pages · ${Math.round((item.pagesDone / item.totalPages) * 100)}%`
        : "in progress"
      : item.quality === "queued"
        ? item.qualityQueuePos
          ? item.qualityQueuePos === 1
            ? "next up"
            : `#${item.qualityQueuePos} in line`
          : "queued"
        : item.quality === "error"
          ? item.error
          : undefined;

  const embedDetail =
    item.embed === "active"
      ? item.embedTotalPages > 0
        ? `${item.embedPagesDone}/${item.embedTotalPages} pages · ${Math.round((item.embedPagesDone / item.embedTotalPages) * 100)}%`
        : "in progress"
      : item.embed === "error"
        ? item.error
        : undefined;

  return [
    { label: "Downloaded", state: item.download, time: item.downloadedAt },
    { label: "Fast parse", state: item.fast, time: item.fastParsedAt },
    { label: "Quality parse", state: item.quality, time: item.parsedAt, detail: qualityDetail },
    { label: "Embed", state: item.embed, time: item.embeddedAt, detail: embedDetail },
  ];
}

function Timeline({ item }: { item: PipelineItem }) {
  const steps = timelineSteps(item);
  return (
    <div className="px-4 pb-3 pt-1 bg-surface/50">
      <div className="ml-[3px]">
        {steps.map((step, i) => {
          const last = i === steps.length - 1;
          const meta =
            step.state === "done"
              ? step.time
                ? fmtClock(step.time)
                : "done"
              : (step.detail ?? STAGE_STATE_LABEL[step.state]);
          return (
            <div key={step.label} className="flex gap-3">
              <div className="flex flex-col items-center">
                <span className={cn("w-2 h-2 rounded-full shrink-0 mt-[5px]", DOT[step.state])} />
                {!last && <span className="w-px flex-1 min-h-3 bg-border" />}
              </div>
              <div className={cn("flex-1 flex items-baseline justify-between gap-3", !last && "pb-2.5")}>
                <span
                  className={cn(
                    "text-xs",
                    step.state === "pending" ? "text-muted-foreground/60" : "text-foreground",
                  )}
                >
                  {step.label}
                </span>
                <span
                  className={cn(
                    "text-[11px] tabular-nums text-right",
                    step.state === "error" ? "text-destructive" : "text-muted-foreground",
                  )}
                >
                  {meta}
                </span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ── Rows ──────────────────────────────────────────────────────────────────────

function Row({
  item,
  expanded,
  onToggle,
  onResume,
}: {
  item: PipelineItem;
  expanded: boolean;
  onToggle: () => void;
  onResume?: (item: PipelineItem) => void;
}) {
  const s = statusOf(item);
  const resumable = onResume && (s.phase === "paused" || s.phase === "failed");
  const percent =
    s.phase === "active" && s.percent != null ? Math.round(s.percent) : null;

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        onClick={onToggle}
        onKeyDown={(e) => e.key === "Enter" && onToggle()}
        className={cn(COLS, "py-2.5 cursor-pointer hover:bg-surface/60 transition-colors")}
      >
        <div className="flex items-center gap-2 min-w-0">
          <CaretRight
            size={9}
            className={cn(
              "shrink-0 text-muted-foreground/50 transition-transform",
              expanded && "rotate-90",
            )}
          />
          <FilePdf size={13} className="shrink-0 text-muted-foreground/70" />
          <span className="text-xs text-foreground truncate">{item.filename}</span>
        </div>

        <span className="text-[11px] text-muted-foreground truncate">{item.code}</span>

        <StageDots item={item} />

        <StageDates item={item} />

        <div className="flex items-center gap-1.5 justify-self-end">
          {percent != null && (
            <span className="text-[11px] text-muted-foreground tabular-nums">{percent}%</span>
          )}
          {resumable && (
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={s.phase === "failed" ? "Retry" : "Resume"}
                  onClick={(e) => {
                    e.stopPropagation();
                    onResume(item);
                  }}
                  className="text-muted-foreground hover:text-foreground"
                >
                  <Play size={11} weight="fill" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {s.phase === "failed" ? "Retry from the failed stage" : "Resume where it left off"}
              </TooltipContent>
            </Tooltip>
          )}
          <Badge variant={BADGE_VARIANT[s.phase]} className="text-[11px]">
            {s.short}
          </Badge>
        </div>
      </div>

      {expanded && <Timeline item={item} />}
    </div>
  );
}

/** Most recent stage timestamp, with the full breakdown one click away in the
 *  row's timeline. */
function StageDates({ item }: { item: PipelineItem }) {
  const latest = Math.max(
    item.downloadedAt ?? 0,
    item.fastParsedAt ?? 0,
    item.parsedAt ?? 0,
    item.embeddedAt ?? 0,
  );
  if (!latest) return <span className="text-[11px] text-muted-foreground/60">—</span>;
  return (
    <span className="text-[11px] text-muted-foreground tabular-nums">{fmtAgo(latest)}</span>
  );
}

/** Sort: running work first, then the queue, then paused, then failures;
 *  freshest first within each group. Completed rows are grouped separately. */
const PHASE_RANK: Record<PipelinePhase, number> = {
  active: 0,
  waiting: 1,
  paused: 2,
  failed: 3,
  done: 4,
};

function byActivity(a: PipelineItem, b: PipelineItem): number {
  const ra = PHASE_RANK[statusOf(a).phase];
  const rb = PHASE_RANK[statusOf(b).phase];
  if (ra !== rb) return ra - rb;
  return b.updatedAt - a.updatedAt;
}

export function PipelineTable({
  items,
  onResume,
}: {
  items: PipelineItem[];
  onResume?: (item: PipelineItem) => void;
}) {
  const [doneExpanded, setDoneExpanded] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const toggle = (path: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    });

  const open = items.filter((it) => !isComplete(it)).sort(byActivity);
  const done = items
    .filter(isComplete)
    .sort((a, b) => b.updatedAt - a.updatedAt);

  const renderRow = (it: PipelineItem) => (
    <Row
      key={it.relativePath}
      item={it}
      expanded={expanded.has(it.relativePath)}
      onToggle={() => toggle(it.relativePath)}
      onResume={onResume}
    />
  );

  return (
    <div className="rounded-lg border border-border-subtle overflow-hidden">
      <div className={cn(COLS, "py-2 border-b border-border-subtle bg-surface")}>
        {["File", "Subject", "Stages", "Updated", "Status"].map((h, i) => (
          <span
            key={h}
            className={cn(
              "text-[11px] font-medium text-muted-foreground uppercase tracking-wider",
              i === 4 && "justify-self-end",
            )}
          >
            {h}
          </span>
        ))}
      </div>

      {open.length === 0 && done.length === 0 && (
        <p className="px-4 py-10 text-center text-xs text-muted-foreground">
          Nothing in the pipeline — run a sync to pull new files.
        </p>
      )}

      <div className="divide-y divide-border-subtle">{open.map(renderRow)}</div>

      {done.length > 0 && (
        <>
          <button
            type="button"
            onClick={() => setDoneExpanded((v) => !v)}
            className={cn(
              "w-full flex items-center gap-1.5 px-4 py-2 text-[11px] font-medium text-muted-foreground",
              "uppercase tracking-wider hover:text-foreground transition-colors cursor-pointer",
              "border-t border-border-subtle bg-surface",
            )}
          >
            {doneExpanded ? <CaretDown size={11} /> : <CaretRight size={11} />}
            Completed ({done.length})
          </button>
          {doneExpanded && (
            <div className="divide-y divide-border-subtle border-t border-border-subtle">
              {done.map(renderRow)}
            </div>
          )}
        </>
      )}
    </div>
  );
}
