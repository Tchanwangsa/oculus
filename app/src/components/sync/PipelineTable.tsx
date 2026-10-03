import { memo, useCallback, useMemo, useState } from "react";
import { CaretRight, FilePdf, Play } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { usePagedRows } from "@/components/ui/TablePagination";
import { GridTable, HeaderLabels } from "@/components/ui/GridTable";
import { fmtAgo, fmtClock } from "@/lib/format";
import {
  statusOf,
  usePipelineStore,
  type PipelineItem,
  type PipelinePhase,
  type StageState,
} from "@/stores/pipelineStore";

/**
 * The ingest ledger: one row per PDF through Download → Parse → Embed, live
 * work ranked first; a row expands into a step timeline. With no Voyage key
 * the embed dot isn't drawn (`embedStage` in `pipelineStore`, set from
 * `indexStore`), so a parsed file is done at two dots rather than stalled.
 */

const DOWNLOAD_PARSE = [
  { key: "download", label: "Download" },
  { key: "parse", label: "Parse" },
] as const;

const EMBED_STAGE = { key: "embed", label: "Embed" } as const;

type Stage = (typeof DOWNLOAD_PARSE)[number] | typeof EMBED_STAGE;

function stages(embedStage: boolean): readonly Stage[] {
  return embedStage ? [...DOWNLOAD_PARSE, EMBED_STAGE] : DOWNLOAD_PARSE;
}

const DOT: Record<StageState, string> = {
  pending: "bg-muted-foreground/25",
  queued: "bg-warning",
  active: "bg-brand animate-pulse",
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

/** Shared by header and rows; stages column is sized to its header word. */
const COLS =
  "grid grid-cols-[minmax(0,1fr)_100px_60px_80px_minmax(120px,160px)] items-center gap-4 px-5";

function StageDots({ item, embedStage }: { item: PipelineItem; embedStage: boolean }) {
  const STAGES = stages(embedStage);
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

function timelineSteps(item: PipelineItem, embedStage: boolean): TimelineStep[] {
  const parseDetail =
    item.parse === "active"
      ? item.totalPages > 0
        ? `${item.pagesDone}/${item.totalPages} pages · ${Math.round((item.pagesDone / item.totalPages) * 100)}%`
        : "in progress"
      : item.parse === "queued"
        ? item.parseQueuePos
          ? item.parseQueuePos === 1
            ? "next up"
            : `#${item.parseQueuePos} in line`
          : "queued"
        : item.parse === "error"
          ? item.error
          : undefined;

  // Embed shows a page fraction — the only thing that moves during one long
  // blocking call (see docs/retrieval.md: an embed blocks for minutes).
  const embedDetail =
    item.embed === "active"
      ? item.embedTotalPages > 0
        ? `${item.embedPagesDone}/${item.embedTotalPages} pages · ${Math.round((item.embedPagesDone / item.embedTotalPages) * 100)}%`
        : "in progress"
      : item.embed === "queued"
        ? "queued"
        : item.embed === "error"
          ? item.error
          : undefined;

  const steps: TimelineStep[] = [
    { label: "Downloaded", state: item.download, time: item.downloadedAt },
    { label: "Parse", state: item.parse, time: item.parsedAt, detail: parseDetail },
  ];
  if (embedStage) {
    steps.push({
      label: "Embed",
      state: item.embed,
      time: item.embeddedAt,
      detail: embedDetail,
    });
  }
  return steps;
}

function Timeline({ item, embedStage }: { item: PipelineItem; embedStage: boolean }) {
  const steps = timelineSteps(item, embedStage);
  return (
    <div className="px-9 pb-3 pt-1">
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

const Row = memo(function Row({
  item,
  embedStage,
  expanded,
  onToggle,
  onResume,
}: {
  item: PipelineItem;
  embedStage: boolean;
  expanded: boolean;
  onToggle: (path: string) => void;
  onResume?: (item: PipelineItem) => void;
}) {
  const s = statusOf(item, embedStage);
  // A parsed-but-unembedded file gets ▶ too: the backlog isn't swept
  // automatically, so this embeds one file without committing the library.
  const embedNow = embedStage && item.parse === "done" && item.embed === "pending";
  const resumable = onResume && (s.phase === "paused" || s.phase === "failed" || embedNow);
  const percent =
    s.phase === "active" && s.percent != null ? Math.round(s.percent) : null;

  return (
    <div>
      <div
        role="button"
        tabIndex={0}
        onClick={() => onToggle(item.relativePath)}
        onKeyDown={(e) => e.key === "Enter" && onToggle(item.relativePath)}
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

        <StageDots item={item} embedStage={embedStage} />

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
                  aria-label={s.phase === "failed" ? "Retry" : embedNow ? "Embed" : "Resume"}
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
                {s.phase === "failed"
                  ? "Try this file again"
                  : embedNow
                    ? "Embed this file now"
                    : "Resume where it left off"}
              </TooltipContent>
            </Tooltip>
          )}
          <Badge variant={BADGE_VARIANT[s.phase]} className="text-[11px]">
            {s.short}
          </Badge>
        </div>
      </div>

      {expanded && <Timeline item={item} embedStage={embedStage} />}
    </div>
  );
});

/** Most recent stage timestamp; the full breakdown is in the timeline. */
function StageDates({ item }: { item: PipelineItem }) {
  const latest = Math.max(item.downloadedAt ?? 0, item.parsedAt ?? 0);
  if (!latest) return <span className="text-[11px] text-muted-foreground/60">—</span>;
  return (
    <span className="text-[11px] text-muted-foreground tabular-nums">{fmtAgo(latest)}</span>
  );
}

/** Sort: running work first, then the queue, then paused, then failures;
 *  done last; freshest first within each group. */
const PHASE_RANK: Record<PipelinePhase, number> = {
  active: 0,
  waiting: 1,
  paused: 2,
  failed: 3,
  done: 4,
};

function byActivity(embedStage: boolean) {
  return (a: PipelineItem, b: PipelineItem): number => {
    const ra = PHASE_RANK[statusOf(a, embedStage).phase];
    const rb = PHASE_RANK[statusOf(b, embedStage).phase];
    if (ra !== rb) return ra - rb;
    return b.updatedAt - a.updatedAt;
  };
}

/** Files per page; caps how many live rows animate. */
const PAGE_SIZE = 50;

export const PipelineTable = memo(function PipelineTable({
  items,
  onResume,
}: {
  items: PipelineItem[];
  onResume?: (item: PipelineItem) => void;
}) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const embedStage = usePipelineStore((s) => s.embedStage);

  const toggle = useCallback((path: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    }), []);

  const sorted = useMemo(
    () => [...items].sort(byActivity(embedStage)),
    [items, embedStage],
  );
  const { page, pageCount, setPage, pageRows } = usePagedRows(sorted, PAGE_SIZE);

  return (
    <GridTable
      cols={COLS}
      header={<HeaderLabels labels={["File", "Subject", "Stages", "Updated", "Status"]} endLast />}
      empty={sorted.length === 0 && "Nothing in the pipeline — run a sync to pull new files."}
      pagination={{ page, pageCount, onPage: setPage, total: sorted.length, unit: "file" }}
    >
      <div className="divide-y divide-border-subtle">
        {pageRows.map((it) => (
          <Row
            key={it.relativePath}
            item={it}
            embedStage={embedStage}
            expanded={expanded.has(it.relativePath)}
            onToggle={toggle}
            onResume={onResume}
          />
        ))}
      </div>
    </GridTable>
  );
});
