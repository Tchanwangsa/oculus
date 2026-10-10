import { memo, useCallback, useMemo, useState } from "react";
import { usePagedRows } from "@/components/ui/table/TablePagination";
import { GridTable } from "@/components/ui/table/GridTable";
import { useNow } from "@/hooks/ui/useNow";
import {
  statusOf,
  usePipelineStore,
  type PipelineItem,
  type PipelinePhase,
} from "@/stores/sync/pipelineStore";
import { COLS } from "@/components/sync/pipeline/constants";
import { latestStageAt } from "@/components/sync/pipeline/facts";
import { Row } from "@/components/sync/pipeline/Row";

/**
 * The ingest ledger: one row per PDF through Download → Parse → Embed, live
 * work ranked first. A row is the file, one segmented track — itself the
 * status — with a caption saying what is happening, and when the file last
 * moved; actions take the time's place on hover. It expands into the facts the row
 * leaves out. With no Voyage key the embed segment isn't drawn (`embedStage`
 * in `pipelineStore`, set from `indexStore`), so a parsed file is done at two;
 * a spreadsheet is never embedded, so its row is two stages, its parse being
 * the conversion to text (`embedsIn`).
 */

/** Sort: running work first, then the queue, then paused, then failures,
 *  then skips; done last. Within a group, the latest stage completion first
 *  — not `updatedAt`, which every progress event bumps, so live rows would
 *  swap places and jump pages. */
const PHASE_RANK: Record<PipelinePhase, number> = {
  active: 0,
  waiting: 1,
  paused: 2,
  failed: 3,
  skipped: 4,
  done: 5,
};

function byActivity(embedStage: boolean) {
  return (a: PipelineItem, b: PipelineItem): number => {
    const ra = PHASE_RANK[statusOf(a, embedStage).phase];
    const rb = PHASE_RANK[statusOf(b, embedStage).phase];
    if (ra !== rb) return ra - rb;
    const ta = latestStageAt(a) || a.startedAt;
    const tb = latestStageAt(b) || b.startedAt;
    if (ta !== tb) return tb - ta;
    return a.relativePath.localeCompare(b.relativePath);
  };
}

/** Files per page; caps how many live rows animate. */
const PAGE_SIZE = 50;

const HEADER = (
  <>
    <span className="text-[11px] font-medium text-muted-foreground">File</span>
    <span className="text-[11px] font-medium text-muted-foreground">Progress</span>
    <span className="justify-self-end text-[11px] font-medium text-muted-foreground">Updated</span>
  </>
);

export const PipelineTable = memo(function PipelineTable({
  items,
  onResume,
  onSkip,
}: {
  items: PipelineItem[];
  onResume?: (item: PipelineItem) => void;
  onSkip?: (item: PipelineItem) => void;
}) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const embedStage = usePipelineStore((s) => s.embedStage);
  const now = useNow(30_000).getTime();

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
    <div className="@container h-full">
      <GridTable
        cols={COLS}
        header={HEADER}
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
              now={now}
              onToggle={toggle}
              onResume={onResume}
              onSkip={onSkip}
            />
          ))}
        </div>
      </GridTable>
    </div>
  );
});
